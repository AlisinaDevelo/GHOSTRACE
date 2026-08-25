use ghostrace::{
    CursorIdentity, CursorStreamMode, EventSource, MountState, SnapshotDigest, VolumeIdentity,
    VolumeObservation, VolumeTransition,
};

fn digest(byte: char) -> SnapshotDigest {
    SnapshotDigest::try_from(format!("sha256:{}", byte.to_string().repeat(64))).expect("digest")
}

fn observation(
    volume: VolumeIdentity,
    scope: SnapshotDigest,
    generation: u64,
    state: MountState,
    snapshot: Option<SnapshotDigest>,
) -> VolumeObservation {
    VolumeObservation::new(volume, scope, generation, state, snapshot)
}

#[test]
fn identity_uses_stable_fields_and_never_uses_display_names() {
    let identity = VolumeIdentity::new(17, 29, Some(digest('a'))).expect("identity");
    let rendered = serde_json::to_string(&identity).expect("identity JSON");
    assert!(rendered.contains("device_id"));
    assert!(rendered.contains("filesystem_id"));
    assert!(rendered.contains("volume_uuid_digest"));
    assert!(!rendered.contains("display_name"));
    assert!(!rendered.contains("Macintosh"));
    assert_ne!(identity.fingerprint(), VolumeIdentity::synthetic("other").fingerprint());
}

#[test]
fn mount_replacement_snapshot_and_path_reuse_are_explicit_discontinuities() {
    let volume_a = VolumeIdentity::synthetic("volume-a");
    let volume_b = VolumeIdentity::synthetic("volume-b");
    let scope_a = digest('b');
    let scope_b = digest('c');
    let mounted = observation(volume_a.clone(), scope_a.clone(), 1, MountState::Mounted, None);
    assert_eq!(mounted.transition_from(&mounted), VolumeTransition::Continuous);
    assert!(!VolumeTransition::Continuous.is_discontinuity());

    let unmounted = observation(volume_a.clone(), scope_a.clone(), 1, MountState::Unmounted, None);
    assert_eq!(unmounted.transition_from(&mounted), VolumeTransition::Unmounted);
    assert!(VolumeTransition::Unmounted.is_discontinuity());
    let mounted_again =
        observation(volume_a.clone(), scope_a.clone(), 2, MountState::Mounted, None);
    assert_eq!(mounted_again.transition_from(&unmounted), VolumeTransition::Mounted);
    assert_eq!(mounted_again.transition_from(&mounted), VolumeTransition::Remounted);

    let restored =
        observation(volume_a.clone(), scope_a.clone(), 2, MountState::Mounted, Some(digest('d')));
    assert_eq!(restored.transition_from(&mounted_again), VolumeTransition::SnapshotRestored);

    let reused_path = observation(volume_b.clone(), scope_a, 3, MountState::Mounted, None);
    assert_eq!(reused_path.transition_from(&mounted_again), VolumeTransition::PathReused);
    let replacement = observation(volume_b, scope_b, 3, MountState::Mounted, None);
    assert_eq!(replacement.transition_from(&mounted_again), VolumeTransition::DeviceReplaced);
}

#[test]
fn cursor_resume_requires_volume_and_stream_mode_match() {
    let volume_a = VolumeIdentity::synthetic("volume-a");
    let volume_b = VolumeIdentity::synthetic("volume-b");
    let first = CursorIdentity::for_volume(
        EventSource::Filesystem,
        "live-filesystem-root",
        CursorStreamMode::PerHost,
        volume_a.clone(),
    )
    .expect("cursor");
    let same = CursorIdentity::for_volume(
        EventSource::Filesystem,
        "live-filesystem-root",
        CursorStreamMode::PerHost,
        volume_a,
    )
    .expect("cursor");
    let other_volume = CursorIdentity::for_volume(
        EventSource::Filesystem,
        "live-filesystem-root",
        CursorStreamMode::PerHost,
        volume_b,
    )
    .expect("cursor");
    let other_mode = CursorIdentity::for_volume(
        EventSource::Filesystem,
        "live-filesystem-root",
        CursorStreamMode::PerDevice,
        VolumeIdentity::synthetic("volume-a"),
    )
    .expect("cursor");

    assert!(first.can_resume_from(&same));
    assert!(!first.can_resume_from(&other_volume));
    assert!(!first.can_resume_from(&other_mode));
    assert!(!CursorIdentity::new(EventSource::Filesystem, "live-filesystem-root")
        .expect("fixture cursor")
        .can_resume_from(&first));
}

#[cfg(unix)]
#[test]
fn path_identity_reads_device_and_filesystem_fields_without_mount_names() {
    let directory = tempfile::tempdir().expect("directory");
    let identity = VolumeIdentity::from_path(directory.path()).expect("volume identity");
    assert_ne!(identity.device_id, 0);
    assert_ne!(identity.filesystem_id, 0);
    let rendered = serde_json::to_string(&identity).expect("identity JSON");
    assert!(!rendered.contains(directory.path().to_string_lossy().as_ref()));
    assert!(!rendered.contains("f_mntonname"));
}
