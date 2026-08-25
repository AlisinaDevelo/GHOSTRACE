use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    },
};

use chrono::{TimeZone, Utc};
use ghostrace::{
    CursorIdentity, DeterministicKeyProvider, DiagnosticRecord, EventEnvelope, EventKind,
    EventPayload, EventSource, Evidence, FaultAction, FaultPlan, FaultPoint, FaultSchedule,
    GhostraceError, IngestionOrigin, Journal, KeyProvider, PolicyProfile, ReasonCode, SourceCursor,
};
use serde::Deserialize;
use tempfile::tempdir;
use uuid::Uuid;

const SCHEDULE_FIXTURE: &str = include_str!("fixtures/fault-schedules-v1.json");
const CHILD_ENV: &str = "GHOSTRACE_FAULT_MATRIX_CHILD";
const CHILD_PATH_ENV: &str = "GHOSTRACE_FAULT_MATRIX_PATH";
const CHILD_POINT_ENV: &str = "GHOSTRACE_FAULT_MATRIX_POINT";

#[derive(Debug, Deserialize)]
struct FaultMatrixFixture {
    version: u32,
    seed_count: u64,
    schedules: Vec<NamedSchedule>,
}

#[derive(Debug, Deserialize)]
struct NamedSchedule {
    name: String,
    seed: u64,
    #[serde(flatten)]
    schedule: FaultSchedule,
}

#[derive(Clone)]
struct GenerationKeyProvider {
    key: [u8; 32],
    generation: u32,
    reads: Arc<AtomicU32>,
}

impl GenerationKeyProvider {
    fn new(generation: u32) -> Self {
        Self {
            key: DeterministicKeyProvider::from_seed(&format!("fault-generation-{generation}"))
                .key()
                .expect("deterministic key"),
            generation,
            reads: Arc::new(AtomicU32::new(0)),
        }
    }

    fn reads(&self) -> u32 {
        self.reads.load(Ordering::SeqCst)
    }
}

impl KeyProvider for GenerationKeyProvider {
    fn key(&self) -> Result<[u8; 32], ghostrace::CryptoError> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        Ok(self.key)
    }
}

fn fixture() -> FaultMatrixFixture {
    serde_json::from_str(SCHEDULE_FIXTURE).expect("fault schedule fixture")
}

fn private_tempdir() -> tempfile::TempDir {
    let directory = tempdir().expect("tempdir");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
            .expect("private tempdir");
    }
    directory
}

fn policy() -> PolicyProfile {
    let mut profile = PolicyProfile::deny_by_default("fault-matrix-policy");
    profile.enable_source(EventSource::Filesystem);
    profile
}

fn event(id: u128, cursor: &str) -> EventEnvelope {
    let origin = IngestionOrigin::fixture_instance("fixture-fault-matrix-fs").expect("origin");
    let timestamp = Utc.timestamp_opt(1_735_689_600 + id as i64, 0).single().expect("timestamp");
    EventEnvelope::new(
        &origin,
        Uuid::from_u128(id),
        timestamp,
        timestamp,
        EventSource::Filesystem,
        EventKind::Gap,
        EventPayload::Gap(gap_payload(id)),
        Some(SourceCursor::try_from(cursor).expect("cursor")),
        "fault-matrix-policy",
        1,
        Evidence::Unknown,
        None,
    )
    .expect("event")
}

fn gap_payload(id: u128) -> ghostrace::GapPayload {
    ghostrace::GapPayload {
        source: EventSource::Filesystem,
        reason_code: ReasonCode::try_from("fault_injected").expect("reason"),
        dropped_count: id as u64,
        from_cursor: None,
        to_cursor: None,
        volume_digest: None,
        root_ids: Vec::new(),
        remediation: None,
    }
}

fn origin() -> IngestionOrigin {
    IngestionOrigin::fixture_instance("fixture-fault-matrix-fs").expect("origin")
}

