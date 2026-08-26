use std::collections::BTreeSet;

use ghostrace::{
    GitAlternateObjectDatabaseState, GitBranchClass, GitIdentity, GitObjectFormat, GitObjectIdRef,
    GitOperation, GitPartialCloneState, GitReplaceRefsState, GitRepositoryKind,
    GitShallowHistoryState, GitSnapshotError, GitSnapshotMetadata, GitSourceLimitations,
    GitSourceScope, GitStatusCounts, GitSubmoduleState, GitWorktreeState, RootId,
    GIT_OBJECT_READ_POLICY, GIT_SNAPSHOT_SCHEMA_JSON, GIT_SNAPSHOT_SCHEMA_VERSION,
    MAX_GIT_SNAPSHOT_BYTES,
};
use serde_json::{json, Value};

const GOLDEN: &str = include_str!("../fixtures/git-snapshot-metadata-v1.golden.json");

fn limitations() -> GitSourceLimitations {
    GitSourceLimitations {
        partial_clone: GitPartialCloneState::Full,
        replace_refs: GitReplaceRefsState::None,
        shallow_history: GitShallowHistoryState::Complete,
        submodules: GitSubmoduleState::None,
        alternate_object_database: GitAlternateObjectDatabaseState::None,
    }
}

fn identity(kind: GitRepositoryKind) -> GitIdentity {
    let (scope, root) = match kind {
        GitRepositoryKind::Submodule => (GitSourceScope::Submodule, "root-submodule"),
        _ => (GitSourceScope::SelectedRoot, "root-main"),
    };
    GitIdentity::from_stable_parts(
        ghostrace::GitFilesystemIdentity::new(44, 55).expect("object identity"),
        (kind != GitRepositoryKind::Bare)
            .then(|| ghostrace::GitFilesystemIdentity::new(44, 56).expect("worktree identity")),
        RootId::try_from(root).expect("root ID"),
        scope,
        kind,
    )
    .expect("Git identity")
}

fn sha1(value: &str) -> GitObjectIdRef {
    GitObjectIdRef::new(GitObjectFormat::Sha1, value).expect("SHA-1 object ID")
}

fn sha256(value: &str) -> GitObjectIdRef {
    GitObjectIdRef::new(GitObjectFormat::Sha256, value).expect("SHA-256 object ID")
}

fn standard_snapshot() -> GitSnapshotMetadata {
    GitSnapshotMetadata::from_identity(
        &identity(GitRepositoryKind::Standard),
        GitObjectFormat::Sha1,
        Some(sha1("0123456789abcdef0123456789abcdef01234567")),
        Some(sha1("89abcdef0123456789abcdef0123456789abcdef")),
        None,
        GitWorktreeState::Modified,
        GitBranchClass::Local,
        GitOperation::Idle,
        GitStatusCounts::new(1, 2, 1, 0).expect("status counts"),
        limitations(),
    )
    .expect("snapshot")
}

#[test]
fn checked_in_contract_is_strict_bounded_and_deterministic() {
    let document: Value = serde_json::from_str(GOLDEN).expect("golden JSON");
    let schema: Value = serde_json::from_str(GIT_SNAPSHOT_SCHEMA_JSON).expect("schema JSON");
    let validator = jsonschema::options().build(&schema).expect("schema compiles");
    assert!(validator.is_valid(&document));

    let snapshot = GitSnapshotMetadata::checked_in().expect("checked-in snapshot");
    assert_eq!(snapshot.schema_version, GIT_SNAPSHOT_SCHEMA_VERSION);
    assert_eq!(snapshot.object_format, GitObjectFormat::Sha1);
    assert_eq!(snapshot.worktree_state, GitWorktreeState::Modified);
    assert_eq!(snapshot.branch_class, GitBranchClass::Local);
    assert_eq!(snapshot.operation, GitOperation::Idle);
    assert_eq!(snapshot.digest().expect("digest"), snapshot.snapshot_digest);
    assert_eq!(serde_json::to_value(&snapshot).expect("snapshot value"), document);

    let mut unknown = document.clone();
    unknown
        .as_object_mut()
        .expect("snapshot object")
        .insert("remote_url".to_owned(), Value::String("GHOSTRACE_SECRET_REMOTE".to_owned()));
    let unknown_text = serde_json::to_string(&unknown).expect("unknown JSON");
    let error = GitSnapshotMetadata::parse(&unknown_text).expect_err("unknown field rejected");
    assert_eq!(error, GitSnapshotError::Malformed);
    assert!(!error.to_string().contains("GHOSTRACE_SECRET_REMOTE"));

    assert_eq!(GIT_OBJECT_READ_POLICY, "metadata_only");
    assert!(serde_json::to_vec(&snapshot).expect("snapshot bytes").len() < MAX_GIT_SNAPSHOT_BYTES);
}

