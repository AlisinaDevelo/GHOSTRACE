use std::{collections::BTreeSet, fs};

use ghostrace::{
    GitContinuity, GitFilesystemIdentity, GitIdentity, GitIdentityError, GitRepositoryKind,
    GitSourceScope, RootId, GIT_IDENTITY_CONTRACT_VERSION, GIT_IDENTITY_SCHEMA_JSON,
};
use serde::Deserialize;
use serde_json::Value;
use tempfile::tempdir;

const FIXTURE: &str = include_str!("../fixtures/git-repository-worktree-identity-v1.json");

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct IdentityFixture {
    schema_version: u32,
    program: String,
    generator: Generator,
    privacy: Privacy,
    resource_limits: ResourceLimits,
    scenarios: Vec<Scenario>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Generator {
    version: String,
    seed: String,
    algorithm: String,
    command: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Privacy {
    synthetic_only: bool,
    user_data_included: bool,
    network_required: bool,
    retains_remote_urls: bool,
    retains_credentials: bool,
    retains_config_values: bool,
    retains_reflog_messages: bool,
    retains_filesystem_paths: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ResourceLimits {
    max_scenarios: usize,
    max_identity_bytes: usize,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Scenario {
    id: String,
    previous: WireIdentity,
    current: WireIdentity,
    expected_continuity: GitContinuity,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireIdentity {
    object_database: WireFilesystemIdentity,
    worktree: Option<WireFilesystemIdentity>,
    selected_root_id: String,
    source_scope: GitSourceScope,
    repository_kind: GitRepositoryKind,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireFilesystemIdentity {
    device_id: u64,
    file_id: u64,
}

fn fixture() -> IdentityFixture {
    serde_json::from_str(FIXTURE).expect("Git identity fixture")
}

fn identity(wire: &WireIdentity) -> GitIdentity {
    GitIdentity::from_stable_parts(
        GitFilesystemIdentity::new(wire.object_database.device_id, wire.object_database.file_id)
            .expect("object database identity"),
        wire.worktree.as_ref().map(|worktree| {
            GitFilesystemIdentity::new(worktree.device_id, worktree.file_id)
                .expect("worktree identity")
        }),
        RootId::try_from(wire.selected_root_id.clone()).expect("selected root ID"),
        wire.source_scope,
        wire.repository_kind,
    )
    .expect("Git identity")
}

#[test]
fn fixture_schema_is_strict_synthetic_and_covers_every_transition() {
    let document: Value = serde_json::from_str(FIXTURE).expect("fixture JSON");
    let schema: Value = serde_json::from_str(GIT_IDENTITY_SCHEMA_JSON).expect("schema JSON");
    let validator = jsonschema::options().build(&schema).expect("identity schema compiles");
    assert!(validator.is_valid(&document));

    let mut unknown = document.clone();
    unknown
        .as_object_mut()
        .expect("fixture object")
        .insert("unknown_field".to_owned(), Value::Bool(true));
    assert!(!validator.is_valid(&unknown));

    let parsed = fixture();
    assert_eq!(parsed.schema_version, GIT_IDENTITY_CONTRACT_VERSION);
    assert_eq!(parsed.program, "ghostrace-git-repository-worktree-identity-v1");
    assert_eq!(parsed.generator.version, "ghostrace-git-identity-fixture-v1");
    assert_eq!(parsed.generator.seed, "ghostrace-git-identity-seed-v1");
    assert!(parsed.generator.algorithm.contains("no runtime randomness"));
    assert!(parsed.generator.command.contains("git_identity"));
    assert!(parsed.privacy.synthetic_only);
    assert!(!parsed.privacy.user_data_included);
    assert!(!parsed.privacy.network_required);
    assert!(!parsed.privacy.retains_remote_urls);
    assert!(!parsed.privacy.retains_credentials);
    assert!(!parsed.privacy.retains_config_values);
    assert!(!parsed.privacy.retains_reflog_messages);
    assert!(!parsed.privacy.retains_filesystem_paths);
    assert_eq!(parsed.resource_limits.max_scenarios, 7);
    assert_eq!(parsed.resource_limits.max_identity_bytes, 4096);
    assert_eq!(parsed.scenarios.len(), parsed.resource_limits.max_scenarios);
    let scenario_ids =
        parsed.scenarios.iter().map(|scenario| scenario.id.as_str()).collect::<BTreeSet<_>>();
    assert_eq!(
        scenario_ids,
        [
            "bare",
            "clone",
            "move",
            "repository_reinitialization",
            "source_scope_rebinding",
            "submodule",
            "worktree_add",
        ]
        .into_iter()
        .collect()
    );

    for scenario in &parsed.scenarios {
        let previous = identity(&scenario.previous);
        let current = identity(&scenario.current);
        assert_eq!(
            current.continuity_from(&previous),
            scenario.expected_continuity,
            "{}",
            scenario.id
        );
        assert!(
            serde_json::to_vec(&current).expect("identity JSON").len()
                <= parsed.resource_limits.max_identity_bytes
        );
    }
}

#[test]
fn repository_id_is_stable_across_worktrees_and_changes_for_a_clone() {
    let parsed = fixture();
    let move_case =
        parsed.scenarios.iter().find(|scenario| scenario.id == "move").expect("move case");
    let worktree_case = parsed
        .scenarios
        .iter()
        .find(|scenario| scenario.id == "worktree_add")
        .expect("worktree case");
    let clone_case =
        parsed.scenarios.iter().find(|scenario| scenario.id == "clone").expect("clone case");
    assert_eq!(
        identity(&move_case.current).repository_id(),
        identity(&move_case.previous).repository_id()
    );
    assert_eq!(
        identity(&worktree_case.current).repository_id(),
        identity(&worktree_case.previous).repository_id()
    );
    assert_ne!(
        identity(&move_case.current).repository_id(),
        identity(&clone_case.current).repository_id()
    );
}

#[test]
fn invalid_scope_and_zero_filesystem_identity_fail_without_input_echo() {
    assert_eq!(GitFilesystemIdentity::new(0, 1), Err(GitIdentityError::InvalidFilesystemIdentity));
    let root = RootId::try_from("root-main").expect("root ID");
    let object = GitFilesystemIdentity::new(1, 2).expect("object identity");
    assert_eq!(
        GitIdentity::from_stable_parts(
            object,
            None,
            root,
            GitSourceScope::Submodule,
            GitRepositoryKind::Standard,
        ),
        Err(GitIdentityError::InvalidScope)
    );
    let mut valid = identity(&fixture().scenarios[0].current);
    valid.contract_version = GIT_IDENTITY_CONTRACT_VERSION + 1;
    assert_eq!(
        valid.validate(),
        Err(GitIdentityError::UnsupportedContractVersion(GIT_IDENTITY_CONTRACT_VERSION + 1))
    );
    assert_eq!(
        valid.continuity_from(&identity(&fixture().scenarios[0].current)),
        GitContinuity::Incomparable
    );
}

#[test]
fn forbidden_git_metadata_is_rejected_and_never_serialized() {
    let parsed = fixture();
    let base = identity(&parsed.scenarios[0].current);
    let forbidden = [
        ("remote_url", "https://user:GHOSTRACE_SECRET@host.invalid/repo.git"),
        ("credential_helper", "GHOSTRACE_SECRET_HELPER"),
        ("config_value", "GHOSTRACE_SECRET_CONFIG"),
        ("reflog_message", "GHOSTRACE_SECRET_REFLOG"),
        ("filesystem_path", "/private/GHOSTRACE_SECRET/path"),
    ];
    for (field, sentinel) in forbidden {
        let mut value = serde_json::to_value(&base).expect("identity JSON");
        value[field] = Value::String(sentinel.to_owned());
        let error =
            serde_json::from_value::<GitIdentity>(value).expect_err("unknown field rejected");
        assert!(!error.to_string().contains(sentinel));
    }

    let serialized = serde_json::to_string(&base).expect("identity JSON");
    assert!(!serialized.contains("remote"));
    assert!(!serialized.contains("credential"));
    assert!(!serialized.contains("reflog"));
    assert!(!serialized.contains("/private/"));
}

#[cfg(unix)]
#[test]
fn moving_a_real_repository_shape_keeps_identity_without_retaining_paths() {
    let directory = tempdir().expect("temporary directory");
    let original = directory.path().join("GHOSTRACE_SECRET_source");
    let moved = directory.path().join("moved");
    let object_database = original.join(".git").join("objects");
    fs::create_dir_all(&object_database).expect("object database");

    let before = GitIdentity::from_paths(
        &object_database,
        Some(&original),
        RootId::try_from("root-main").expect("root ID"),
        GitSourceScope::SelectedRoot,
        GitRepositoryKind::Standard,
    )
    .expect("identity before move");

    fs::rename(&original, &moved).expect("move repository shape");
    let after = GitIdentity::from_paths(
        &moved.join(".git").join("objects"),
        Some(&moved),
        RootId::try_from("root-main").expect("root ID"),
        GitSourceScope::SelectedRoot,
        GitRepositoryKind::Standard,
    )
    .expect("identity after move");

    assert_eq!(after.continuity_from(&before), GitContinuity::Continuous);
    assert_eq!(after.object_database_digest, before.object_database_digest);
    assert_eq!(after.worktree_digest, before.worktree_digest);
    let serialized = serde_json::to_string(&after).expect("identity JSON");
    assert!(!serialized.contains("GHOSTRACE_SECRET_source"));
    assert!(!format!("{after:?}").contains("GHOSTRACE_SECRET_source"));
}

#[cfg(unix)]
#[test]
fn bare_identity_omits_worktree_and_linked_worktree_changes_only_worktree() {
    let directory = tempdir().expect("temporary directory");
    let bare = directory.path().join("bare.git");
    let linked = directory.path().join("linked-worktree");
    fs::create_dir(&bare).expect("bare repository shape");
    fs::create_dir(&linked).expect("linked worktree shape");

    let bare_identity = GitIdentity::from_paths(
        &bare,
        None,
        RootId::try_from("root-bare").expect("root ID"),
        GitSourceScope::Repository,
        GitRepositoryKind::Bare,
    )
    .expect("bare identity");
    assert!(bare_identity.worktree_digest.is_none());

    let standard = GitIdentity::from_stable_parts(
        GitFilesystemIdentity::new(44, 55).expect("object identity"),
        Some(GitFilesystemIdentity::new(44, 56).expect("worktree identity")),
        RootId::try_from("root-main").expect("root ID"),
        GitSourceScope::SelectedRoot,
        GitRepositoryKind::Standard,
    )
    .expect("standard identity");
    let linked_identity = GitIdentity::from_stable_parts(
        GitFilesystemIdentity::new(44, 55).expect("object identity"),
        Some(GitFilesystemIdentity::new(44, 57).expect("worktree identity")),
        RootId::try_from("root-linked").expect("root ID"),
        GitSourceScope::Worktree,
        GitRepositoryKind::Standard,
    )
    .expect("linked identity");
    assert_eq!(linked_identity.continuity_from(&standard), GitContinuity::WorktreeChanged);
}