fn identity() -> CursorIdentity {
    CursorIdentity::new(EventSource::Filesystem, "fixture-fault-matrix-fs").expect("identity")
}

fn plan(schedule: FaultSchedule) -> FaultPlan {
    FaultPlan::from_schedules(schedule.occurrence as u64, vec![schedule]).expect("plan")
}

fn assert_recovered(journal: &Journal, event: &EventEnvelope, provider: &GenerationKeyProvider) {
    let recovered = journal.clone().with_fault_plan(FaultPlan::none());
    let policy = policy();
    let sequence = recovered.ingest(&origin(), event, &policy).expect("retry after fault");
    assert_eq!(sequence, 1);
    let events = recovered.events().expect("events after retry");
    assert_eq!(events.len(), 1);
    assert_eq!(events.iter().filter(|stored| stored.event.kind == EventKind::Gap).count(), 1);
    let state = recovered
        .cursor_state(&identity())
        .expect("cursor state")
        .expect("cursor state after retry");
    assert_eq!(state.token.raw().as_str(), "seq-0-1");
    assert_eq!(state.last_event_id, Some(event.event_id.to_string()));
    assert_eq!(provider.generation, 7);
    assert!(provider.reads() >= 1, "retry must read the stable key generation");
}

#[test]
fn fixture_names_every_durable_fault_boundary_and_is_bounded() {
    let matrix = fixture();
    assert_eq!(matrix.version, 1);
    assert_eq!(matrix.seed_count, 32);
    assert_eq!(matrix.schedules.len(), FaultPoint::ALL.len());
    let mut names = matrix.schedules.iter().map(|case| case.name.as_str()).collect::<Vec<_>>();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), matrix.schedules.len());
    for case in &matrix.schedules {
        assert!(case.seed > 0 && case.seed <= matrix.seed_count);
        assert_eq!(case.schedule.occurrence, 1);
        assert_eq!(case.schedule.action, FaultAction::Return);
        assert!(FaultPoint::ALL.contains(&case.schedule.point));
    }
}

