//! Browser native-host manifest registration.

#![cfg(unix)]

use std::{
    fs,
    os::unix::fs::{symlink, PermissionsExt},
    path::PathBuf,
};

use ghostrace::{
    exact_origin, NativeHostAction, NativeHostError, NativeHostHealth, NativeHostInstaller,
    NATIVE_HOST_NAME,
};
use serde_json::Value;

const EXTENSION: &str = "abcdefghijklmnopabcdefghijklmnop";

struct Fixture {
    _directory: tempfile::TempDir,
    support: PathBuf,
    host: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().expect("tempdir");
        let base = fs::canonicalize(directory.path()).expect("canonical");
        fs::set_permissions(&base, fs::Permissions::from_mode(0o755)).expect("chmod");
        let support = base.join("Application Support");
        fs::create_dir(&support).expect("support");
        fs::set_permissions(&support, fs::Permissions::from_mode(0o755)).expect("chmod");
        let host = base.join("ghostrace-native-host");
        fs::write(&host, "#!/bin/sh\n").expect("host");
        fs::set_permissions(&host, fs::Permissions::from_mode(0o755)).expect("chmod");
        Self { _directory: directory, support, host }
    }

    fn installer(&self) -> NativeHostInstaller {
        NativeHostInstaller::new(&self.support, &self.host, EXTENSION).expect("installer")
    }

    fn manifest(&self, relative: &str) -> PathBuf {
        self.support.join(relative).join(format!("{NATIVE_HOST_NAME}.json"))
    }
}

#[test]
fn origins_must_be_exact_extension_identifiers() {
    assert_eq!(
        exact_origin(EXTENSION).expect("origin"),
        format!("chrome-extension://{EXTENSION}/")
    );
    for bad in [
        "*",
        "abcdefghijklmnopabcdefghijklmnoz",
        "abc",
        "ABCDEFGHIJKLMNOPABCDEFGHIJKLMNOP",
        "chrome-extension://*/",
        "",
    ] {
        assert_eq!(exact_origin(bad), Err(NativeHostError::InvalidOrigin), "{bad:?}");
    }
    let fixture = Fixture::new();
    assert!(NativeHostInstaller::new(&fixture.support, &fixture.host, "*").is_err());
    assert_eq!(
        NativeHostInstaller::new(&fixture.support, "relative/host", EXTENSION).err(),
        Some(NativeHostError::InvalidHostBinary)
    );
}

#[test]
fn plan_install_verify_and_uninstall_are_exact_and_idempotent() {
    let fixture = Fixture::new();
    let installer = fixture.installer();
    let plan = installer.plan("chrome").expect("plan");
    assert_eq!(plan.action, NativeHostAction::Create);
    assert_eq!(plan.path, fixture.manifest("Google/Chrome/NativeMessagingHosts"));
    assert!(!plan.path.exists(), "planning must not write");
    assert_eq!(installer.verify("chrome").expect("verify"), NativeHostHealth::NotInstalled);

    assert_eq!(installer.install("chrome").expect("install").action, NativeHostAction::Create);
    assert_eq!(installer.install("chrome").expect("again").action, NativeHostAction::Unchanged);
    assert_eq!(installer.verify("chrome").expect("verify"), NativeHostHealth::Intact);

    let manifest: Value =
        serde_json::from_slice(&fs::read(&plan.path).expect("manifest")).expect("json");
    assert_eq!(manifest["name"], NATIVE_HOST_NAME);
    assert_eq!(manifest["type"], "stdio");
    assert_eq!(manifest["path"], fixture.host.to_string_lossy().as_ref());
    assert_eq!(
        manifest["allowed_origins"],
        serde_json::json!([format!("chrome-extension://{EXTENSION}/")])
    );

    assert_eq!(installer.uninstall("chrome").expect("uninstall").action, NativeHostAction::Remove);
    assert!(!plan.path.exists());
    assert_eq!(installer.verify("chrome").expect("verify"), NativeHostHealth::NotInstalled);
    assert_eq!(installer.uninstall("chrome").expect("again").action, NativeHostAction::Unchanged);
    assert_eq!(installer.plan("safari").err(), Some(NativeHostError::UnknownChannel));
}

