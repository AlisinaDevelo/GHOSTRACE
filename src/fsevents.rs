//! A bounded, owner-thread FSEvents stream lifecycle adapter.
//!
//! The adapter deliberately stops at the native stream boundary.  It does not
//! canonicalize selected roots, make a consent decision, or write to the
//! journal; those are separate collector gates. Event flags can be normalized
//! through [`FseventsEvent::normalize_flags`]. A caller must
//! schedule and drive the stream on one run-loop thread, and must keep the
//! returned value on that thread until it is dropped.
//!
//! On macOS the implementation uses the C FSEvents API directly.  The callback
//! context is one `Box<CallbackState>` owned by this wrapper.  The context has
//! no Core Foundation retain/release callbacks: `FSEventStreamInvalidate` is
//! called before `FSEventStreamRelease`, and only then is the box reclaimed.
//! This ordering prevents a native callback from observing freed Rust state.

use std::{
    marker::PhantomData,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    thread::{self, ThreadId},
    time::Duration,
};

#[cfg(target_os = "macos")]
use std::sync::Mutex;

use thiserror::Error;

use crate::{
    cursor::{CursorKind, CursorStreamMode, CursorToken, ReplayConfiguration},
    error::GhostraceError,
    model::{SnapshotDigest, SourceCursor},
};

#[cfg(target_os = "macos")]
use std::{
    ffi::{c_char, c_void, CStr, CString, OsStr},
    os::unix::ffi::OsStrExt,
    panic::{catch_unwind, AssertUnwindSafe},
};

/// FSEventStream's sentinel for "start at the current point in history".
pub const EVENT_ID_SINCE_NOW: u64 = u64::MAX;

/// Why a persisted FSEvents startup position cannot be used as a native
/// `sinceWhen` value. These refusals are explicit so a caller cannot silently
/// fall back to a live stream and imply that the missing history was covered.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum StartupCursorRejection {
    #[error("zero is not a bounded history boundary")]
    Zero,
    #[error("the requested history is older than the available source window")]
    Stale,
    #[error("the requested history is newer than the available source window")]
    Future,
    #[error("the source event ID counter wrapped; a new stream epoch is required")]
    Wrapped,
    #[error("the persisted cursor is not a valid ordered FSEvents event ID")]
    Corrupted,
}

/// Errors raised while classifying a startup cursor before native stream
/// creation. No source or path value is included in the error text.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum StartupCursorError {
    #[error("startup cursor refused: {0}")]
    Refused(StartupCursorRejection),
    #[error("history cursor range is invalid")]
    InvalidRange,
}

/// A bounded source-history window supplied by a platform probe or a durable
/// source receipt. Event IDs are global for one FSEvents stream identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HistoryCursorRange {
    pub oldest_event_id: u64,
    pub newest_event_id: u64,
}

impl HistoryCursorRange {
    pub fn new(oldest_event_id: u64, newest_event_id: u64) -> Result<Self, StartupCursorError> {
        if oldest_event_id == 0 || newest_event_id < oldest_event_id {
            return Err(StartupCursorError::InvalidRange);
        }
        Ok(Self { oldest_event_id, newest_event_id })
    }
}

/// The only startup positions accepted by the native FSEvents adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StartupCursor {
    SinceNow,
    EventId { event_id: u64 },
}

impl StartupCursor {
    pub fn from_since_when(since_when: u64) -> Result<Self, StartupCursorError> {
        if since_when == EVENT_ID_SINCE_NOW {
            return Ok(Self::SinceNow);
        }
        Self::from_event_id(since_when)
    }

    pub fn from_source_cursor(cursor: &SourceCursor) -> Result<Self, StartupCursorError> {
        let token = CursorToken::new(cursor.clone());
        match token.kind() {
            CursorKind::Sequence => {
                let Some(position) = token.position() else {
                    return Err(StartupCursorError::Refused(StartupCursorRejection::Corrupted));
                };
                let Ok(event_id) = u64::try_from(position) else {
                    return Err(StartupCursorError::Refused(StartupCursorRejection::Corrupted));
                };
                Self::from_event_id(event_id)
            }
            CursorKind::Wrap => Err(StartupCursorError::Refused(StartupCursorRejection::Wrapped)),
            CursorKind::Opaque | CursorKind::Reset => {
                Err(StartupCursorError::Refused(StartupCursorRejection::Corrupted))
            }
        }
    }

    pub fn decision(
        self,
        available: Option<HistoryCursorRange>,
    ) -> Result<StartupCursorDecision, StartupCursorError> {
        match self {
            Self::SinceNow => Ok(StartupCursorDecision::SinceNow),
            Self::EventId { event_id } => {
                if let Some(range) = available {
                    if event_id < range.oldest_event_id {
                        return Err(StartupCursorError::Refused(StartupCursorRejection::Stale));
                    }
                    if event_id > range.newest_event_id {
                        return Err(StartupCursorError::Refused(StartupCursorRejection::Future));
                    }
                }
                Ok(StartupCursorDecision::Replay { event_id })
            }
        }
    }

