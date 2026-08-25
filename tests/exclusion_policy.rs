use ghostrace::{
    EntryKind, ExclusionAction, ExclusionDecision, ExclusionKind, ExclusionPolicy,
    ExclusionPolicyHistory, ExclusionReason, ExclusionRule, ExclusionSubject,
    EXCLUSION_POLICY_SCHEMA_JSON, MAX_EXCLUSION_PATTERN_BYTES, MAX_EXCLUSION_RULES,
};
use serde_json::json;

fn subject(root: &str) -> ExclusionSubject<'_> {
    ExclusionSubject::new(root)
}

fn assert_decision(
    decision: ExclusionDecision,
    action: ExclusionAction,
    kind: Option<ExclusionKind>,
    reason: ExclusionReason,
) {
    assert_eq!(decision.action, action);
    assert_eq!(decision.matched_kind, kind);
    assert_eq!(decision.reason, reason);
}

#[test]
fn precedence_is_order_independent_and_safety_first() {
    let rules = vec![
        ExclusionRule::root("root-a", ExclusionAction::Allow).expect("root rule"),
        ExclusionRule::subtree("build", ExclusionAction::Deny).expect("subtree rule"),
        ExclusionRule::file_kind(EntryKind::Symlink, ExclusionAction::Redact),
        ExclusionRule::application("com.example.*", ExclusionAction::Summarize)
            .expect("application rule"),
        ExclusionRule::temporary_file(ExclusionAction::Allow),
        ExclusionRule::vcs(ExclusionAction::Deny),
        ExclusionRule::user("root-a/build/*", ExclusionAction::Allow).expect("user rule"),
    ];
    let policy = ExclusionPolicy::with_rules(7, rules.clone()).expect("policy");
    let reversed = ExclusionPolicy::with_rules(7, rules.into_iter().rev()).expect("reversed");

    let nested = subject("root-a").with_relative_path("BUILD/cache");
    let decision = policy.evaluate(nested);
    assert_decision(
        decision,
        ExclusionAction::Deny,
        Some(ExclusionKind::Subtree),
        ExclusionReason::Subtree,
    );
    assert_eq!(decision, reversed.evaluate(nested));

    let symlink = subject("root-a")
        .with_relative_path("source/file")
        .with_file_kind(EntryKind::Symlink)
        .with_application("com.example.editor");
    assert_decision(
        policy.evaluate(symlink),
        ExclusionAction::Redact,
        Some(ExclusionKind::FileKind),
        ExclusionReason::FileKind,
    );

    let vcs = subject("root-a").with_relative_path("source").temporary_file().vcs();
    assert_decision(
        policy.evaluate(vcs),
        ExclusionAction::Deny,
        Some(ExclusionKind::Vcs),
        ExclusionReason::Vcs,
    );

    let root_only = policy.evaluate(subject("root-a"));
    assert_decision(
        root_only,
        ExclusionAction::Allow,
        Some(ExclusionKind::Root),
        ExclusionReason::Root,
    );
    let unmatched = policy.evaluate(subject("root-b").with_relative_path("notes/readme"));
    assert_decision(unmatched, ExclusionAction::Allow, None, ExclusionReason::NoMatch);
    assert_eq!(unmatched.reason_code(), "no_exclusion_match");
}

#[test]
fn nested_case_variant_and_escaped_patterns_are_bounded() {
    let nested = ExclusionPolicy::with_rules(
        1,
        [ExclusionRule::subtree("Build/Cache", ExclusionAction::Deny).expect("subtree")],
    )
    .expect("policy");
    assert_eq!(
        nested.evaluate(subject("root-a").with_relative_path("BUILD/cache/deep/file")).action,
        ExclusionAction::Deny
    );

    let literal_star = ExclusionPolicy::with_rules(
        1,
        [ExclusionRule::subtree(r"build/\*", ExclusionAction::Deny).expect("escaped subtree")],
    )
    .expect("policy");
    assert_eq!(
        literal_star.evaluate(subject("root-a").with_relative_path("build/*")).action,
        ExclusionAction::Deny
    );
    assert_eq!(
        literal_star.evaluate(subject("root-a").with_relative_path("build/cache")).action,
        ExclusionAction::Allow
    );

    assert!(ExclusionRule::user("", ExclusionAction::Deny).is_err());
    assert!(ExclusionRule::subtree("../private", ExclusionAction::Deny).is_err());
    assert!(ExclusionRule::subtree("dangling\\", ExclusionAction::Deny).is_err());
    assert!(ExclusionRule::subtree(r"escaped\x", ExclusionAction::Deny).is_err());
    assert!(ExclusionRule::subtree(
        "x".repeat(MAX_EXCLUSION_PATTERN_BYTES + 1),
        ExclusionAction::Deny
    )
    .is_err());

    let invalid = nested.evaluate(subject("root-a").with_relative_path("../private/secret"));
    assert_decision(invalid, ExclusionAction::Deny, None, ExclusionReason::InvalidSubject);
    assert!(!serde_json::to_string(&invalid).expect("decision JSON").contains("private"));
}

