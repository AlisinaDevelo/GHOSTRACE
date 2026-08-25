use chrono::{TimeZone, Utc};
use ghostrace::{
    ConsentPreview, ConsentState, ConsentStateMachine, ConsentTransitionKind, EventSource,
    PolicyDocument,
};

fn policy() -> PolicyDocument {
    PolicyDocument::new(
        "filesystem-consent-v1",
        4,
        [EventSource::Filesystem],
        ["workspace-root", "workspace-secondary"],
        false,
    )
    .expect("policy")
    .with_excluded_roots(["workspace-secondary-excluded"])
    .expect("exclusions")
}

#[test]
fn preview_makes_root_scope_and_known_limits_visible_before_explicit_confirmation() {
    let document = policy();
    let preview = ConsentPreview::from_policy(
        &document,
        ["path_digest", "operation", "entry_kind"],
        ["fsevents_coalescing", "no_process_attribution", "history_can_be_dropped"],
    )
    .expect("preview");

    assert_eq!(
        preview.canonical_roots().iter().map(|root| root.as_str()).collect::<Vec<_>>(),
        vec!["workspace-root", "workspace-secondary"]
    );
    assert_eq!(preview.excluded_roots()[0].as_str(), "workspace-secondary-excluded");
    assert_eq!(preview.retained_fields()[0].as_str(), "entry_kind");
    assert!(preview
        .coverage_limits()
        .iter()
        .any(|limit| limit.as_str() == "no_process_attribution"));
    let rendered = serde_json::to_string(&preview).expect("preview JSON");
    assert!(rendered.contains("workspace-root"));
    assert!(rendered.contains("fsevents_coalescing"));
    assert!(!rendered.contains("/Users/"));
}

#[test]
fn confirmed_preview_binds_receipt_to_policy_without_retaining_scope_names_and_revoke_is_terminal()
{
    let document = policy();
    let expected_digest = document.scope_digest().expect("scope digest");
    let preview = ConsentPreview::from_policy(
        &document,
        ["path_digest", "operation"],
        ["fsevents_coalescing", "no_process_attribution"],
    )
    .expect("preview");
    let confirmation = preview.confirm();
    let mut machine = ConsentStateMachine::new();
    let granted = machine
        .grant_preview(
            confirmation,
            Utc.timestamp_opt(1_750_000_000, 0).single().expect("timestamp"),
            "human",
            "root_opt_in",
        )
        .expect("grant");

    assert_eq!(machine.state(), ConsentState::Active);
    assert_eq!(granted.policy_version, 4);
    assert_eq!(granted.scope_digest, expected_digest);
    let receipt_json = serde_json::to_string(&granted).expect("receipt JSON");
    assert!(!receipt_json.contains("workspace-root"));
    assert!(!receipt_json.contains("path_digest"));
    assert!(!receipt_json.contains("/Users/"));

    let revoked = machine
        .revoke(
            Utc.timestamp_opt(1_750_000_001, 0).single().expect("timestamp"),
            "human",
            "user_revoked",
        )
        .expect("revoke");
    assert_eq!(revoked.transition, ConsentTransitionKind::Revoked);
    assert_eq!(revoked.state, ConsentState::Revoked);
    assert!(revoked.state.is_terminal());
    assert!(!machine.is_capture_allowed());
    assert!(machine
        .revoke(
            Utc.timestamp_opt(1_750_000_002, 0).single().expect("timestamp"),
            "human",
            "second_revoke",
        )
        .is_err());
}

#[test]
fn preview_rejects_empty_scope_or_missing_coverage_limits() {
    let empty = PolicyDocument::new(
        "empty-scope-v1",
        1,
        [EventSource::Filesystem],
        std::iter::empty::<&str>(),
        false,
    )
    .expect("empty policy is valid but cannot be opted in");
    assert!(ConsentPreview::from_policy(&empty, ["path_digest"], ["coalescing"]).is_err());

    let document = policy();
    assert!(ConsentPreview::from_policy(&document, ["path_digest"], std::iter::empty::<&str>(),)
        .is_err());
}