#[test]
fn object_ids_are_explicitly_algorithm_aware_and_content_free() {
    let sha1_value = "0123456789abcdef0123456789abcdef01234567";
    let sha256_value = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    let first = GitObjectIdRef::parse_tagged(&format!("sha1:{sha1_value}")).expect("tagged SHA-1");
    let second =
        GitObjectIdRef::parse_tagged(&format!("sha256:{sha256_value}")).expect("tagged SHA-256");
    assert_eq!(first.algorithm(), GitObjectFormat::Sha1);
    assert_eq!(second.algorithm(), GitObjectFormat::Sha256);
    assert_eq!(first.tagged(), format!("sha1:{sha1_value}"));
    assert_eq!(second.tagged(), format!("sha256:{sha256_value}"));
    for invalid in [
        "sha1:0123456789abcdef0123456789abcdef0123456",
        "sha1:0123456789abcdef0123456789abcdef012345678",
        "sha1:0123456789abcdef0123456789abcdef0123456Z",
        "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcde",
        "SHA1:0123456789abcdef0123456789abcdef01234567",
        "sha1:../../etc/passwd",
    ] {
        assert_eq!(GitObjectIdRef::parse_tagged(invalid), Err(GitSnapshotError::InvalidObjectId));
    }

    let snapshot = standard_snapshot();
    let serialized = serde_json::to_string(&snapshot).expect("snapshot JSON");
    for forbidden in [
        "remote",
        "commit_message",
        "author",
        "filename",
        "diff",
        "patch",
        "content",
        "GHOSTRACE_SECRET",
    ] {
        assert!(!serialized.contains(forbidden), "serialized snapshot contains {forbidden}");
    }
    // The public constructor accepts only normalized values and has no path,
    // command, or object-reader parameter; this is the no-object-read default.
    assert_eq!(snapshot.head.as_ref().expect("head").algorithm(), GitObjectFormat::Sha1);
}

#[test]
fn snapshot_captures_bounded_status_and_operation_facts_without_names() {
    let snapshot = standard_snapshot();
    assert_eq!(snapshot.status.staged, 1);
    assert_eq!(snapshot.status.unstaged, 2);
    assert_eq!(snapshot.status.untracked, 1);
    assert_eq!(snapshot.status.conflicted, 0);
    assert_eq!(snapshot.operation, GitOperation::Idle);
    assert_eq!(snapshot.branch_class, GitBranchClass::Local);

    let mut values = BTreeSet::new();
    for operation in [
        GitOperation::Merge,
        GitOperation::Rebase,
        GitOperation::CherryPick,
        GitOperation::Revert,
        GitOperation::Bisect,
        GitOperation::Apply,
        GitOperation::Sequencer,
        GitOperation::Unknown,
    ] {
        let candidate = GitSnapshotMetadata::from_identity(
            &identity(GitRepositoryKind::Standard),
            GitObjectFormat::Sha1,
            None,
            None,
            None,
            GitWorktreeState::Unknown,
            GitBranchClass::Unknown,
            operation,
            GitStatusCounts::new(0, 0, 0, 0).expect("empty counts"),
            limitations(),
        )
        .expect("operation snapshot");
        values.insert(serde_json::to_string(&candidate).expect("operation JSON"));
    }
    assert_eq!(values.len(), 8);

    assert!(GitStatusCounts::new(1_000_000, 0, 0, 0).is_ok());
    assert_eq!(
        GitStatusCounts::new(1_000_001, 0, 0, 0),
        Err(GitSnapshotError::StatusCountExceeded)
    );
}