#[test]
fn version_updates_apply_to_future_only_and_preserve_recorded_version() {
    let first = ExclusionPolicy::with_rules(
        1,
        [ExclusionRule::subtree("cache", ExclusionAction::Deny).expect("first rule")],
    )
    .expect("first policy");
    let mut history = ExclusionPolicyHistory::new(first).expect("history");
    let cache = subject("root-a").with_relative_path("cache/object");
    let recorded = history.evaluate_future(cache);
    assert_eq!(recorded.policy_version, 1);
    assert_eq!(recorded.action, ExclusionAction::Deny);

    let second = ExclusionPolicy::new(2).expect("second policy");
    history.install(second).expect("future policy");
    assert_eq!(history.evaluate_future(cache).action, ExclusionAction::Allow);
    assert_eq!(history.evaluate_future(cache).policy_version, 2);
    assert_eq!(history.evaluate_recorded(1, cache).expect("old policy"), recorded);
    assert!(history.evaluate_recorded(99, cache).is_err());
    assert!(history.install(ExclusionPolicy::new(2).expect("duplicate version")).is_err());
    assert!(history.install(ExclusionPolicy::new(1).expect("downgrade")).is_err());
}

#[test]
fn serialization_schema_digest_and_property_corpus_are_deterministic() {
    let first = ExclusionPolicy::with_rules(
        4,
        [
            ExclusionRule::user("root-a/tmp/*", ExclusionAction::Redact).expect("user"),
            ExclusionRule::vcs(ExclusionAction::Deny),
        ],
    )
    .expect("policy");
    let second = ExclusionPolicy::with_rules(
        4,
        [
            ExclusionRule::vcs(ExclusionAction::Deny),
            ExclusionRule::user("root-a/tmp/*", ExclusionAction::Redact).expect("user"),
        ],
    )
    .expect("reordered policy");
    assert_eq!(first.scope_digest().expect("digest"), second.scope_digest().expect("digest"));
    let schema: serde_json::Value =
        serde_json::from_str(EXCLUSION_POLICY_SCHEMA_JSON).expect("schema");
    let validator = jsonschema::options().build(&schema).expect("valid schema");
    let value: serde_json::Value =
        serde_json::from_str(&first.to_json().expect("policy JSON")).expect("policy value");
    assert!(validator.is_valid(&value));
    assert_eq!(ExclusionPolicy::from_json(&value.to_string()).expect("round trip"), first);

    // The matcher has bounded input and no backtracking. Exercise the largest
    // legal pattern/subject shape repeatedly without measuring wall-clock time.
    let long_pattern = format!("{}*", "a".repeat(MAX_EXCLUSION_PATTERN_BYTES - 1));
    let policy = ExclusionPolicy::with_rules(
        5,
        [ExclusionRule::user(long_pattern, ExclusionAction::Deny).expect("bounded pattern")],
    )
    .expect("bounded policy");
    let long_path = "a".repeat(900);
    for _ in 0..512 {
        let decision = policy.evaluate(subject("root-a").with_relative_path(&long_path));
        assert!(matches!(decision.action, ExclusionAction::Deny | ExclusionAction::Allow));
    }

    let too_many =
        (0..=MAX_EXCLUSION_RULES).map(|_| ExclusionRule::temporary_file(ExclusionAction::Deny));
    assert!(ExclusionPolicy::with_rules(1, too_many).is_err());
    let unknown = json!({"schema_version": 1, "version": 1, "rules": [], "extra": true});
    assert!(ExclusionPolicy::from_json(&unknown.to_string()).is_err());
}
