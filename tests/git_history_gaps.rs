//! Git history transitions: observed ref movement stays separate from inferred
//! ancestry, and rewritten, missing, truncated, replaced, or unprobed history
//! becomes a typed gap carrying the last known and current bounded state.

use ghostrace::{
    GitAlternateObjectDatabaseState, GitAncestryInference, GitAncestryProbe, GitBranchClass,
    GitHistoryError, GitHistoryGapReason, GitHistoryTransition, GitObjectFormat, GitObjectIdRef,
    GitOperation, GitPartialCloneState, GitRefMovement, GitReplaceRefsState, GitRepositoryKind,
    GitShallowHistoryState, GitSnapshotMetadata, GitSourceLimitations, GitStatusCounts,
    GitSubmoduleState, GitWorktreeState, RepositoryId, GIT_HISTORY_TRANSITIONS_JSON,
};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    schema_version: u32,
    program: String,
    privacy: Privacy,
    object_format: GitObjectFormat,
    repository_id: String,
    cases: Vec<Case>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Privacy {
    synthetic_only: bool,
    user_data_included: bool,
    network_required: bool,
    retains_ref_names: bool,
    reads_object_content: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    id: String,
    description: String,
    previous: State,
    current: State,
    probe: GitAncestryProbe,
    expected: Expected,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct State {
    head: Option<String>,
    branch_class: GitBranchClass,
    shallow_history: GitShallowHistoryState,
    replace_refs: GitReplaceRefsState,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    ref_movement: GitRefMovement,
    ancestry: GitAncestryInference,
    gap: Option<GitHistoryGapReason>,
}

fn corpus() -> Corpus {
    serde_json::from_str(GIT_HISTORY_TRANSITIONS_JSON).expect("transition corpus")
}

fn snapshot(
    repository: &str,
    format: GitObjectFormat,
    head: Option<&str>,
    branch_class: GitBranchClass,
    shallow_history: GitShallowHistoryState,
    replace_refs: GitReplaceRefsState,
) -> GitSnapshotMetadata {
    GitSnapshotMetadata::new(
        RepositoryId::try_from(repository).expect("repository id"),
        GitRepositoryKind::Standard,
        format,
        head.map(|hex| GitObjectIdRef::new(format, hex).expect("object id")),
        None,
        None,
        GitWorktreeState::Clean,
        branch_class,
        GitOperation::Idle,
        GitStatusCounts::new(0, 0, 0, 0).expect("counts"),
        GitSourceLimitations {
            partial_clone: GitPartialCloneState::Full,
            replace_refs,
            shallow_history,
            submodules: GitSubmoduleState::None,
            alternate_object_database: GitAlternateObjectDatabaseState::None,
        },
    )
    .expect("snapshot")
}

fn state_snapshot(corpus: &Corpus, state: &State) -> GitSnapshotMetadata {
    snapshot(
        &corpus.repository_id,
        corpus.object_format,
        state.head.as_deref(),
        state.branch_class,
        state.shallow_history,
        state.replace_refs,
    )
}

#[test]
fn corpus_is_synthetic_and_covers_every_required_history_change() {
    let corpus = corpus();
    assert_eq!(corpus.schema_version, 1);
    assert_eq!(corpus.program, "ghostrace-git-history-transitions-v1");
    assert!(corpus.privacy.synthetic_only);
    assert!(!corpus.privacy.user_data_included);
    assert!(!corpus.privacy.network_required);
    assert!(!corpus.privacy.retains_ref_names);
    assert!(!corpus.privacy.reads_object_content);
    let ids = corpus.cases.iter().map(|case| case.id.as_str()).collect::<Vec<_>>();
    for required in [
        "rebase",
        "reset",
        "force_update",
        "amend",
        "gc",
        "shallow_deepen",
        "worktree_detach",
        "object_loss",
    ] {
        assert!(ids.contains(&required), "missing {required}");
    }
    assert!(corpus.cases.iter().all(|case| !case.description.is_empty()));
}

#[test]
fn every_corpus_case_classifies_as_expected() {
    let corpus = corpus();
    for case in &corpus.cases {
        let previous = state_snapshot(&corpus, &case.previous);
        let current = state_snapshot(&corpus, &case.current);
        let transition =
            GitHistoryTransition::classify(&previous, &current, case.probe).expect(&case.id);
        assert_eq!(transition.ref_movement, case.expected.ref_movement, "{}", case.id);
        assert_eq!(transition.ancestry, case.expected.ancestry, "{}", case.id);
        assert_eq!(transition.gap.as_ref().map(|gap| gap.reason), case.expected.gap, "{}", case.id);
        if let Some(gap) = &transition.gap {
            assert_eq!(gap.last_known.head, previous.head, "{}", case.id);
            assert_eq!(gap.current.head, current.head, "{}", case.id);
            assert_eq!(gap.last_known.branch_class, previous.branch_class, "{}", case.id);
            assert_eq!(gap.current.shallow_history, current.limitations.shallow_history);
        }
    }
}

#[test]
fn ancestry_is_never_inferred_without_a_positive_or_negative_probe_answer() {
    let corpus = corpus();
    for case in &corpus.cases {
        let previous = state_snapshot(&corpus, &case.previous);
        let current = state_snapshot(&corpus, &case.current);
        let transition =
            GitHistoryTransition::classify(&previous, &current, case.probe).expect("classify");
        match transition.ancestry {
            GitAncestryInference::Descendant => {
                assert_eq!(case.probe, GitAncestryProbe::PreviousIsAncestor, "{}", case.id)
            }
            GitAncestryInference::NotDescendant => {
                assert_eq!(case.probe, GitAncestryProbe::PreviousNotAncestor, "{}", case.id)
            }
            GitAncestryInference::Unknown => assert!(transition.gap.is_some(), "{}", case.id),
            GitAncestryInference::NotApplicable => {
                assert_ne!(transition.ref_movement, GitRefMovement::HeadMoved, "{}", case.id)
            }
        }
    }
}

#[test]
fn transitions_serialize_strictly_without_ref_names_or_paths() {
    let corpus = corpus();
    let case = corpus.cases.iter().find(|case| case.id == "rebase").expect("rebase");
    let transition = GitHistoryTransition::classify(
        &state_snapshot(&corpus, &case.previous),
        &state_snapshot(&corpus, &case.current),
        case.probe,
    )
    .expect("classify");
    let json = serde_json::to_value(&transition).expect("JSON");
    let round_trip: GitHistoryTransition = serde_json::from_value(json.clone()).expect("parse");
    assert_eq!(round_trip, transition);
    let mut injected = json.clone();
    injected["ref_name"] = serde_json::json!("refs/heads/secret-branch");
    assert!(serde_json::from_value::<GitHistoryTransition>(injected).is_err());
    let text = json.to_string();
    assert!(!text.contains("refs/"));
    assert!(!text.contains('/'));
}

#[test]
fn mismatched_repositories_or_formats_are_refused() {
    let a = snapshot(
        "git-demo",
        GitObjectFormat::Sha1,
        Some("1111111111111111111111111111111111111111"),
        GitBranchClass::Local,
        GitShallowHistoryState::Complete,
        GitReplaceRefsState::None,
    );
    let other = snapshot(
        "git-other",
        GitObjectFormat::Sha1,
        Some("2222222222222222222222222222222222222222"),
        GitBranchClass::Local,
        GitShallowHistoryState::Complete,
        GitReplaceRefsState::None,
    );
    let sha256 = snapshot(
        "git-demo",
        GitObjectFormat::Sha256,
        Some(&"3".repeat(64)),
        GitBranchClass::Local,
        GitShallowHistoryState::Complete,
        GitReplaceRefsState::None,
    );
    assert_eq!(
        GitHistoryTransition::classify(&a, &other, GitAncestryProbe::NotProbed),
        Err(GitHistoryError::RepositoryMismatch)
    );
    assert_eq!(
        GitHistoryTransition::classify(&a, &sha256, GitAncestryProbe::NotProbed),
        Err(GitHistoryError::ObjectFormatMismatch)
    );
}

/// A reference probe against real, throwaway repositories. It asks Git only
/// the bounded questions a future adapter may ask and keeps only exit status
/// and object IDs; ref names, messages, and paths are never retained.
#[cfg(unix)]
mod real_git {
    use std::{
        path::Path,
        process::{Command, Stdio},
    };

    use super::*;

    fn git(dir: &Path, args: &[&str]) -> std::process::Output {
        Command::new("git")
            .args(args)
            .current_dir(dir)
            .env_clear()
            .env("PATH", "/usr/bin:/bin:/usr/local/bin:/opt/homebrew/bin")
            .env("HOME", dir)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_AUTHOR_NAME", "fixture")
            .env("GIT_AUTHOR_EMAIL", "fixture@example.invalid")
            .env("GIT_COMMITTER_NAME", "fixture")
            .env("GIT_COMMITTER_EMAIL", "fixture@example.invalid")
            .env("GIT_AUTHOR_DATE", "2026-01-01T00:00:00Z")
            .env("GIT_COMMITTER_DATE", "2026-01-01T00:00:00Z")
            .stdin(Stdio::null())
            .output()
            .expect("git runs")
    }

    fn ok(dir: &Path, args: &[&str]) {
        let output = git(dir, args);
        assert!(output.status.success(), "git {args:?} failed");
    }

    fn head(dir: &Path) -> String {
        String::from_utf8(git(dir, &["rev-parse", "HEAD"]).stdout).expect("utf8").trim().to_owned()
    }

    fn probe(dir: &Path, previous: &str, current: &str) -> GitAncestryProbe {
        if !git(dir, &["cat-file", "-e", &format!("{previous}^{{commit}}")]).status.success() {
            return GitAncestryProbe::PreviousObjectMissing;
        }
        match git(dir, &["merge-base", "--is-ancestor", previous, current]).status.code() {
            Some(0) => GitAncestryProbe::PreviousIsAncestor,
            Some(1) => GitAncestryProbe::PreviousNotAncestor,
            _ => GitAncestryProbe::NotProbed,
        }
    }

    fn at(hex: &str) -> GitSnapshotMetadata {
        snapshot(
            "git-real",
            GitObjectFormat::Sha1,
            Some(hex),
            GitBranchClass::Local,
            GitShallowHistoryState::Complete,
            GitReplaceRefsState::None,
        )
    }

    fn classify(dir: &Path, previous: &str, current: &str) -> GitHistoryTransition {
        GitHistoryTransition::classify(&at(previous), &at(current), probe(dir, previous, current))
            .expect("classify")
    }

    fn git_available() -> bool {
        Command::new("git").arg("--version").stdout(Stdio::null()).status().is_ok()
    }

    fn commit(dir: &Path, message: &str) {
        ok(dir, &["commit", "--allow-empty", "-q", "-m", message]);
    }

    #[test]
    fn real_repository_operations_match_the_contract() {
        if !git_available() {
            eprintln!("git is unavailable; skipping the real-repository reference probe");
            return;
        }
        let repo = tempfile::tempdir().expect("repo");
        let dir = repo.path();
        ok(dir, &["init", "-q", "--object-format=sha1"]);
        commit(dir, "one");
        let one = head(dir);

        commit(dir, "two");
        let two = head(dir);
        let forward = classify(dir, &one, &two);
        assert_eq!(forward.ancestry, GitAncestryInference::Descendant);
        assert!(forward.gap.is_none());

        ok(dir, &["commit", "--amend", "--allow-empty", "-q", "-m", "two amended"]);
        let amended = head(dir);
        let amend = classify(dir, &two, &amended);
        assert_eq!(amend.ancestry, GitAncestryInference::NotDescendant);
        assert_eq!(amend.gap.map(|gap| gap.reason), Some(GitHistoryGapReason::HistoryRewritten));

        ok(dir, &["reset", "-q", "--hard", &one]);
        let reset = classify(dir, &amended, &one);
        assert_eq!(reset.gap.map(|gap| gap.reason), Some(GitHistoryGapReason::HistoryRewritten));

        commit(dir, "three");
        let three = head(dir);
        ok(dir, &["reflog", "expire", "--expire=now", "--all"]);
        ok(dir, &["gc", "-q", "--prune=now"]);
        let collected = classify(dir, &amended, &three);
        assert_eq!(collected.ancestry, GitAncestryInference::Unknown);
        assert_eq!(collected.gap.map(|gap| gap.reason), Some(GitHistoryGapReason::ObjectMissing));

        // Detaching at the same commit moves no history.
        ok(dir, &["checkout", "-q", "--detach"]);
        assert_eq!(head(dir), three);
        let detached = GitHistoryTransition::classify(
            &at(&three),
            &snapshot(
                "git-real",
                GitObjectFormat::Sha1,
                Some(&three),
                GitBranchClass::DetachedHead,
                GitShallowHistoryState::Complete,
                GitReplaceRefsState::None,
            ),
            GitAncestryProbe::NotProbed,
        )
        .expect("classify");
        assert_eq!(detached.ref_movement, GitRefMovement::Detached);
        assert!(detached.gap.is_none());
    }
}
