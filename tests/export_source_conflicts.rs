//! Export destinations must never replace a journal or its SQLite artifacts.

use std::{
    fs,
    path::{Path, PathBuf},
};

use ghostrace::{
    export::{export_journal, export_journal_with_confirmation},
    ingest_fixture, preview_export, validate_export, DeterministicKeyProvider, ExportRequest,
    GhostraceError, Journal, PolicyProfile,
};

struct FileJournal {
    journal: Journal,
    path: PathBuf,
    directory: tempfile::TempDir,
}

impl FileJournal {
    fn new() -> Self {
        // Keeping the directory under the current working directory gives us
        // both absolute and relative names without changing the process cwd.
        let directory = tempfile::Builder::new()
            .prefix(".ghostrace-export-conflict-")
            .tempdir_in(std::env::current_dir().expect("cwd"))
            .expect("private directory");
        private_directory(directory.path());
        let path = fs::canonicalize(directory.path())
            .expect("canonical directory")
            .join("journal.sqlite3");
        let journal = Journal::open_fixture(
            &path,
            DeterministicKeyProvider::from_seed("export-source-conflicts"),
        )
        .expect("journal");
        ingest_fixture(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/causal-chain.jsonl"),
            &journal,
            &PolicyProfile::fixture_default(),
        )
        .expect("fixture ingest");
        Self { journal, path, directory }
    }

    fn relative_path(&self) -> PathBuf {
        let cwd = fs::canonicalize(std::env::current_dir().expect("cwd")).expect("canonical cwd");
        self.path.strip_prefix(cwd).expect("relative journal name").to_path_buf()
    }

    fn artifact(&self, suffix: &str) -> PathBuf {
        self.path.with_file_name(format!("journal.sqlite3{suffix}"))
    }

    fn snapshot(&self) -> Vec<(PathBuf, Vec<u8>)> {
        ["", "-wal", "-shm", "-journal", "-tmp", "-backup"]
            .into_iter()
            .filter_map(|suffix| {
                let path = self.artifact(suffix);
                path.exists().then(|| {
                    let bytes = fs::read(&path).expect("artifact bytes");
                    (path, bytes)
                })
            })
            .collect()
    }

    fn assert_preserved(&self, before: &[(PathBuf, Vec<u8>)]) {
        assert_eq!(self.snapshot(), before, "refusal must preserve every journal artifact");
        assert!(
            fs::read_dir(self.directory.path()).expect("directory").all(|entry| !entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .starts_with(".ghostrace-export-incomplete-")),
            "refusal must not leave export spools"
        );
        let reopened = Journal::open_fixture(
            &self.path,
            DeterministicKeyProvider::from_seed("export-source-conflicts"),
        )
        .expect("original journal reopens");
        assert_eq!(reopened.events().expect("events").len(), 8);
        assert!(reopened.verify_authenticated_state().expect("valid authenticated state").valid);
    }
}

fn private_directory(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).expect("private permissions");
    }
    #[cfg(not(unix))]
    let _ = path;
}

fn assert_conflict<T>(result: Result<T, GhostraceError>) {
    match result {
        Err(GhostraceError::ExportSourceConflict) => {}
        Err(error) => panic!("expected ExportSourceConflict, got {error:?}"),
        Ok(_) => panic!("export unexpectedly accepted a protected journal destination"),
    }
}