#[test]
fn return_fault_matrix_rolls_back_or_leaves_a_retryable_commit() {
    let matrix = fixture();
    let event = event(1, "seq-0-1");
    let policy = policy();

    for case in matrix.schedules {
        let provider = GenerationKeyProvider::new(7);
        let schedule = case.schedule;
        if matches!(
            schedule.point,
            FaultPoint::StorageBeforeOpen
                | FaultPoint::StorageAfterOpen
                | FaultPoint::StorageBeforeVerify
                | FaultPoint::StorageAfterVerify
                | FaultPoint::MigrationBeforeTransaction
                | FaultPoint::MigrationAfterSql
                | FaultPoint::MigrationBeforeCommit
                | FaultPoint::MigrationAfterCommit
        ) {
            let directory = private_tempdir();
            let path = directory.path().join(format!("{}.sqlite3", case.name));
            let result =
                Journal::open_fixture_with_fault_plan(&path, provider.clone(), plan(schedule));
            assert!(matches!(result, Err(GhostraceError::InjectedFault { .. })), "{}", case.name);
            let recovered = Journal::open_fixture(&path, provider.clone()).expect("reopen");
            assert_eq!(recovered.schema_version().expect("schema"), 4);
            assert_recovered(&recovered, &event, &provider);
            continue;
        }

        if matches!(schedule.point, FaultPoint::CheckpointBefore | FaultPoint::CheckpointAfter) {
            let directory = private_tempdir();
            let path = directory.path().join("checkpoint.sqlite3");
            let journal = Journal::open_fixture(&path, provider.clone())
                .expect("open checkpoint journal")
                .with_fault_plan(plan(schedule));
            let error = journal
                .checkpoint(ghostrace::CheckpointMode::Truncate)
                .expect_err("checkpoint fault");
            assert!(matches!(error, GhostraceError::InjectedFault { .. }));
            let recovered = Journal::open_fixture(&path, provider.clone()).expect("reopen");
            assert!(recovered.shutdown().expect("recovery checkpoint").within_policy());
            assert_recovered(&recovered, &event, &provider);
            continue;
        }

        if matches!(schedule.point, FaultPoint::BackupBeforeCopy | FaultPoint::BackupAfterCopy) {
            let directory = private_tempdir();
            let path = directory.path().join("backup-source.sqlite3");
            let destination = directory.path().join("backup.sqlite3");
            let journal = Journal::open_fixture(&path, provider.clone())
                .expect("open backup journal")
                .with_fault_plan(plan(schedule));
            let error = journal.backup_snapshot(&destination).expect_err("backup fault");
            assert!(matches!(error, GhostraceError::InjectedFault { .. }));
            let recovered = Journal::open_fixture(&path, provider.clone()).expect("reopen");
            if destination.exists() {
                let snapshot =
                    Journal::open_fixture(&destination, provider.clone()).expect("snapshot");
                assert_eq!(snapshot.events().expect("snapshot events").len(), 0);
            }
            assert_recovered(&recovered, &event, &provider);
            continue;
        }

        let journal =
            Journal::in_memory(provider.clone()).expect("journal").with_fault_plan(plan(schedule));
        let diagnostics = if matches!(
            schedule.point,
            FaultPoint::DiagnosticBeforeInsert | FaultPoint::DiagnosticAfterInsert
        ) {
            vec![DiagnosticRecord::new("fault.matrix", "bounded test diagnostic")
                .expect("diagnostic")]
        } else {
            Vec::new()
        };
        if matches!(
            schedule.point,
            FaultPoint::ControlBeforeTransaction
                | FaultPoint::ControlAfterTransaction
                | FaultPoint::ControlBeforeCommit
                | FaultPoint::ControlAfterCommit
        ) {
            journal.ingest(&origin(), &event, &policy).expect("seed control state");
            let reset = SourceCursor::try_from("reset-1-0").expect("reset cursor");
            let error =
                journal.reset_cursor(&identity(), &reset, &policy).expect_err("control fault");
            assert!(matches!(error, GhostraceError::InjectedFault { .. }));
            let recovered = journal.clone().with_fault_plan(FaultPlan::none());
            if schedule.point == FaultPoint::ControlAfterCommit {
                assert_eq!(
                    recovered.cursor_state(&identity()).expect("state").expect("state").status,
                    ghostrace::CursorStatus::Reset
                );
            } else {
                assert_eq!(
                    recovered.cursor_state(&identity()).expect("state").expect("state").status,
                    ghostrace::CursorStatus::Active
                );
            }
            assert_eq!(recovered.events().expect("events").len(), 1);
            continue;
        }
        let error = journal
            .ingest_batch_with_diagnostics(&origin(), &[event.clone()], &policy, &diagnostics)
            .expect_err("fault must return");
        assert!(matches!(error, GhostraceError::InjectedFault { .. }), "{}", case.name);
        if schedule.point == FaultPoint::IngestAfterCommit {
            assert_eq!(journal.events().expect("committed events").len(), 1);
            assert_eq!(journal.diagnostic_count().expect("committed diagnostics"), 0);
            assert_eq!(journal.ingest(&origin(), &event, &policy).expect("idempotent retry"), 1);
        } else {
            assert_eq!(journal.events().expect("rolled-back events").len(), 0);
            assert_eq!(journal.diagnostic_count().expect("rolled-back diagnostics"), 0);
            assert_recovered(&journal, &event, &provider);
        }
    }
}

