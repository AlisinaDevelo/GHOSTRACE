//! The single durable writer boundary.
//!
//! A writer owns one FIFO worker and admits only bounded work.  Queue pressure
//! is observable: callers either wait for a bounded interval, receive an
//! explicit rejection, or receive a source-labelled gap outcome.  The worker
//! acknowledges only after the journal transaction has committed.

use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, RecvTimeoutError, SyncSender},
        Arc, Condvar, Mutex,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use chrono::Utc;
use rusqlite::ErrorCode;
use uuid::Uuid;

use crate::{
    error::GhostraceError,
    journal::{DiagnosticRecord, Journal},
    model::{EventEnvelope, EventSource, IngestionOrigin},
    policy::PolicyProfile,
};

pub const DEFAULT_QUEUE_ITEMS: usize = 64;
pub const DEFAULT_MAX_BATCH_ITEMS: usize = 16;
pub const DEFAULT_MAX_MEMORY_BYTES: u64 = 4 * 1024 * 1024;
pub const DEFAULT_MAX_WAIT_MS: u64 = 250;
pub const DEFAULT_MAX_RETRIES: u8 = 2;

const MAX_QUEUE_ITEMS: usize = 4_096;
const MAX_BATCH_ITEMS: usize = 1_024;
const MAX_MEMORY_BYTES: u64 = 64 * 1024 * 1024;
const MAX_WAIT_MS: u64 = 30_000;
const MAX_RETRIES: u8 = 8;

/// What a source adapter observes when its bounded queue has no admission
/// capacity.  `Gap` is an explicit outcome and must be forwarded to the
/// source's repair path; it is never an implicit drop.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QueueFullPolicy {
    Block,
    Reject,
    EmitGap,
}

impl Default for QueueFullPolicy {
    fn default() -> Self {
        Self::Block
    }
}

/// Runtime limits for one durable writer.  The limits are public contracts so
/// an adapter can record the exact admission policy it used.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WriterConfig {
    pub queue_items: usize,
    pub max_batch_items: usize,
    pub max_memory_bytes: u64,
    pub max_wait: Duration,
    pub max_retries: u8,
    pub default_queue_full_policy: QueueFullPolicy,
    pub source_queue_full_policies: BTreeMap<EventSource, QueueFullPolicy>,
}

impl Default for WriterConfig {
    fn default() -> Self {
        Self {
            queue_items: DEFAULT_QUEUE_ITEMS,
            max_batch_items: DEFAULT_MAX_BATCH_ITEMS,
            max_memory_bytes: DEFAULT_MAX_MEMORY_BYTES,
            max_wait: Duration::from_millis(DEFAULT_MAX_WAIT_MS),
            max_retries: DEFAULT_MAX_RETRIES,
            default_queue_full_policy: QueueFullPolicy::default(),
            source_queue_full_policies: BTreeMap::new(),
        }
    }
}

impl WriterConfig {
    pub fn with_source_queue_policy(
        mut self,
        source: EventSource,
        policy: QueueFullPolicy,
    ) -> Self {
        self.source_queue_full_policies.insert(source, policy);
        self
    }

    pub fn queue_full_policy(&self, source: EventSource) -> QueueFullPolicy {
        self.source_queue_full_policies
            .get(&source)
            .copied()
            .unwrap_or(self.default_queue_full_policy)
    }