#[cfg(unix)]
fn assert_relocated_source_refused(suffix: &str) {
    use ghostrace::{CryptoError, KeyProvider};
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };

    struct MoveSourceArtifact {
        inner: DeterministicKeyProvider,
        armed: Arc<AtomicBool>,
        artifact: PathBuf,
        destination: PathBuf,
    }
    impl KeyProvider for MoveSourceArtifact {
        fn key(&self) -> Result<[u8; 32], CryptoError> {
            if self.armed.swap(false, Ordering::SeqCst) {
                fs::rename(&self.artifact, &self.destination).expect("move active source artifact");
            }
            self.inner.key()
        }
    }

    let directory = tempfile::tempdir().expect("directory");
    private_directory(directory.path());
    let path = directory.path().join("journal.sqlite3");
    let artifact = path.with_file_name(format!("journal.sqlite3{suffix}"));
    let output = directory.path().join("unrelated-export.jsonl");
    let armed = Arc::new(AtomicBool::new(false));
    let journal = Journal::open_fixture(
        &path,
        MoveSourceArtifact {
            inner: DeterministicKeyProvider::from_seed("export-source-relocation"),
            armed: Arc::clone(&armed),
            artifact: artifact.clone(),
            destination: output.clone(),
        },
    )
    .expect("journal");
    ingest_fixture(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/causal-chain.jsonl"),
        &journal,
        &PolicyProfile::fixture_default(),
    )
    .expect("fixture ingest");
    journal.for_each_ordered_event(|_| Ok(())).expect("warm read-only snapshot");
    let before = ["", "-wal", "-shm"]
        .into_iter()
        .map(|suffix| {
            let artifact = path.with_file_name(format!("journal.sqlite3{suffix}"));
            let bytes = fs::read(&artifact).expect("artifact bytes");
            (artifact, bytes)
        })
        .collect::<Vec<_>>();
    armed.store(true, Ordering::SeqCst);
    let result = export_journal(&journal, &output, true);
    assert!(!armed.load(Ordering::SeqCst), "relocation seam must run during decryption");
    assert_conflict(result);
    let moved_bytes = before.iter().find(|(path, _)| path == &artifact).expect("moved artifact");
    assert_eq!(fs::read(&output).expect("relocated artifact bytes"), moved_bytes.1);
    assert!(!artifact.exists(), "refusal must leave the relocated artifact in place");
    fs::rename(&output, &artifact).expect("restore source artifact name");
    for (artifact, bytes) in before {
        assert_eq!(fs::read(artifact).expect("artifact bytes"), bytes);
    }
    assert!(
        fs::read_dir(directory.path()).expect("directory").all(|entry| !entry
            .expect("entry")
            .file_name()
            .to_string_lossy()
            .starts_with(".ghostrace-export-incomplete-")),
        "refusal must not leave export spools"
    );
    let reopened = Journal::open_fixture(
        &path,
        DeterministicKeyProvider::from_seed("export-source-relocation"),
    )
    .expect("restored source reopens");
    assert_eq!(reopened.events().expect("events").len(), 8);
    assert!(reopened.verify_authenticated_state().expect("authenticated state").valid);
}

#[cfg(unix)]
#[test]
fn forced_export_rejects_the_main_database_relocated_during_decryption() {
    assert_relocated_source_refused("");
}

#[cfg(unix)]
#[test]
fn forced_export_rejects_the_wal_relocated_during_decryption() {
    assert_relocated_source_refused("-wal");
}

#[cfg(unix)]
#[test]
fn forced_export_rejects_the_shm_relocated_during_decryption() {
    assert_relocated_source_refused("-shm");
}