    fn from_event_id(event_id: u64) -> Result<Self, StartupCursorError> {
        if event_id == 0 {
            return Err(StartupCursorError::Refused(StartupCursorRejection::Zero));
        }
        if event_id == EVENT_ID_SINCE_NOW {
            return Err(StartupCursorError::Refused(StartupCursorRejection::Corrupted));
        }
        Ok(Self::EventId { event_id })
    }
}

/// Startup behavior after the cursor has passed validation and any available
/// history-window check.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum StartupCursorDecision {
    SinceNow,
    Replay { event_id: u64 },
}

/// The default stream latency.  A short latency is useful for the lifecycle
/// integration tests while still allowing the daemon to coalesce callbacks.
pub const DEFAULT_LATENCY: Duration = Duration::from_millis(250);

/// The adapter's bounded input and callback limits.
pub const MAX_WATCH_PATHS: usize = 64;
pub const MAX_PATH_BYTES: usize = 4 * 1024;
pub const MAX_CALLBACK_EVENTS: usize = 4 * 1024;
pub const MAX_CALLBACK_PATH_BYTES: usize = 4 * 1024;

/// FSEvents create flags used by the fixture and integration contracts.
pub const FLAG_NO_DEFER: u32 = 0x0000_0002;
/// Request a callback when a selected watch root is renamed or removed.
pub const FLAG_WATCH_ROOT: u32 = 0x0000_0004;
pub const FLAG_FILE_EVENTS: u32 = 0x0000_0010;
/// Callback representation flags intentionally rejected by this raw-path adapter.
///
/// The callback parser below expects the documented `char *` path array.  These
/// modes ask Core Services for CFType/extended-data representations instead,
/// so accepting them would make the pointer interpretation unsafe.
pub const FLAG_USE_CF_TYPES: u32 = 0x0000_0001;
pub const FLAG_USE_EXTENDED_DATA: u32 = 0x0000_0040;
pub const FLAG_FULL_HISTORY: u32 = 0x0000_0080;
pub const FLAG_WITH_DOC_ID: u32 = 0x0000_0100;

/// Errors returned before or during stream lifecycle operations.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum FseventsError {
    #[error("FSEvents stream support requires macOS")]
    UnsupportedPlatform,

    #[error("at least one FSEvents watch path is required")]
    EmptyPaths,

    #[error("FSEvents watch path count exceeds the bound of {MAX_WATCH_PATHS}")]
    TooManyPaths,

    #[error("FSEvents watch path contains a NUL byte")]
    PathContainsNul,

    #[error("FSEvents watch path exceeds the bound of {MAX_PATH_BYTES} bytes")]
    PathTooLong,

    #[error("FSEvents latency must be finite and no greater than one hour")]
    InvalidLatency,

    #[error("FSEvents callback representation flags are unsupported by the raw-path adapter")]
    UnsupportedCallbackFlags,

    #[error("FSEvents startup cursor is invalid: {reason}")]
    InvalidStartupCursor { reason: StartupCursorRejection },

    #[error("the FSEvents stream is not scheduled on a run loop")]
    NotScheduled,

    #[error("the FSEvents stream is already scheduled")]
    AlreadyScheduled,

    #[error("the FSEvents stream is already running")]
    AlreadyRunning,

    #[error("the FSEvents stream is not running")]
    NotRunning,

    #[error("the FSEvents stream has been invalidated")]
    Invalidated,

    #[error("FSEvents lifecycle methods must run on the owner thread")]
    WrongThread,

    #[error("FSEventStreamCreate returned NULL")]
    NativeCreateFailed,

    #[error("FSEventStreamStart returned false")]
    NativeStartFailed,

    #[error("a Core Foundation path value could not be created")]
    NativePathFailed,
}

/// The observable lifecycle state of a stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StreamState {
    /// Native object exists but has not been scheduled.
    Created,
    /// Native object is scheduled, but not receiving events.
    Scheduled,
    /// Native object is receiving events.
    Running,
    /// Native object was stopped and remains restartable.
    Stopped,
    /// Native object has been invalidated and cannot be restarted.
    Invalidated,
}

/// Configuration passed to `FSEventStreamCreate`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FseventsOptions {
    pub since_when: u64,
    pub latency: Duration,
    pub flags: u32,
    /// Stream scope must be carried into cursor identity by the caller.
    pub stream_mode: CursorStreamMode,
}

impl Default for FseventsOptions {
    fn default() -> Self {
        Self {
            since_when: EVENT_ID_SINCE_NOW,
            latency: DEFAULT_LATENCY,
            flags: FLAG_FILE_EVENTS | FLAG_NO_DEFER | FLAG_WATCH_ROOT,
            stream_mode: CursorStreamMode::PerHost,
        }
    }
}

impl FseventsOptions {
    fn validate(&self) -> Result<(), FseventsError> {
        StartupCursor::from_since_when(self.since_when).map_err(|error| {
            FseventsError::InvalidStartupCursor {
                reason: match error {
                    StartupCursorError::Refused(reason) => reason,
                    StartupCursorError::InvalidRange => StartupCursorRejection::Corrupted,
                },
            }
        })?;
        let seconds = self.latency.as_secs_f64();
        if !seconds.is_finite() || seconds > 60.0 * 60.0 {
            return Err(FseventsError::InvalidLatency);
        }
        if self.flags
            & (FLAG_USE_CF_TYPES | FLAG_USE_EXTENDED_DATA | FLAG_FULL_HISTORY | FLAG_WITH_DOC_ID)
            != 0
        {
            return Err(FseventsError::UnsupportedCallbackFlags);
        }
        Ok(())
    }