#[test]
fn bounded_seed_matrix_replays_minimized_schedules() {
    let matrix = fixture();
    let points = [
        FaultPoint::IngestBeforeTransaction,
        FaultPoint::KeyBeforeAccess,
        FaultPoint::EventBeforeInsert,
        FaultPoint::EventAfterInsert,
        FaultPoint::CursorBeforeUpdate,
        FaultPoint::IngestBeforeCommit,
        FaultPoint::IngestAfterCommit,
    ];
    for seed in 0..matrix.seed_count {
        let point = points[(seed as usize) % points.len()];
        let provider = GenerationKeyProvider::new(7);
        let journal = Journal::in_memory(provider.clone()).expect("journal").with_fault_plan(
            FaultPlan::from_schedules(
                seed,
                vec![FaultSchedule { point, occurrence: 1, action: FaultAction::Return }],
            )
            .expect("bounded plan"),
        );
        assert!(matches!(
            journal.ingest(&origin(), &event(100 + seed as u128, "seq-0-1"), &policy()),
            Err(GhostraceError::InjectedFault { .. })
        ));
        let recovered = journal.with_fault_plan(FaultPlan::none());
        let expected_events = usize::from(point == FaultPoint::IngestAfterCommit);
        assert_eq!(recovered.events().expect("events").len(), expected_events);
        recovered
            .ingest(&origin(), &event(100 + seed as u128, "seq-0-1"), &policy())
            .expect("retry");
        assert_eq!(recovered.events().expect("events").len(), 1);
        assert_eq!(provider.generation, 7);
        assert!(provider.reads() >= 1);
    }
    println!("FAULT_MATRIX_SEEDS {}", matrix.seed_count);
}

#[test]
fn abrupt_faults_recover_after_restart() {
    if std::env::var_os(CHILD_ENV).is_some() {
        run_abort_child();
        unreachable!("fault child must abort");
    }

    for (point, expected_events) in [
        (FaultPoint::MigrationAfterSql, 0usize),
        (FaultPoint::KeyAfterAccess, 0usize),
        (FaultPoint::EventAfterInsert, 0usize),
        (FaultPoint::IngestBeforeCommit, 0usize),
        (FaultPoint::IngestAfterCommit, 1usize),
    ] {
        let directory = private_tempdir();
        let path = directory.path().join(format!("abort-{point}.sqlite3"));
        let status = Command::new(std::env::current_exe().expect("test executable"))
            .arg("--exact")
            .arg("abrupt_faults_recover_after_restart")
            .arg("--nocapture")
            .env(CHILD_ENV, "1")
            .env(CHILD_PATH_ENV, &path)
            .env(CHILD_POINT_ENV, point.as_str())
            .status()
            .expect("spawn fault child");
        assert!(!status.success(), "fault child must abort at {point}");

        let provider = DeterministicKeyProvider::from_seed("fault-child");
        let recovered = Journal::open_fixture(&path, provider).expect("reopen after abort");
        assert_eq!(recovered.events().expect("events").len(), expected_events, "{point}");
        if expected_events == 0 {
            recovered
                .ingest(&origin(), &event(1, "seq-0-1"), &policy())
                .expect("retry after abort");
            assert_eq!(recovered.events().expect("retried event").len(), 1);
        } else {
            assert_eq!(
                recovered
                    .ingest(&origin(), &event(1, "seq-0-1"), &policy())
                    .expect("idempotent retry"),
                1
            );
        }
        assert!(recovered.shutdown().expect("shutdown").within_policy());
    }
}

fn run_abort_child() {
    let path = PathBuf::from(std::env::var(CHILD_PATH_ENV).expect("child path"));
    let point_name = std::env::var(CHILD_POINT_ENV).expect("child point");
    let point = FaultPoint::ALL
        .into_iter()
        .find(|candidate| candidate.as_str() == point_name)
        .expect("known fault point");
    let plan = FaultPlan::abort_once(point);
    if point == FaultPoint::MigrationAfterSql {
        let _ = Journal::open_fixture_with_fault_plan(
            &path,
            DeterministicKeyProvider::from_seed("fault-child"),
            plan,
        );
    } else {
        let journal =
            Journal::open_fixture(&path, DeterministicKeyProvider::from_seed("fault-child"))
                .expect("child open")
                .with_fault_plan(plan);
        let _ = journal.ingest(&origin(), &event(1, "seq-0-1"), &policy());
    }
    std::process::abort();
}
