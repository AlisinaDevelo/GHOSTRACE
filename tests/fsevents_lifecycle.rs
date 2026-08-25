use std::{
    fs,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};

use ghostrace::{FseventsError, FseventsEvent, FseventsOptions, FseventsStream, StreamState};
use tempfile::tempdir;

#[cfg(not(target_os = "macos"))]
#[test]
fn non_macos_is_an_explicit_no_go() {
    let result = FseventsStream::new(
        [PathBuf::from("/tmp/ghostrace-test")],
        FseventsOptions::default(),
        |_| {},
    );
    assert!(matches!(result, Err(FseventsError::UnsupportedPlatform)));
}

#[cfg(target_os = "macos")]
#[test]
fn native_stream_observes_metadata_and_restarts_on_one_run_loop_thread() {
    let directory = tempdir().expect("private temporary directory");
    let root_path = directory.path().join("selected-root");
    fs::create_dir(&root_path).expect("selected root");
    let root = fs::canonicalize(root_path).expect("canonical selected root");
    let received = Arc::new(Mutex::new(Vec::<FseventsEvent>::new()));
    let received_clone = Arc::clone(&received);
    let options =
        FseventsOptions { latency: Duration::from_millis(20), ..FseventsOptions::default() };
    let mut stream = FseventsStream::new([root.clone()], options, move |events| {
        received_clone.lock().expect("callback lock").extend_from_slice(events);
    })
    .expect("FSEventStreamCreate");

    assert_eq!(stream.state(), StreamState::Created);
    assert_eq!(stream.start(), Err(FseventsError::NotScheduled));
    stream.schedule_on_current_run_loop().expect("schedule");
    assert_eq!(stream.schedule_on_current_run_loop(), Err(FseventsError::AlreadyScheduled));
    stream.start().expect("start");
    assert_eq!(stream.start(), Err(FseventsError::AlreadyRunning));
    assert_eq!(stream.state(), StreamState::Running);

    let first = root.join("first.txt");
    fs::write(&first, b"metadata-only fixture").expect("create fixture file");
    for _ in 0..30 {
        stream.run_current_run_loop_for(Duration::from_millis(100)).expect("run owner loop");
        if !received.lock().expect("received lock").is_empty() {
            break;
        }
    }
    stream.flush().expect("flush");
    stream.run_current_run_loop_for(Duration::from_millis(100)).expect("drain owner loop");
    let first_batch = received.lock().expect("received lock").clone();
    assert!(!first_batch.is_empty(), "FSEvents callback did not arrive");
    assert!(first_batch.iter().any(|event| event.path.starts_with(&root)));
    assert!(first_batch.iter().all(|event| event.path != PathBuf::from("/")));

    stream.stop().expect("stop");
    assert_eq!(stream.state(), StreamState::Stopped);
    assert_eq!(stream.stop(), Err(FseventsError::NotRunning));
    assert_eq!(stream.flush(), Err(FseventsError::NotRunning));
    stream.restart().expect("restart");
    assert_eq!(stream.state(), StreamState::Running);

    let second = root.join("second.txt");
    fs::write(&second, b"second metadata-only fixture").expect("create second fixture file");
    stream.run_current_run_loop_for(Duration::from_millis(500)).expect("run restarted owner loop");
    stream.flush().expect("flush restarted stream");
    stream.invalidate().expect("invalidate");
    assert_eq!(stream.state(), StreamState::Invalidated);
    assert_eq!(stream.invalidate(), Ok(()));
    assert_eq!(stream.start(), Err(FseventsError::Invalidated));
    assert!(stream.callback_health().delivered_events >= 1);
}