    /// Classify the configured `sinceWhen` value before stream creation. A
    /// missing range means that the source has not yet supplied an availability
    /// probe; the explicit nonzero event ID remains a replay request and any
    /// later source-loss signal must become a gap.
    pub fn startup_decision(
        &self,
        available: Option<HistoryCursorRange>,
    ) -> Result<StartupCursorDecision, StartupCursorError> {
        StartupCursor::from_since_when(self.since_when)?.decision(available)
    }

    /// Convert stream settings into the path-free durable replay contract.
    pub fn replay_configuration(
        &self,
        root_scope_digest: SnapshotDigest,
        exclusions_digest: SnapshotDigest,
    ) -> Result<ReplayConfiguration, GhostraceError> {
        self.validate().map_err(|error| GhostraceError::InvalidEvent(error.to_string()))?;
        ReplayConfiguration::new(
            root_scope_digest,
            exclusions_digest,
            self.since_when,
            self.latency,
            self.flags & FLAG_FILE_EVENTS != 0,
        )
    }
}

/// A bounded event delivered by an FSEvents callback.
///
/// Paths are metadata only.  The adapter never opens them or reads their
/// contents. The raw `flags` value is retained alongside the canonical,
/// path-free result returned by [`FseventsEvent::normalize_flags`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FseventsEvent {
    pub path: PathBuf,
    pub event_id: u64,
    pub flags: u32,
}

impl FseventsEvent {
    /// Normalize the raw Core Services flag word without retaining its path.
    pub fn normalize_flags(&self) -> crate::fsevents_flags::NormalizedFseventsEvent {
        crate::fsevents_flags::normalize_fsevents_event(self.event_id, self.flags)
    }
}

/// Callback health counters, useful for a collector's lifecycle receipt.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CallbackHealth {
    pub delivered_batches: u64,
    pub delivered_events: u64,
    pub malformed_batches: u64,
    pub panics: u64,
}

#[cfg(target_os = "macos")]
type EventCallback = Box<dyn FnMut(&[FseventsEvent]) + Send + 'static>;

struct CallbackState {
    #[cfg(target_os = "macos")]
    callback: Mutex<EventCallback>,
    delivered_batches: AtomicU64,
    delivered_events: AtomicU64,
    malformed_batches: AtomicU64,
    panics: AtomicU64,
}

impl CallbackState {
    #[cfg(target_os = "macos")]
    fn new<F>(callback: F) -> Self
    where
        F: FnMut(&[FseventsEvent]) + Send + 'static,
    {
        Self {
            callback: Mutex::new(Box::new(callback)),
            delivered_batches: AtomicU64::new(0),
            delivered_events: AtomicU64::new(0),
            malformed_batches: AtomicU64::new(0),
            panics: AtomicU64::new(0),
        }
    }

    fn health(&self) -> CallbackHealth {
        CallbackHealth {
            delivered_batches: self.delivered_batches.load(Ordering::Relaxed),
            delivered_events: self.delivered_events.load(Ordering::Relaxed),
            malformed_batches: self.malformed_batches.load(Ordering::Relaxed),
            panics: self.panics.load(Ordering::Relaxed),
        }
    }