#[cfg(unix)]
#[test]
fn parent_replacement_during_streaming_cannot_redirect_export_into_the_journal() {
    use ghostrace::{CryptoError, KeyProvider};
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };

    struct SwapParent {
        inner: DeterministicKeyProvider,
        armed: Arc<AtomicBool>,
        output_parent: PathBuf,
        moved_parent: PathBuf,
        source_parent: PathBuf,
    }
    impl KeyProvider for SwapParent {
        fn key(&self) -> Result<[u8; 32], CryptoError> {
            if self.armed.swap(false, Ordering::SeqCst) {
                let files = fs::read_dir(&self.output_parent)
                    .expect("output parent")
                    .map(|entry| entry.expect("entry").file_name())
                    .collect::<Vec<_>>();
                assert!(files.iter().all(|name| name
                    .to_string_lossy()
                    .starts_with(".ghostrace-export-incomplete-body-")));
                fs::rename(&self.output_parent, &self.moved_parent).expect("move output parent");
                std::os::unix::fs::symlink(&self.source_parent, &self.output_parent)
                    .expect("redirect output parent");
                // The old exporter reopens this spool through the replaced
                // parent. A retained anonymous handle cannot be redirected.
                for name in files {
                    fs::rename(self.moved_parent.join(&name), self.source_parent.join(name))
                        .expect("move named spool into redirected parent");
                }
            }
            self.inner.key()
        }
    }

    let directory = tempfile::tempdir().expect("directory");
    private_directory(directory.path());
    let source_parent = directory.path().join("source");
    let output_parent = directory.path().join("exports");
    let moved_parent = directory.path().join("moved-exports");
    fs::create_dir(&output_parent).expect("output parent");
    let path = source_parent.join("journal.sqlite3");
    let armed = Arc::new(AtomicBool::new(false));
    let journal = Journal::open_fixture(
        &path,
        SwapParent {
            inner: DeterministicKeyProvider::from_seed("export-source-parent-race"),
            armed: Arc::clone(&armed),
            output_parent: output_parent.clone(),
            moved_parent: moved_parent.clone(),
            source_parent: source_parent.clone(),
        },
    )
    .expect("journal");
    ingest_fixture(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/causal-chain.jsonl"),
        &journal,
        &PolicyProfile::fixture_default(),
    )
    .expect("fixture ingest");
    journal.for_each_ordered_event(|_| Ok(())).expect("warm read-only snapshot");
    let before = ["journal.sqlite3", "journal.sqlite3-wal", "journal.sqlite3-shm"]
        .into_iter()
        .map(|name| {
            let artifact = source_parent.join(name);
            let bytes = fs::read(&artifact).expect("artifact bytes");
            (artifact, bytes)
        })
        .collect::<Vec<_>>();
    armed.store(true, Ordering::SeqCst);
    let result = export_journal(&journal, output_parent.join("journal.sqlite3"), true);
    assert!(
        matches!(result, Err(GhostraceError::PathRace)),
        "parent replacement must be refused, got {result:?}"
    );
    assert!(!armed.load(Ordering::SeqCst), "replacement seam must run");
    for (artifact, bytes) in before {
        assert_eq!(fs::read(artifact).expect("artifact bytes"), bytes);
    }
    for parent in [&source_parent, &moved_parent] {
        assert!(
            fs::read_dir(parent).expect("parent").all(|entry| !entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .starts_with(".ghostrace-export-incomplete-")),
            "refusal must clean up spools through the original directory"
        );
    }
    fs::remove_file(output_parent).expect("remove replacement symlink");
    let reopened = Journal::open_fixture(
        &path,
        DeterministicKeyProvider::from_seed("export-source-parent-race"),
    )
    .expect("source reopens");
    assert_eq!(reopened.events().expect("events").len(), 8);
    assert!(reopened.verify_authenticated_state().expect("authenticated state").valid);
}

#[test]
fn forced_export_rejects_a_relative_alias_of_the_source() {
    let fixture = FileJournal::new();
    let alias = fixture.relative_path();
    assert_ne!(fixture.path, alias);
    let before = fixture.snapshot();
    assert_conflict(export_journal(&fixture.journal, &alias, true));
    fixture.assert_preserved(&before);
}

#[test]
fn preview_rejects_a_relative_alias_of_the_source() {
    let fixture = FileJournal::new();
    let before = fixture.snapshot();
    assert_conflict(preview_export(
        &fixture.journal,
        &PolicyProfile::fixture_default(),
        &ExportRequest { force: true, ..Default::default() },
        fixture.relative_path(),
    ));
    fixture.assert_preserved(&before);
}

#[test]
fn confirmed_export_rejects_substitution_of_a_source_alias() {
    let fixture = FileJournal::new();
    let output = fixture.directory.path().join("existing-export.jsonl");
    fs::write(&output, b"previous export\n").expect("existing output");
    let policy = PolicyProfile::fixture_default();
    let preview = preview_export(
        &fixture.journal,
        &policy,
        &ExportRequest { force: true, ..Default::default() },
        &output,
    )
    .expect("unrelated destination preview");
    let before = fixture.snapshot();
    assert_conflict(export_journal_with_confirmation(
        &fixture.journal,
        fixture.relative_path(),
        preview.confirm(),
        &policy,
    ));
    assert_eq!(fs::read(output).expect("unrelated destination"), b"previous export\n");
    fixture.assert_preserved(&before);
}

#[test]
fn forced_export_rejects_existing_wal_and_shm_destinations() {
    for suffix in ["-wal", "-shm"] {
        let fixture = FileJournal::new();
        let output = fixture.artifact(suffix);
        assert!(output.exists(), "test needs a live {suffix} artifact");
        let before = fixture.snapshot();
        assert_conflict(export_journal(&fixture.journal, &output, true));
        fixture.assert_preserved(&before);
    }
}

#[test]
fn export_reserves_absent_rollback_and_temporary_sidecar_names() {
    for suffix in ["-journal", "-tmp", "-backup"] {
        let fixture = FileJournal::new();
        let output = fixture.artifact(suffix);
        assert!(!output.exists());
        let before = fixture.snapshot();
        assert_conflict(export_journal(&fixture.journal, &output, false));
        assert!(!output.exists(), "refusal must leave an absent sidecar absent");
        fixture.assert_preserved(&before);
    }
}