#[test]
fn unrelated_manifests_are_preserved_and_foreign_ones_refused() {
    let fixture = Fixture::new();
    let directory = fixture.support.join("Google/Chrome/NativeMessagingHosts");
    fs::create_dir_all(&directory).expect("dir");
    let other = directory.join("com.example.other.json");
    fs::write(&other, "{\"name\":\"com.example.other\"}").expect("other");
    let installer = fixture.installer();
    installer.install("chrome").expect("install");
    installer.uninstall("chrome").expect("uninstall");
    assert_eq!(fs::read_to_string(&other).expect("other"), "{\"name\":\"com.example.other\"}");

    // A manifest with our name that GHOSTRACE did not write is never taken over.
    let ours = fixture.manifest("Google/Chrome/NativeMessagingHosts");
    fs::write(&ours, "{\"name\":\"imposter\"}").expect("foreign");
    assert_eq!(installer.plan("chrome").err(), Some(NativeHostError::ForeignManifest));
    assert_eq!(installer.install("chrome").err(), Some(NativeHostError::ForeignManifest));
    assert_eq!(installer.uninstall("chrome").err(), Some(NativeHostError::ForeignManifest));
    assert_eq!(fs::read_to_string(&ours).expect("kept"), "{\"name\":\"imposter\"}");
}

#[test]
fn a_manifest_edited_to_admit_another_origin_is_drift_and_never_removed() {
    let fixture = Fixture::new();
    let installer = fixture.installer();
    installer.install("chrome").expect("install");
    let path = fixture.manifest("Google/Chrome/NativeMessagingHosts");
    let edited = fs::read_to_string(&path).expect("read").replace(
        &format!("chrome-extension://{EXTENSION}/"),
        "chrome-extension://ponmlkjihgfedcbaponmlkjihgfedcba/",
    );
    fs::write(&path, &edited).expect("tamper");
    assert_eq!(installer.verify("chrome").expect("verify"), NativeHostHealth::Drifted);
    assert_eq!(installer.plan("chrome").err(), Some(NativeHostError::Drift));
    assert_eq!(installer.uninstall("chrome").err(), Some(NativeHostError::Drift));
    assert_eq!(fs::read_to_string(&path).expect("kept"), edited);
}

#[test]
fn unsafe_directories_links_and_modes_are_refused() {
    let fixture = Fixture::new();
    let elsewhere = fixture.support.parent().expect("base").join("elsewhere");
    fs::create_dir(&elsewhere).expect("elsewhere");
    fs::create_dir_all(fixture.support.join("Google")).expect("google");
    symlink(&elsewhere, fixture.support.join("Google/Chrome")).expect("symlink");
    assert_eq!(fixture.installer().install("chrome").err(), Some(NativeHostError::UnsafeDirectory));
    assert_eq!(fs::read_dir(&elsewhere).expect("read").count(), 0);

    let writable = fixture.support.join("Chromium");
    fs::create_dir(&writable).expect("dir");
    fs::set_permissions(&writable, fs::Permissions::from_mode(0o777)).expect("chmod");
    assert_eq!(
        fixture.installer().install("chromium").err(),
        Some(NativeHostError::UnsafeDirectory)
    );

    let edge = fixture.support.join("Microsoft Edge/NativeMessagingHosts");
    fs::create_dir_all(&edge).expect("edge");
    let target = elsewhere.join("target.json");
    fs::write(&target, "{}").expect("target");
    symlink(&target, edge.join(format!("{NATIVE_HOST_NAME}.json"))).expect("manifest link");
    assert_eq!(fixture.installer().install("edge").err(), Some(NativeHostError::ForeignManifest));
    assert_eq!(fs::read_to_string(&target).expect("target"), "{}");
}

#[test]
fn channels_are_independent() {
    let fixture = Fixture::new();
    let installer = fixture.installer();
    installer.install("chrome").expect("chrome");
    installer.install("edge").expect("edge");
    installer.uninstall("chrome").expect("uninstall chrome");
    assert_eq!(installer.verify("edge").expect("edge"), NativeHostHealth::Intact);
    assert_eq!(installer.verify("chrome").expect("chrome"), NativeHostHealth::NotInstalled);
}

#[test]
fn moving_the_host_binary_is_an_upgrade_of_an_intact_manifest() {
    let fixture = Fixture::new();
    fixture.installer().install("chrome").expect("install");
    let moved = fixture.host.with_file_name("ghostrace-native-host-v2");
    fs::copy(&fixture.host, &moved).expect("copy host");
    let upgraded =
        NativeHostInstaller::new(&fixture.support, &moved, EXTENSION).expect("installer");
    assert_eq!(upgraded.plan("chrome").expect("plan").action, NativeHostAction::Replace);
    assert_eq!(upgraded.install("chrome").expect("upgrade").action, NativeHostAction::Replace);
    assert_eq!(upgraded.verify("chrome").expect("verify"), NativeHostHealth::Intact);
    let manifest: Value = serde_json::from_slice(
        &fs::read(fixture.manifest("Google/Chrome/NativeMessagingHosts")).expect("manifest"),
    )
    .expect("json");
    assert_eq!(manifest["path"], moved.to_string_lossy().as_ref());
}