    #[cfg(target_os = "macos")]
    fn dispatch_raw(
        &self,
        num_events: usize,
        event_paths: *mut c_void,
        event_flags: *const u32,
        event_ids: *const u64,
    ) {
        if num_events == 0 {
            return;
        }
        if num_events > MAX_CALLBACK_EVENTS
            || event_paths.is_null()
            || event_flags.is_null()
            || event_ids.is_null()
        {
            self.malformed_batches.fetch_add(1, Ordering::Relaxed);
            return;
        }

        let paths = event_paths.cast::<*const c_char>();
        let mut events = Vec::with_capacity(num_events);
        for index in 0..num_events {
            // FSEvents owns these pointers only for the duration of this
            // callback.  Copy each bounded path before returning to C.
            let path_pointer = unsafe { *paths.add(index) };
            if path_pointer.is_null() {
                self.malformed_batches.fetch_add(1, Ordering::Relaxed);
                return;
            }
            let path_bytes = unsafe { CStr::from_ptr(path_pointer).to_bytes() };
            if path_bytes.len() > MAX_CALLBACK_PATH_BYTES {
                self.malformed_batches.fetch_add(1, Ordering::Relaxed);
                return;
            }
            let path = PathBuf::from(OsStr::from_bytes(path_bytes));
            let flags = unsafe { *event_flags.add(index) };
            let event_id = unsafe { *event_ids.add(index) };
            events.push(FseventsEvent { path, event_id, flags });
        }

        let callback_result = catch_unwind(AssertUnwindSafe(|| {
            let mut callback =
                self.callback.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            callback(&events);
        }));
        if callback_result.is_err() {
            // Never unwind through the C ABI.  A collector can inspect this
            // counter and turn the condition into a bounded lifecycle error.
            self.panics.fetch_add(1, Ordering::Relaxed);
            return;
        }
        self.delivered_batches.fetch_add(1, Ordering::Relaxed);
        self.delivered_events.fetch_add(events.len() as u64, Ordering::Relaxed);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NativeError {
    #[cfg(any(target_os = "macos", test))]
    Schedule,
    #[cfg(any(target_os = "macos", test))]
    Start,
    #[cfg(not(target_os = "macos"))]
    Unsupported,
}

trait NativeLifecycle {
    fn schedule(&mut self) -> Result<(), NativeError>;
    fn start(&mut self) -> Result<(), NativeError>;
    fn stop(&mut self);
    fn flush(&mut self);
    fn invalidate(&mut self);
    fn release(self);
}

fn map_native_error(error: NativeError) -> FseventsError {
    match error {
        #[cfg(any(target_os = "macos", test))]
        NativeError::Schedule => FseventsError::NativeCreateFailed,
        #[cfg(any(target_os = "macos", test))]
        NativeError::Start => FseventsError::NativeStartFailed,
        #[cfg(not(target_os = "macos"))]
        NativeError::Unsupported => FseventsError::UnsupportedPlatform,
    }
}

/// The state machine is deliberately separate from the platform backend.  Its
/// mock backend tests prove exactly-once stop/invalidate/release behavior even
/// when a native schedule or start operation fails part-way through setup.
struct LifecycleController<N: NativeLifecycle> {
    native: Option<N>,
    state: StreamState,
}

impl<N: NativeLifecycle> LifecycleController<N> {
    #[cfg(any(target_os = "macos", test))]
    fn new(native: N) -> Self {
        Self { native: Some(native), state: StreamState::Created }
    }

    fn state(&self) -> StreamState {
        self.state
    }

    fn native_mut(&mut self) -> Result<&mut N, FseventsError> {
        self.native.as_mut().ok_or(FseventsError::Invalidated)
    }

    fn schedule(&mut self) -> Result<(), FseventsError> {
        match self.state {
            StreamState::Created => {
                self.native_mut()?.schedule().map_err(map_native_error)?;
                self.state = StreamState::Scheduled;
                Ok(())
            }
            StreamState::Scheduled | StreamState::Running | StreamState::Stopped => {
                Err(FseventsError::AlreadyScheduled)
            }
            StreamState::Invalidated => Err(FseventsError::Invalidated),
        }
    }

    fn start(&mut self) -> Result<(), FseventsError> {
        match self.state {
            StreamState::Scheduled | StreamState::Stopped => {
                self.native_mut()?.start().map_err(map_native_error)?;
                self.state = StreamState::Running;
                Ok(())
            }
            StreamState::Created => Err(FseventsError::NotScheduled),
            StreamState::Running => Err(FseventsError::AlreadyRunning),
            StreamState::Invalidated => Err(FseventsError::Invalidated),
        }
    }

    fn stop(&mut self) -> Result<(), FseventsError> {
        match self.state {
            StreamState::Running => {
                self.native_mut()?.stop();
                self.state = StreamState::Stopped;
                Ok(())
            }
            StreamState::Created | StreamState::Scheduled => Err(FseventsError::NotRunning),
            StreamState::Stopped => Err(FseventsError::NotRunning),
            StreamState::Invalidated => Err(FseventsError::Invalidated),
        }
    }

    fn restart(&mut self) -> Result<(), FseventsError> {
        if self.state == StreamState::Running {
            self.stop()?;
        }
        self.start()
    }

    fn flush(&mut self) -> Result<(), FseventsError> {
        if self.state != StreamState::Running {
            return Err(match self.state {
                StreamState::Created => FseventsError::NotScheduled,
                StreamState::Scheduled | StreamState::Stopped => FseventsError::NotRunning,
                StreamState::Invalidated => FseventsError::Invalidated,
                StreamState::Running => unreachable!(),
            });
        }
        self.native_mut()?.flush();
        Ok(())
    }

    fn invalidate(&mut self) -> Result<(), FseventsError> {
        match self.state {
            StreamState::Created => Err(FseventsError::NotScheduled),
            StreamState::Scheduled | StreamState::Stopped | StreamState::Running => {
                if self.state == StreamState::Running {
                    self.native_mut()?.stop();
                    self.state = StreamState::Stopped;
                }
                self.native_mut()?.invalidate();
                self.state = StreamState::Invalidated;
                Ok(())
            }
            // Invalidation is intentionally idempotent.  The native call is
            // still made exactly once because the state transitions terminally.
            StreamState::Invalidated => Ok(()),
        }
    }

    fn shutdown(&mut self) {
        let Some(mut native) = self.native.take() else {
            return;
        };
        match self.state {
            StreamState::Running => {
                native.stop();
                native.invalidate();
            }
            StreamState::Scheduled | StreamState::Stopped => native.invalidate(),
            StreamState::Created | StreamState::Invalidated => {}
        }
        native.release();
        self.state = StreamState::Invalidated;
    }
}

impl<N: NativeLifecycle> Drop for LifecycleController<N> {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(target_os = "macos")]
mod ffi {
    use super::{c_char, c_void};

    pub type Boolean = u8;
    pub type CFIndex = isize;
    pub type CFTimeInterval = f64;
    pub type CFAllocatorRef = *const c_void;
    pub type CFArrayRef = *const c_void;
    pub type CFStringRef = *const c_void;
    pub type CFRunLoopRef = *const c_void;
    pub type FSEventStreamRef = *mut c_void;
    pub type ConstFSEventStreamRef = *const c_void;
    pub type FSEventStreamEventId = u64;
    pub type FSEventStreamEventFlags = u32;
    pub type FSEventStreamCreateFlags = u32;
    pub type FSEventStreamCallback = unsafe extern "C" fn(
        ConstFSEventStreamRef,
        *mut c_void,
        usize,
        *mut c_void,
        *const FSEventStreamEventFlags,
        *const FSEventStreamEventId,
    );

    pub type CFArrayRetainCallBack =
        unsafe extern "C" fn(CFAllocatorRef, *const c_void) -> *const c_void;
    pub type CFArrayReleaseCallBack = unsafe extern "C" fn(CFAllocatorRef, *const c_void);
    pub type CFArrayCopyDescriptionCallBack = unsafe extern "C" fn(*const c_void) -> CFStringRef;
    pub type CFArrayEqualCallBack = unsafe extern "C" fn(*const c_void, *const c_void) -> Boolean;

    #[repr(C)]
    pub struct CFArrayCallBacks {
        pub version: CFIndex,
        pub retain: Option<CFArrayRetainCallBack>,
        pub release: Option<CFArrayReleaseCallBack>,
        pub copy_description: Option<CFArrayCopyDescriptionCallBack>,
        pub equal: Option<CFArrayEqualCallBack>,
    }

    pub type CFAllocatorRetainCallBack = unsafe extern "C" fn(*const c_void) -> *const c_void;
    pub type CFAllocatorReleaseCallBack = unsafe extern "C" fn(*const c_void);
    pub type CFAllocatorCopyDescriptionCallBack =
        unsafe extern "C" fn(*const c_void) -> CFStringRef;

    #[repr(C)]
    pub struct FSEventStreamContext {
        pub version: CFIndex,
        pub info: *mut c_void,
        pub retain: Option<CFAllocatorRetainCallBack>,
        pub release: Option<CFAllocatorReleaseCallBack>,
        pub copy_description: Option<CFAllocatorCopyDescriptionCallBack>,
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        pub static kCFTypeArrayCallBacks: CFArrayCallBacks;
        pub static kCFRunLoopDefaultMode: CFStringRef;
        pub fn CFArrayCreate(
            allocator: CFAllocatorRef,
            values: *const *const c_void,
            num_values: CFIndex,
            callbacks: *const CFArrayCallBacks,
        ) -> CFArrayRef;
        pub fn CFStringCreateWithFileSystemRepresentation(
            allocator: CFAllocatorRef,
            buffer: *const c_char,
        ) -> CFStringRef;
        pub fn CFRelease(value: *const c_void);
        pub fn CFRunLoopGetCurrent() -> CFRunLoopRef;
        pub fn CFRunLoopRunInMode(
            mode: CFStringRef,
            seconds: CFTimeInterval,
            return_after_source_handled: Boolean,
        ) -> i32;
    }

    #[link(name = "CoreServices", kind = "framework")]
    unsafe extern "C" {
        pub fn FSEventStreamCreate(
            allocator: CFAllocatorRef,
            callback: FSEventStreamCallback,
            context: *mut FSEventStreamContext,
            paths_to_watch: CFArrayRef,
            since_when: FSEventStreamEventId,
            latency: CFTimeInterval,
            flags: FSEventStreamCreateFlags,
        ) -> FSEventStreamRef;
        pub fn FSEventStreamScheduleWithRunLoop(
            stream: FSEventStreamRef,
            run_loop: CFRunLoopRef,
            mode: CFStringRef,
        );
        pub fn FSEventStreamStart(stream: FSEventStreamRef) -> Boolean;
        pub fn FSEventStreamFlushSync(stream: FSEventStreamRef);
        pub fn FSEventStreamStop(stream: FSEventStreamRef);
        pub fn FSEventStreamInvalidate(stream: FSEventStreamRef);
        pub fn FSEventStreamRelease(stream: FSEventStreamRef);
    }
}

#[cfg(target_os = "macos")]
struct NativeStream {
    raw: Option<ffi::FSEventStreamRef>,
}

#[cfg(target_os = "macos")]
impl NativeStream {
    fn create(
        paths: &[PathBuf],
        options: &FseventsOptions,
        context: *mut c_void,
    ) -> Result<Self, FseventsError> {
        let path_array = create_path_array(paths)?;
        let mut stream_context = ffi::FSEventStreamContext {
            version: 0,
            info: context,
            retain: None,
            release: None,
            copy_description: None,
        };
        let raw = unsafe {
            ffi::FSEventStreamCreate(
                std::ptr::null(),
                callback_trampoline,
                &mut stream_context,
                path_array,
                options.since_when,
                options.latency.as_secs_f64(),
                options.flags,
            )
        };
        unsafe { ffi::CFRelease(path_array) };
        if raw.is_null() {
            return Err(FseventsError::NativeCreateFailed);
        }
        Ok(Self { raw: Some(raw) })
    }

    fn raw(&self) -> ffi::FSEventStreamRef {
        self.raw.expect("native stream must be live until release")
    }
}

#[cfg(target_os = "macos")]
impl NativeLifecycle for NativeStream {
    fn schedule(&mut self) -> Result<(), NativeError> {
        let run_loop = unsafe { ffi::CFRunLoopGetCurrent() };
        if run_loop.is_null() {
            return Err(NativeError::Schedule);
        }
        unsafe {
            ffi::FSEventStreamScheduleWithRunLoop(self.raw(), run_loop, ffi::kCFRunLoopDefaultMode);
        }
        Ok(())
    }

    fn start(&mut self) -> Result<(), NativeError> {
        let started = unsafe { ffi::FSEventStreamStart(self.raw()) };
        if started == 0 {
            Err(NativeError::Start)
        } else {
            Ok(())
        }
    }

    fn stop(&mut self) {
        unsafe { ffi::FSEventStreamStop(self.raw()) };
    }

    fn flush(&mut self) {
        unsafe { ffi::FSEventStreamFlushSync(self.raw()) };
    }

    fn invalidate(&mut self) {
        unsafe { ffi::FSEventStreamInvalidate(self.raw()) };
    }

    fn release(mut self) {
        if let Some(raw) = self.raw.take() {
            unsafe { ffi::FSEventStreamRelease(raw) };
        }
    }
}

#[cfg(not(target_os = "macos"))]
struct NativeStream;

#[cfg(not(target_os = "macos"))]
impl NativeLifecycle for NativeStream {
    fn schedule(&mut self) -> Result<(), NativeError> {
        Err(NativeError::Unsupported)
    }

    fn start(&mut self) -> Result<(), NativeError> {
        Err(NativeError::Unsupported)
    }

    fn stop(&mut self) {}

    fn flush(&mut self) {}

    fn invalidate(&mut self) {}

    fn release(self) {}
}

#[cfg(target_os = "macos")]
fn create_path_array(paths: &[PathBuf]) -> Result<ffi::CFArrayRef, FseventsError> {
    let mut strings = Vec::with_capacity(paths.len());
    for path in paths {
        let bytes = path.as_os_str().as_bytes();
        let c_string = CString::new(bytes).map_err(|_| FseventsError::PathContainsNul)?;
        let value = unsafe {
            ffi::CFStringCreateWithFileSystemRepresentation(std::ptr::null(), c_string.as_ptr())
        };
        if value.is_null() {
            for string in strings.drain(..) {
                unsafe { ffi::CFRelease(string) };
            }
            return Err(FseventsError::NativePathFailed);
        }
        strings.push(value);
    }
    let values = strings.to_vec();
    let array = unsafe {
        ffi::CFArrayCreate(
            std::ptr::null(),
            values.as_ptr(),
            values.len() as ffi::CFIndex,
            &ffi::kCFTypeArrayCallBacks,
        )
    };
    for string in strings {
        unsafe { ffi::CFRelease(string) };
    }
    if array.is_null() {
        return Err(FseventsError::NativePathFailed);
    }
    Ok(array)
}

#[cfg(target_os = "macos")]
unsafe extern "C" fn callback_trampoline(
    _stream: ffi::ConstFSEventStreamRef,
    client_callback_info: *mut c_void,
    num_events: usize,
    event_paths: *mut c_void,
    event_flags: *const ffi::FSEventStreamEventFlags,
    event_ids: *const ffi::FSEventStreamEventId,
) {
    let Some(callback_state) = (client_callback_info as *const CallbackState).as_ref() else {
        return;
    };
    callback_state.dispatch_raw(num_events, event_paths, event_flags, event_ids);
}

/// A single-threaded, RAII-owned FSEventStream wrapper.
///
/// The stream is intentionally `!Send`/`!Sync`: scheduling, starting, flushing,
/// stopping, restarting, invalidating, and dropping it must happen on the
/// thread that created and scheduled it.  `Drop` stops a running stream,
/// invalidates any scheduled stream, releases the native object once, and only
/// then drops the callback context.
pub struct FseventsStream {
    lifecycle: LifecycleController<NativeStream>,
    callback_state: Box<CallbackState>,
    owner_thread: ThreadId,
    _not_send_or_sync: PhantomData<std::rc::Rc<()>>,
}

impl FseventsStream {
    /// Create a stream.  Creation does not start observation; callers must
    /// explicitly schedule it on the owner thread and then call `start`.
    pub fn new<I, P, F>(
        paths: I,
        options: FseventsOptions,
        callback: F,
    ) -> Result<Self, FseventsError>
    where
        I: IntoIterator<Item = P>,
        P: Into<PathBuf>,
        F: FnMut(&[FseventsEvent]) + Send + 'static,
    {
        let paths = paths.into_iter().map(Into::into).collect::<Vec<_>>();
        validate_paths(&paths)?;
        options.validate()?;

        #[cfg(not(target_os = "macos"))]
        {
            let _ = (paths, options, callback);
            Err(FseventsError::UnsupportedPlatform)
        }

        #[cfg(target_os = "macos")]
        {
            let mut callback_state = Box::new(CallbackState::new(callback));
            let context = (&mut *callback_state) as *mut CallbackState as *mut c_void;
            let native = NativeStream::create(&paths, &options, context)?;
            Ok(Self {
                lifecycle: LifecycleController::new(native),
                callback_state,
                owner_thread: thread::current().id(),
                _not_send_or_sync: PhantomData,
            })
        }
    }

    pub fn state(&self) -> StreamState {
        self.lifecycle.state()
    }

    pub fn callback_health(&self) -> CallbackHealth {
        self.callback_state.health()
    }

    pub fn schedule_on_current_run_loop(&mut self) -> Result<(), FseventsError> {
        self.ensure_owner()?;
        self.lifecycle.schedule()
    }

    pub fn start(&mut self) -> Result<(), FseventsError> {
        self.ensure_owner()?;
        self.lifecycle.start()
    }

    pub fn stop(&mut self) -> Result<(), FseventsError> {
        self.ensure_owner()?;
        self.lifecycle.stop()
    }

    pub fn restart(&mut self) -> Result<(), FseventsError> {
        self.ensure_owner()?;
        self.lifecycle.restart()
    }

    pub fn flush(&mut self) -> Result<(), FseventsError> {
        self.ensure_owner()?;
        self.lifecycle.flush()
    }

    pub fn invalidate(&mut self) -> Result<(), FseventsError> {
        self.ensure_owner()?;
        self.lifecycle.invalidate()
    }

    /// Drive the current run loop for one bounded interval.
    ///
    /// This helper exists to make the owner-thread contract explicit in tests
    /// and small command-line integrations.  Production collectors may instead
    /// drive their own run-loop integration, but must preserve the same thread
    /// and mode assumptions.
    #[cfg(target_os = "macos")]
    pub fn run_current_run_loop_for(&self, duration: Duration) -> Result<(), FseventsError> {
        self.ensure_owner()?;
        let seconds = duration.as_secs_f64();
        if !seconds.is_finite() || seconds > 60.0 * 60.0 {
            return Err(FseventsError::InvalidLatency);
        }
        unsafe {
            ffi::CFRunLoopRunInMode(ffi::kCFRunLoopDefaultMode, seconds, 0);
        }
        Ok(())
    }

    fn ensure_owner(&self) -> Result<(), FseventsError> {
        if thread::current().id() == self.owner_thread {
            Ok(())
        } else {
            Err(FseventsError::WrongThread)
        }
    }
}

impl Drop for FseventsStream {
    fn drop(&mut self) {
        // The native stream is invalidated and released before this struct's
        // callback_state field is dropped.  This is the final ownership fence.
        self.lifecycle.shutdown();
    }
}

fn validate_paths(paths: &[PathBuf]) -> Result<(), FseventsError> {
    if paths.is_empty() {
        return Err(FseventsError::EmptyPaths);
    }
    if paths.len() > MAX_WATCH_PATHS {
        return Err(FseventsError::TooManyPaths);
    }
    for path in paths {
        #[cfg(target_os = "macos")]
        let bytes = path.as_os_str().as_bytes();
        #[cfg(not(target_os = "macos"))]
        let lossy = path.to_string_lossy();
        #[cfg(not(target_os = "macos"))]
        let bytes = lossy.as_bytes();
        if bytes.len() > MAX_PATH_BYTES {
            return Err(FseventsError::PathTooLong);
        }
        if bytes.contains(&0) {
            return Err(FseventsError::PathContainsNul);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[derive(Default)]
    struct MockLog {
        calls: Vec<&'static str>,
        fail_schedule: bool,
        fail_start: bool,
    }

    struct MockNative {
        log: Rc<RefCell<MockLog>>,
    }

    impl NativeLifecycle for MockNative {
        fn schedule(&mut self) -> Result<(), NativeError> {
            let mut log = self.log.borrow_mut();
            log.calls.push("schedule");
            if log.fail_schedule {
                Err(NativeError::Schedule)
            } else {
                Ok(())
            }
        }

        fn start(&mut self) -> Result<(), NativeError> {
            let mut log = self.log.borrow_mut();
            log.calls.push("start");
            if log.fail_start {
                Err(NativeError::Start)
            } else {
                Ok(())
            }
        }

        fn stop(&mut self) {
            self.log.borrow_mut().calls.push("stop");
        }

        fn flush(&mut self) {
            self.log.borrow_mut().calls.push("flush");
        }

        fn invalidate(&mut self) {
            self.log.borrow_mut().calls.push("invalidate");
        }

        fn release(self) {
            self.log.borrow_mut().calls.push("release");
        }
    }

    fn controller(log: Rc<RefCell<MockLog>>) -> LifecycleController<MockNative> {
        LifecycleController::new(MockNative { log })
    }

    #[test]
    fn lifecycle_requires_schedule_and_is_restartable_after_stop() {
        let log = Rc::new(RefCell::new(MockLog::default()));
        let mut lifecycle = controller(Rc::clone(&log));
        assert_eq!(lifecycle.start(), Err(FseventsError::NotScheduled));
        lifecycle.schedule().expect("schedule");
        lifecycle.start().expect("start");
        lifecycle.flush().expect("flush");
        lifecycle.stop().expect("stop");
        lifecycle.restart().expect("restart");
        lifecycle.stop().expect("stop again");
        drop(lifecycle);
        assert_eq!(
            log.borrow().calls,
            vec!["schedule", "start", "flush", "stop", "start", "stop", "invalidate", "release"]
        );
    }

    #[test]
    fn schedule_failure_releases_without_invalidating_an_unscheduled_native_object() {
        let log = Rc::new(RefCell::new(MockLog { fail_schedule: true, ..MockLog::default() }));
        let mut lifecycle = controller(Rc::clone(&log));
        assert_eq!(lifecycle.schedule(), Err(FseventsError::NativeCreateFailed));
        drop(lifecycle);
        assert_eq!(log.borrow().calls, vec!["schedule", "release"]);
    }

    #[test]
    fn start_failure_invalidates_a_scheduled_object_once() {
        let log = Rc::new(RefCell::new(MockLog { fail_start: true, ..MockLog::default() }));
        let mut lifecycle = controller(Rc::clone(&log));
        lifecycle.schedule().expect("schedule");
        assert_eq!(lifecycle.start(), Err(FseventsError::NativeStartFailed));
        drop(lifecycle);
        assert_eq!(log.borrow().calls, vec!["schedule", "start", "invalidate", "release"]);
    }

    #[test]
    fn explicit_invalidation_is_idempotent_and_never_releases_twice() {
        let log = Rc::new(RefCell::new(MockLog::default()));
        let mut lifecycle = controller(Rc::clone(&log));
        lifecycle.schedule().expect("schedule");
        lifecycle.start().expect("start");
        lifecycle.invalidate().expect("invalidate");
        lifecycle.invalidate().expect("idempotent invalidate");
        assert_eq!(lifecycle.state(), StreamState::Invalidated);
        drop(lifecycle);
        assert_eq!(log.borrow().calls, vec!["schedule", "start", "stop", "invalidate", "release"]);
    }

    #[test]
    fn input_bounds_are_checked_before_platform_creation() {
        assert_eq!(validate_paths(&[]), Err(FseventsError::EmptyPaths));
        let too_many = (0..=MAX_WATCH_PATHS).map(|_| PathBuf::from("/tmp")).collect::<Vec<_>>();
        assert_eq!(validate_paths(&too_many), Err(FseventsError::TooManyPaths));
        let nul = PathBuf::from("/tmp/a\0b");
        assert_eq!(validate_paths(&[nul]), Err(FseventsError::PathContainsNul));
    }

    #[test]
    fn callback_representation_flags_are_rejected_before_native_creation() {
        for flags in
            [FLAG_USE_CF_TYPES, FLAG_USE_EXTENDED_DATA, FLAG_FULL_HISTORY, FLAG_WITH_DOC_ID]
        {
            let options = FseventsOptions { flags, ..FseventsOptions::default() };
            assert_eq!(options.validate(), Err(FseventsError::UnsupportedCallbackFlags));
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn callback_trampoline_copies_paths_and_contains_panics() {
        use std::sync::{Arc, Mutex};

        let received = Arc::new(Mutex::new(Vec::<FseventsEvent>::new()));
        let received_clone = Arc::clone(&received);
        let mut state = Box::new(CallbackState::new(move |events| {
            received_clone.lock().expect("callback lock").extend_from_slice(events);
        }));
        let path = CString::new("/private/tmp/ghostrace-callback-fixture").expect("path");
        let paths = [path.as_ptr()];
        let flags = [FLAG_FILE_EVENTS];
        let ids = [42_u64];
        let info = (&mut *state) as *mut CallbackState as *mut c_void;
        unsafe {
            callback_trampoline(
                std::ptr::null(),
                info,
                1,
                paths.as_ptr() as *mut c_void,
                flags.as_ptr(),
                ids.as_ptr(),
            );
        }
        assert_eq!(state.health().delivered_batches, 1);
        assert_eq!(state.health().delivered_events, 1);
        assert_eq!(received.lock().expect("received lock")[0].event_id, 42);

        let malformed_info = (&mut *state) as *mut CallbackState as *mut c_void;
        unsafe {
            callback_trampoline(
                std::ptr::null(),
                malformed_info,
                1,
                std::ptr::null_mut(),
                flags.as_ptr(),
                ids.as_ptr(),
            );
            callback_trampoline(
                std::ptr::null(),
                malformed_info,
                MAX_CALLBACK_EVENTS + 1,
                paths.as_ptr() as *mut c_void,
                flags.as_ptr(),
                ids.as_ptr(),
            );
        }
        assert_eq!(state.health().malformed_batches, 2);

        let mut panic_state = Box::new(CallbackState::new(|_| panic!("test callback panic")));
        let panic_info = (&mut *panic_state) as *mut CallbackState as *mut c_void;
        unsafe {
            callback_trampoline(
                std::ptr::null(),
                panic_info,
                1,
                paths.as_ptr() as *mut c_void,
                flags.as_ptr(),
                ids.as_ptr(),
            );
        }
        assert_eq!(panic_state.health().panics, 1);
        assert_eq!(panic_state.health().delivered_batches, 0);
    }
}