#[test]
fn source_limitations_are_required_and_bare_repositories_have_no_worktree() {
    let bare_identity = identity(GitRepositoryKind::Bare);
    let bare = GitSnapshotMetadata::from_identity(
        &bare_identity,
        GitObjectFormat::Sha256,
        Some(sha256("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef")),
        Some(sha256("abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789")),
        None,
        GitWorktreeState::NotApplicable,
        GitBranchClass::NoWorktree,
        GitOperation::Idle,
        GitStatusCounts::new(0, 0, 0, 0).expect("empty counts"),
        GitSourceLimitations {
            partial_clone: GitPartialCloneState::Partial,
            replace_refs: GitReplaceRefsState::Active,
            shallow_history: GitShallowHistoryState::Shallow,
            submodules: GitSubmoduleState::Unknown,
            alternate_object_database: GitAlternateObjectDatabaseState::Present,
        },
    )
    .expect("bare snapshot");
    assert_eq!(bare.repository_kind, GitRepositoryKind::Bare);
    assert_eq!(bare.worktree_state, GitWorktreeState::NotApplicable);
    assert_eq!(bare.branch_class, GitBranchClass::NoWorktree);
    assert_eq!(bare.limitations.partial_clone, GitPartialCloneState::Partial);
    assert_eq!(bare.limitations.replace_refs, GitReplaceRefsState::Active);
    assert_eq!(bare.limitations.shallow_history, GitShallowHistoryState::Shallow);
    assert_eq!(bare.limitations.submodules, GitSubmoduleState::Unknown);
    assert_eq!(
        bare.limitations.alternate_object_database,
        GitAlternateObjectDatabaseState::Present
    );

    let mut invalid = bare.clone();
    invalid.worktree_state = GitWorktreeState::Clean;
    assert_eq!(invalid.validate(), Err(GitSnapshotError::RepositoryKindMismatch));
    let mut incomplete = standard_snapshot();
    incomplete.limitations.submodules = GitSubmoduleState::Present;
    assert_eq!(incomplete.validate(), Err(GitSnapshotError::DigestMismatch));

    let submodule_identity = identity(GitRepositoryKind::Submodule);
    let mut submodule_limitations = limitations();
    submodule_limitations.submodules = GitSubmoduleState::None;
    assert_eq!(
        GitSnapshotMetadata::from_identity(
            &submodule_identity,
            GitObjectFormat::Sha1,
            None,
            None,
            None,
            GitWorktreeState::Clean,
            GitBranchClass::DetachedHead,
            GitOperation::Idle,
            GitStatusCounts::new(0, 0, 0, 0).expect("empty counts"),
            submodule_limitations,
        ),
        Err(GitSnapshotError::RepositoryKindMismatch)
    );
}

#[test]
fn object_format_and_digest_changes_fail_closed_without_echoing_input() {
    let mut wrong_format = standard_snapshot();
    wrong_format.object_format = GitObjectFormat::Sha256;
    assert_eq!(wrong_format.validate(), Err(GitSnapshotError::ObjectFormatMismatch));

    let mut changed = standard_snapshot();
    changed.operation = GitOperation::Merge;
    assert_eq!(changed.validate(), Err(GitSnapshotError::DigestMismatch));

    let mut value: Value = serde_json::from_str(GOLDEN).expect("golden JSON");
    value["branch"] = json!("GHOSTRACE_SECRET/feature");
    let error = GitSnapshotMetadata::parse(&serde_json::to_string(&value).expect("JSON"))
        .expect_err("raw branch rejected");
    assert_eq!(error, GitSnapshotError::Malformed);
    assert!(!error.to_string().contains("GHOSTRACE_SECRET"));

    let oversized = "x".repeat(MAX_GIT_SNAPSHOT_BYTES + 1);
    assert_eq!(GitSnapshotMetadata::parse(&oversized), Err(GitSnapshotError::MetadataTooLarge));

    let mut missing = value;
    missing.as_object_mut().expect("snapshot object").remove("head");
    assert_eq!(
        GitSnapshotMetadata::parse(&serde_json::to_string(&missing).expect("JSON")),
        Err(GitSnapshotError::Malformed)
    );
}