#[test]
fn preview_rejects_every_sqlite_artifact_name() {
    let fixture = FileJournal::new();
    let policy = PolicyProfile::fixture_default();
    let before = fixture.snapshot();
    for suffix in ["-wal", "-shm", "-journal", "-tmp", "-backup"] {
        assert_conflict(preview_export(
            &fixture.journal,
            &policy,
            &ExportRequest::default(),
            fixture.artifact(suffix),
        ));
    }
    fixture.assert_preserved(&before);
}

#[cfg(unix)]
#[test]
fn export_rejects_a_hard_link_alias_before_touching_source_bytes() {
    let fixture = FileJournal::new();
    let alias = fixture.directory.path().join("hard-link.jsonl");
    fs::hard_link(&fixture.path, &alias).expect("hard link");
    let before = fixture.snapshot();
    assert_conflict(export_journal(&fixture.journal, &alias, true));
    fs::remove_file(alias).expect("remove deliberate extra link before reopen");
    fixture.assert_preserved(&before);
}

#[test]
fn file_backed_export_can_create_an_unrelated_parent_directory() {
    let fixture = FileJournal::new();
    let output = fixture.directory.path().join("new-exports/journal.sqlite3");
    assert!(!output.parent().expect("output parent").exists());
    export_journal(&fixture.journal, &output, false).expect("unrelated export");
    assert_eq!(validate_export(output).expect("valid export").event_count, 8);
    assert_eq!(fixture.journal.events().expect("original events").len(), 8);
}

#[test]
fn confirmed_export_can_replace_an_unrelated_file_beside_the_journal() {
    let fixture = FileJournal::new();
    let output = fixture.directory.path().join("journal.sqlite3.export");
    fs::write(&output, b"previous export\n").expect("old export");
    let policy = PolicyProfile::fixture_default();
    let preview = preview_export(
        &fixture.journal,
        &policy,
        &ExportRequest { force: true, ..Default::default() },
        &output,
    )
    .expect("unrelated export preview");
    export_journal_with_confirmation(&fixture.journal, &output, preview.confirm(), &policy)
        .expect("confirmed unrelated export");
    assert_eq!(validate_export(output).expect("valid export").event_count, 8);
    assert_eq!(fixture.journal.events().expect("original events").len(), 8);
}

#[cfg(target_os = "macos")]
#[test]
fn preview_reserves_absent_sidecar_case_aliases_on_case_insensitive_volumes() {
    let fixture = FileJournal::new();
    if !fixture.path.with_file_name("JOURNAL.SQLITE3").exists() {
        eprintln!("case-sensitive volume: case aliases are distinct files");
        return;
    }
    let before = fixture.snapshot();
    for name in ["JOURNAL.SQLITE3-JOURNAL", "JOURNAL.SQLITE3-BACKUP"] {
        let output = fixture.path.with_file_name(name);
        assert!(!output.exists());
        assert_conflict(preview_export(
            &fixture.journal,
            &PolicyProfile::fixture_default(),
            &ExportRequest::default(),
            &output,
        ));
        assert!(!output.exists());
    }
    fixture.assert_preserved(&before);
}

#[cfg(target_os = "macos")]
#[test]
fn forced_export_rejects_the_trusted_macos_tmp_alias() {
    let directory = tempfile::tempdir_in("/private/tmp").expect("directory");
    private_directory(directory.path());
    let canonical = fs::canonicalize(directory.path()).expect("canonical directory");
    let path = canonical.join("journal.sqlite3");
    let journal =
        Journal::open_fixture(&path, DeterministicKeyProvider::from_seed("export-macos-alias"))
            .expect("journal");
    ingest_fixture(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/causal-chain.jsonl"),
        &journal,
        &PolicyProfile::fixture_default(),
    )
    .expect("fixture ingest");
    let alias = Path::new("/tmp").join(path.strip_prefix("/private/tmp").expect("tmp path"));
    let before = fs::read(&path).expect("source bytes");
    assert_conflict(export_journal(&journal, &alias, true));
    assert_eq!(fs::read(&path).expect("source bytes"), before);
    assert_eq!(journal.events().expect("events").len(), 8);
}