    pub fn validate(&self) -> Result<(), GhostraceError> {
        if self.queue_items == 0 || self.queue_items > MAX_QUEUE_ITEMS {
            return Err(GhostraceError::InvalidWriterConfig(
                "queue_items must be between 1 and 4096".to_owned(),
            ));
        }
        if self.max_batch_items == 0 || self.max_batch_items > MAX_BATCH_ITEMS {
            return Err(GhostraceError::InvalidWriterConfig(
                "max_batch_items must be between 1 and 1024".to_owned(),
            ));
        }
        if self.max_memory_bytes == 0 || self.max_memory_bytes > MAX_MEMORY_BYTES {
            return Err(GhostraceError::InvalidWriterConfig(
                "max_memory_bytes must be between 1 and 67108864".to_owned(),
            ));
        }
        let max_wait_ms = self.max_wait.as_millis();
        if max_wait_ms == 0 || max_wait_ms > u128::from(MAX_WAIT_MS) {
            return Err(GhostraceError::InvalidWriterConfig(
                "max_wait must be between 1ms and 30000ms".to_owned(),
            ));
        }
        if self.max_retries > MAX_RETRIES {
            return Err(GhostraceError::InvalidWriterConfig(
                "max_retries must be at most 8".to_owned(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WriteAck {
    pub request_id: Uuid,
    pub source: EventSource,
    pub ingest_sequences: Vec<u64>,
    pub event_ids: Vec<Uuid>,
    pub policy_profile_id: String,
    pub policy_profile_version: u32,
    pub diagnostic_count: usize,
    pub attempts: u32,
    pub queue_wait_ms: u64,
    pub committed_at: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WriterGapReason {
    QueueFull,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WriterGap {
    pub source: EventSource,
    pub event_count: usize,
    pub reason: WriterGapReason,
}

#[derive(Debug)]
pub enum WriterSubmission {
    Queued(WriteTicket),
    Gap(WriterGap),
}

#[derive(Debug)]
pub enum WriterOutcome {
    Committed(WriteAck),
    Gap(WriterGap),
}

/// A queued request's acknowledgement.  Cancellation is best effort and is
/// accepted only before the worker starts the SQLite transaction; once started,
/// the request is allowed to finish and its acknowledgement remains durable.
#[derive(Debug)]
pub struct WriteTicket {
    request_id: Uuid,
    source: EventSource,
    receiver: Receiver<Result<WriteAck, GhostraceError>>,
    cancelled: Arc<AtomicBool>,
    started: Arc<AtomicBool>,
}

impl WriteTicket {
    pub fn request_id(&self) -> Uuid {
        self.request_id
    }

    pub fn source(&self) -> EventSource {
        self.source
    }

    /// Mark a request cancelled if it has not begun its transaction.  A false
    /// result means the worker already owns the request and will acknowledge it.
    pub fn cancel(&self) -> bool {
        if self.started.load(Ordering::Acquire) {
            return false;
        }
        self.cancelled.store(true, Ordering::Release);
        true
    }

    pub fn wait(self) -> Result<WriteAck, GhostraceError> {
        self.receiver.recv().unwrap_or(Err(GhostraceError::WriterStopped))
    }

    pub fn wait_timeout(self, timeout: Duration) -> Result<WriteAck, GhostraceError> {
        match self.receiver.recv_timeout(timeout) {
            Ok(result) => result,
            Err(RecvTimeoutError::Timeout) => Err(GhostraceError::WriterAckTimeout {
                max_wait_ms: timeout.as_millis().min(u128::from(u64::MAX)) as u64,
            }),
            Err(RecvTimeoutError::Disconnected) => Err(GhostraceError::WriterStopped),
        }
    }
}

pub struct Writer {
    sender: Option<SyncSender<WriteRequest>>,
    state: Arc<AdmissionState>,
    config: WriterConfig,
    worker: Option<JoinHandle<()>>,
}

impl std::fmt::Debug for Writer {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Writer").field("config", &self.config).finish_non_exhaustive()
    }
}

impl Writer {
    pub fn new(journal: Journal, config: WriterConfig) -> Result<Self, GhostraceError> {
        config.validate()?;
        let (sender, receiver) = mpsc::sync_channel(config.queue_items);
        let state = Arc::new(AdmissionState::default());
        let worker_state = Arc::clone(&state);
        let worker_config = config.clone();
        let worker = thread::Builder::new()
            .name("ghostrace-durable-writer".to_owned())
            .spawn(move || worker_loop(journal, worker_config, receiver, worker_state, None))
            .map_err(|_| GhostraceError::WriterStopped)?;
        Ok(Self { sender: Some(sender), state, config, worker: Some(worker) })
    }

    #[cfg(test)]
    fn new_with_gate(
        journal: Journal,
        config: WriterConfig,
        gate: TestGate,
    ) -> Result<Self, GhostraceError> {
        config.validate()?;
        let (sender, receiver) = mpsc::sync_channel(config.queue_items);
        let state = Arc::new(AdmissionState::default());
        let worker_state = Arc::clone(&state);
        let worker_config = config.clone();
        let worker = thread::Builder::new()
            .name("ghostrace-test-writer".to_owned())
            .spawn(move || worker_loop(journal, worker_config, receiver, worker_state, Some(gate)))
            .map_err(|_| GhostraceError::WriterStopped)?;
        Ok(Self { sender: Some(sender), state, config, worker: Some(worker) })
    }

    pub fn config(&self) -> &WriterConfig {
        &self.config
    }

    pub fn enqueue(
        &self,
        origin: IngestionOrigin,
        events: Vec<EventEnvelope>,
        policy: PolicyProfile,
        diagnostics: Vec<DiagnosticRecord>,
    ) -> Result<WriterSubmission, GhostraceError> {
        let source = validate_batch(&events, &self.config)?;
        let bytes = estimate_request_bytes(&events, &policy, &diagnostics)?;
        if bytes > self.config.max_memory_bytes {
            return Err(GhostraceError::WriterMemoryBound {
                bytes,
                max_bytes: self.config.max_memory_bytes,
            });
        }

        match self.state.reserve(
            source,
            self.config.queue_full_policy(source),
            self.config.queue_items,
            bytes,
            self.config.max_memory_bytes,
            self.config.max_wait,
        )? {
            Admission::Gap => {
                return Ok(WriterSubmission::Gap(WriterGap {
                    source,
                    event_count: events.len(),
                    reason: WriterGapReason::QueueFull,
                }));
            }
            Admission::Reserved => {}
        }

        let request_id = Uuid::new_v4();
        let cancelled = Arc::new(AtomicBool::new(false));
        let started = Arc::new(AtomicBool::new(false));
        let (response_sender, response_receiver) = mpsc::channel();
        let request = WriteRequest {
            request_id,
            source,
            origin,
            events,
            policy,
            diagnostics,
            response_sender,
            cancelled: Arc::clone(&cancelled),
            started: Arc::clone(&started),
            reserved_bytes: bytes,
            enqueued_at: Instant::now(),
        };
        let Some(sender) = self.sender.as_ref() else {
            self.state.release(bytes);
            return Err(GhostraceError::WriterStopped);
        };
        if sender.send(request).is_err() {
            self.state.release(bytes);
            return Err(GhostraceError::WriterStopped);
        }
        Ok(WriterSubmission::Queued(WriteTicket {
            request_id,
            source,
            receiver: response_receiver,
            cancelled,
            started,
        }))
    }

    pub fn submit(
        &self,
        origin: IngestionOrigin,
        events: Vec<EventEnvelope>,
        policy: PolicyProfile,
        diagnostics: Vec<DiagnosticRecord>,
    ) -> Result<WriterOutcome, GhostraceError> {
        match self.enqueue(origin, events, policy, diagnostics)? {
            WriterSubmission::Queued(ticket) => Ok(WriterOutcome::Committed(ticket.wait()?)),
            WriterSubmission::Gap(gap) => Ok(WriterOutcome::Gap(gap)),
        }
    }

    pub fn outstanding(&self) -> (usize, u64) {
        self.state.snapshot()
    }
}

impl Drop for Writer {
    fn drop(&mut self) {
        self.sender.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[derive(Default)]
struct AdmissionState {
    inner: Mutex<AdmissionCounters>,
    wake: Condvar,
}

#[derive(Default)]
struct AdmissionCounters {
    outstanding: usize,
    bytes: u64,
    stopped: bool,
}

enum Admission {
    Reserved,
    Gap,
}

impl AdmissionState {
    fn reserve(
        &self,
        source: EventSource,
        policy: QueueFullPolicy,
        max_items: usize,
        bytes: u64,
        max_bytes: u64,
        max_wait: Duration,
    ) -> Result<Admission, GhostraceError> {
        let deadline = Instant::now() + max_wait;
        let mut counters = self.inner.lock().map_err(|_| GhostraceError::WriterStopped)?;
        loop {
            if counters.stopped {
                return Err(GhostraceError::WriterStopped);
            }
            if counters.outstanding < max_items && counters.bytes.saturating_add(bytes) <= max_bytes
            {
                counters.outstanding += 1;
                counters.bytes = counters.bytes.saturating_add(bytes);
                return Ok(Admission::Reserved);
            }
            match policy {
                QueueFullPolicy::Reject => {
                    return Err(GhostraceError::WriterQueueFull { event_source: source });
                }
                QueueFullPolicy::EmitGap => return Ok(Admission::Gap),
                QueueFullPolicy::Block => {
                    let remaining = deadline.saturating_duration_since(Instant::now());
                    if remaining.is_zero() {
                        return Err(GhostraceError::WriterQueueWaitTimeout {
                            event_source: source,
                            max_wait_ms: max_wait.as_millis().min(u128::from(u64::MAX)) as u64,
                        });
                    }
                    let (next, result) = self
                        .wake
                        .wait_timeout(counters, remaining)
                        .map_err(|_| GhostraceError::WriterStopped)?;
                    counters = next;
                    if result.timed_out() {
                        return Err(GhostraceError::WriterQueueWaitTimeout {
                            event_source: source,
                            max_wait_ms: max_wait.as_millis().min(u128::from(u64::MAX)) as u64,
                        });
                    }
                }
            }
        }
    }

    fn release(&self, bytes: u64) {
        if let Ok(mut counters) = self.inner.lock() {
            counters.outstanding = counters.outstanding.saturating_sub(1);
            counters.bytes = counters.bytes.saturating_sub(bytes);
            self.wake.notify_all();
        }
    }

    fn snapshot(&self) -> (usize, u64) {
        self.inner.lock().map(|counters| (counters.outstanding, counters.bytes)).unwrap_or_default()
    }

    fn stop(&self) {
        if let Ok(mut counters) = self.inner.lock() {
            counters.stopped = true;
            self.wake.notify_all();
        }
    }
}

struct WriteRequest {
    request_id: Uuid,
    source: EventSource,
    origin: IngestionOrigin,
    events: Vec<EventEnvelope>,
    policy: PolicyProfile,
    diagnostics: Vec<DiagnosticRecord>,
    response_sender: mpsc::Sender<Result<WriteAck, GhostraceError>>,
    cancelled: Arc<AtomicBool>,
    started: Arc<AtomicBool>,
    reserved_bytes: u64,
    enqueued_at: Instant,
}

fn worker_loop(
    journal: Journal,
    config: WriterConfig,
    receiver: Receiver<WriteRequest>,
    state: Arc<AdmissionState>,
    #[cfg(test)] gate: Option<TestGate>,
    #[cfg(not(test))] _gate: Option<()>,
) {
    while let Ok(request) = receiver.recv() {
        #[cfg(test)]
        if let Some(gate) = gate.as_ref() {
            gate.enter_and_wait();
        }
        let result = if request.cancelled.load(Ordering::Acquire) {
            Err(GhostraceError::WriterCancelled)
        } else {
            request.started.store(true, Ordering::Release);
            if request.cancelled.load(Ordering::Acquire) {
                Err(GhostraceError::WriterCancelled)
            } else {
                let queue_wait_ms =
                    request.enqueued_at.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
                process_request(&journal, &config, &request, queue_wait_ms)
            }
        };
        state.release(request.reserved_bytes);
        let _ = request.response_sender.send(result);
    }
    state.stop();
}

fn process_request(
    journal: &Journal,
    config: &WriterConfig,
    request: &WriteRequest,
    queue_wait_ms: u64,
) -> Result<WriteAck, GhostraceError> {
    let mut attempts = 0u32;
    let sequences = loop {
        attempts += 1;
        match journal.ingest_batch_with_diagnostics(
            &request.origin,
            &request.events,
            &request.policy,
            &request.diagnostics,
        ) {
            Ok(sequences) => break sequences,
            Err(error) if retryable(&error) && attempts <= u32::from(config.max_retries) => {
                let shift = attempts.saturating_sub(1).min(7);
                let delay_ms = 5u64.saturating_mul(1u64 << shift);
                thread::sleep(Duration::from_millis(delay_ms));
            }
            Err(error) if retryable(&error) => {
                return Err(GhostraceError::WriterRetryExhausted { attempts });
            }
            Err(error) => return Err(error),
        }
    };
    Ok(WriteAck {
        request_id: request.request_id,
        source: request.source,
        ingest_sequences: sequences,
        event_ids: request.events.iter().map(|event| event.event_id).collect(),
        policy_profile_id: request.policy.id.clone(),
        policy_profile_version: request.policy.version,
        diagnostic_count: request.diagnostics.len(),
        attempts,
        queue_wait_ms,
        committed_at: Utc::now().to_rfc3339(),
    })
}

fn retryable(error: &GhostraceError) -> bool {
    matches!(
        error,
        GhostraceError::Database(rusqlite::Error::SqliteFailure(sqlite_error, _))
            if matches!(sqlite_error.code, ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked)
    )
}

fn validate_batch(
    events: &[EventEnvelope],
    config: &WriterConfig,
) -> Result<EventSource, GhostraceError> {
    if events.is_empty() {
        return Err(GhostraceError::WriterBatchBound {
            items: 0,
            max_items: config.max_batch_items,
        });
    }
    if events.len() > config.max_batch_items {
        return Err(GhostraceError::WriterBatchBound {
            items: events.len(),
            max_items: config.max_batch_items,
        });
    }
    let source = events[0].source;
    if events.iter().any(|event| event.source != source) {
        return Err(GhostraceError::WriterMixedSources);
    }
    Ok(source)
}

fn estimate_request_bytes(
    events: &[EventEnvelope],
    policy: &PolicyProfile,
    diagnostics: &[DiagnosticRecord],
) -> Result<u64, GhostraceError> {
    let event_bytes = serde_json::to_vec(events)?.len() as u64;
    let policy_bytes = serde_json::to_vec(policy)?.len() as u64;
    let diagnostic_bytes = serde_json::to_vec(diagnostics)?.len() as u64;
    let total = event_bytes
        .checked_add(policy_bytes)
        .and_then(|value| value.checked_add(diagnostic_bytes))
        .and_then(|value| value.checked_add(256))
        .ok_or_else(|| GhostraceError::WriterMemoryBound {
            bytes: u64::MAX,
            max_bytes: u64::MAX,
        })?;
    Ok(total)
}

#[cfg(test)]
#[derive(Clone, Default)]
struct TestGate {
    state: Arc<(Mutex<TestGateState>, Condvar)>,
}

#[cfg(test)]
#[derive(Default)]
struct TestGateState {
    entered: bool,
    release: bool,
}

#[cfg(test)]
impl TestGate {
    fn new() -> Self {
        Self::default()
    }

    fn enter_and_wait(&self) {
        let (lock, wake) = &*self.state;
        let mut state = lock.lock().expect("test gate lock");
        state.entered = true;
        wake.notify_all();
        while !state.release {
            state = wake.wait(state).expect("test gate wait");
        }
    }

    fn wait_until_entered(&self) {
        let (lock, wake) = &*self.state;
        let mut state = lock.lock().expect("test gate lock");
        while !state.entered {
            state = wake.wait(state).expect("test gate wait");
        }
    }

    fn release(&self) {
        let (lock, wake) = &*self.state;
        let mut state = lock.lock().expect("test gate lock");
        state.release = true;
        wake.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{fixture::read_fixture, DeterministicKeyProvider};
    use std::{path::Path, time::Duration};

    fn fixture() -> (IngestionOrigin, EventEnvelope, PolicyProfile) {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/causal-chain.jsonl");
        let event = read_fixture(path).expect("fixture").remove(0);
        (IngestionOrigin::fixture(), event, PolicyProfile::fixture_default())
    }

    #[test]
    fn queue_full_policies_are_source_specific_and_explicit() {
        let (origin, event, policy) = fixture();
        let gate = TestGate::new();
        let journal = Journal::in_memory(DeterministicKeyProvider::from_seed("writer-test-reject"))
            .expect("journal");
        let writer = Writer::new_with_gate(
            journal,
            WriterConfig {
                queue_items: 1,
                default_queue_full_policy: QueueFullPolicy::Reject,
                ..WriterConfig::default()
            },
            gate.clone(),
        )
        .expect("writer");
        let first = writer
            .enqueue(origin.clone(), vec![event.clone()], policy.clone(), Vec::new())
            .expect("first");
        gate.wait_until_entered();
        assert!(matches!(
            writer.enqueue(origin.clone(), vec![event.clone()], policy.clone(), Vec::new()),
            Err(GhostraceError::WriterQueueFull { .. })
        ));
        gate.release();
        let WriterSubmission::Queued(ticket) = first else { panic!("unexpected gap") };
        ticket.wait().expect("ack");

        let gate = TestGate::new();
        let journal = Journal::in_memory(DeterministicKeyProvider::from_seed("writer-test-gap"))
            .expect("journal");
        let writer = Writer::new_with_gate(
            journal,
            WriterConfig {
                queue_items: 1,
                default_queue_full_policy: QueueFullPolicy::EmitGap,
                ..WriterConfig::default()
            },
            gate.clone(),
        )
        .expect("writer");
        let first = writer
            .enqueue(origin.clone(), vec![event.clone()], policy.clone(), Vec::new())
            .expect("first");
        gate.wait_until_entered();
        let gap = writer.enqueue(origin, vec![event], policy, Vec::new()).expect("gap");
        assert!(matches!(
            gap,
            WriterSubmission::Gap(WriterGap { reason: WriterGapReason::QueueFull, .. })
        ));
        gate.release();
        let WriterSubmission::Queued(ticket) = first else { panic!("unexpected gap") };
        ticket.wait().expect("ack");
    }

    #[test]
    fn block_policy_has_a_bounded_wait_and_queued_cancellation_is_visible() {
        let (origin, event, policy) = fixture();
        let gate = TestGate::new();
        let journal = Journal::in_memory(DeterministicKeyProvider::from_seed("writer-test-block"))
            .expect("journal");
        let writer = Writer::new_with_gate(
            journal.clone(),
            WriterConfig {
                queue_items: 1,
                max_wait: Duration::from_millis(10),
                default_queue_full_policy: QueueFullPolicy::Block,
                ..WriterConfig::default()
            },
            gate.clone(),
        )
        .expect("writer");
        let first = writer
            .enqueue(origin.clone(), vec![event.clone()], policy.clone(), Vec::new())
            .expect("first");
        gate.wait_until_entered();
        assert!(matches!(
            writer.enqueue(origin.clone(), vec![event.clone()], policy.clone(), Vec::new()),
            Err(GhostraceError::WriterQueueWaitTimeout { .. })
        ));
        gate.release();
        let WriterSubmission::Queued(ticket) = first else { panic!("unexpected gap") };
        ticket.wait().expect("ack");

        let gate = TestGate::new();
        let journal = Journal::in_memory(DeterministicKeyProvider::from_seed("writer-test-cancel"))
            .expect("journal");
        let writer = Writer::new_with_gate(
            journal.clone(),
            WriterConfig { queue_items: 2, ..WriterConfig::default() },
            gate.clone(),
        )
        .expect("writer");
        let first = writer
            .enqueue(origin.clone(), vec![event.clone()], policy.clone(), Vec::new())
            .expect("first");
        gate.wait_until_entered();
        let second = match writer.enqueue(origin, vec![event], policy, Vec::new()).expect("second")
        {
            WriterSubmission::Queued(ticket) => ticket,
            WriterSubmission::Gap(_) => panic!("unexpected gap"),
        };
        assert!(second.cancel());
        gate.release();
        let WriterSubmission::Queued(ticket) = first else { panic!("unexpected gap") };
        ticket.wait().expect("first ack");
        assert!(matches!(second.wait(), Err(GhostraceError::WriterCancelled)));
        assert_eq!(journal.events().expect("events").len(), 1);
    }
}
