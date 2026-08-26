use std::{fs, path::PathBuf};

use chrono::{TimeZone, Utc};
use ghostrace::{
    ingest_fixture, CollectorLifecyclePayload, DeterministicKeyProvider, EventEnvelope, EventKind,
    EventPayload, EventSource, Evidence, IngestionOrigin, Journal, PolicyProfile, RetentionPolicy,
    RetentionSelectionReason, RootId,
};
use tempfile::tempdir;
use uuid::Uuid;

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/causal-chain.jsonl")
}

fn journal(seed: &str) -> (tempfile::TempDir, Journal, PolicyProfile) {
    let directory = tempdir().expect("directory");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
            .expect("private directory");
    }
    let path = directory.path().join("journal.sqlite3");
    let policy = PolicyProfile::fixture_default();
    let journal =
        Journal::open_fixture(&path, DeterministicKeyProvider::from_seed(seed)).expect("journal");
    ingest_fixture(fixture_path(), &journal, &policy).expect("fixture ingest");
    (directory, journal, policy)
}

fn timestamp(seconds: i64) -> chrono::DateTime<Utc> {
    Utc.timestamp_opt(seconds, 0).single().expect("timestamp")
}

#[test]
fn default_retention_window_is_explicitly_ninety_days() {
    let as_of = timestamp(1_767_225_608);
    let policy = RetentionPolicy::default_at(as_of);
    assert_eq!(policy.before, Some(as_of - chrono::Duration::days(90)));
    assert!(policy.preserve_gaps);
}

fn lifecycle_event(id: u128, seconds: i64, policy: &PolicyProfile) -> EventEnvelope {
    lifecycle_event_with_cursor(id, seconds, policy, "cursor-retention", None)
}

fn lifecycle_event_with_cursor(
    id: u128,
    seconds: i64,
    policy: &PolicyProfile,
    cursor: &str,
    parent_event_id: Option<Uuid>,
) -> EventEnvelope {
    let origin = IngestionOrigin::fixture_instance("fixture-retention").expect("origin");
    EventEnvelope::new(
        &origin,
        Uuid::from_u128(id),
        timestamp(seconds),
        timestamp(seconds),
        EventSource::Lifecycle,
        EventKind::CollectorStarted,
        EventPayload::CollectorStarted(CollectorLifecyclePayload {
            collector: EventSource::Lifecycle,
            instance_label: "retention-test".try_into().expect("label"),
        }),
        Some(cursor.try_into().expect("cursor")),
        policy.id.clone(),
        policy.version,
        Evidence::Direct,
        parent_event_id,
    )
    .expect("event")
}

#[test]
fn dry_run_reports_scope_ranges_sources_generations_gaps_and_space() {
    let (_directory, journal, _policy) = journal("retention-report");
    let policy = RetentionPolicy::before(timestamp(1_767_225_608));
    let plan = journal.retention_plan(&policy).expect("retention plan");

    assert_eq!(plan.snapshot_event_count, 8);
    assert_eq!(plan.scoped_event_count, 8);
    assert_eq!(plan.eligible_event_count, 7, "the gap is protected by default");
    assert_eq!(plan.affected_event_count, 7);
    assert_eq!(plan.affected_gap_count, 0);
    assert_eq!(plan.protected_gap_count, 1);
    assert_eq!(plan.affected_key_generations.get(&1), Some(&7));
    assert_eq!(plan.affected_sources.get(&EventSource::Lifecycle), Some(&2));
    assert_eq!(plan.affected_sources.get(&EventSource::Browser), Some(&2));
    assert!(plan.affected_observed_from.is_some());
    assert!(plan.affected_observed_until.is_some());
    assert!(plan.estimated_reclaimed_bytes > 0);
    assert_eq!(plan.selection_reasons.get(&RetentionSelectionReason::Time), Some(&7));
    assert!(plan.legal_hold_is_not_supported());
    plan.validate().expect("plan validates");
    plan.confirm().validate().expect("confirmation validates");
}

#[test]
fn source_and_root_scope_are_intersected_before_limits() {
    let (_directory, journal, _policy) = journal("retention-scope");
    let mut policy = RetentionPolicy::before(timestamp(1_767_225_608));
    policy.source = Some(EventSource::Filesystem);
    policy.root_id = Some(RootId::try_from("workspace-demo").expect("root"));
    let plan = journal.retention_plan(&policy).expect("retention plan");

    assert_eq!(plan.scoped_event_count, 1);
    assert_eq!(plan.eligible_event_count, 1);
    assert_eq!(plan.affected_event_count, 1);
    assert_eq!(plan.affected_sources, [(EventSource::Filesystem, 1)].into_iter().collect());
}

#[test]
fn count_and_byte_limits_select_oldest_rows_with_explicit_reason_precedence() {
    let (_directory, journal, _policy) = journal("retention-limits");
    let count_policy =
        RetentionPolicy { retain_at_most_events: Some(3), ..RetentionPolicy::default() };
    let count_plan = journal.retention_plan(&count_policy).expect("count plan");
    assert_eq!(count_plan.eligible_event_count, 7);
    assert_eq!(count_plan.affected_event_count, 4);
    assert_eq!(count_plan.selection_reasons.get(&RetentionSelectionReason::EventCount), Some(&4));

    let byte_policy =
        RetentionPolicy { retain_at_most_bytes: Some(1_000), ..RetentionPolicy::default() };
    let byte_plan = journal.retention_plan(&byte_policy).expect("byte plan");
    assert!(byte_plan.affected_event_count > 0);
    assert!(byte_plan.estimated_reclaimed_bytes > 0);
    assert!(byte_plan.selection_reasons.contains_key(&RetentionSelectionReason::ByteLimit));
}

#[test]
fn confirmation_binds_snapshot_and_policy_without_scope_expansion() {
    let (_directory, journal, policy) = journal("retention-confirmation");
    let retention_policy = RetentionPolicy::before(timestamp(1_767_225_608));
    let first = journal.retention_plan(&retention_policy).expect("first plan");
    let confirmation = first.confirm();
    confirmation.validate().expect("confirmation");
    assert!(first.matches_confirmation(&confirmation));

    journal
        .ingest(
            &IngestionOrigin::fixture_instance("fixture-retention").expect("origin"),
            &lifecycle_event(0x9_999, 1_767_225_000, &policy),
            &policy,
        )
        .expect("concurrent ingest");
    let second = journal.retention_plan(&retention_policy).expect("second plan");

    assert_eq!(confirmation.snapshot_boundary, first.snapshot_boundary);
    assert_eq!(confirmation.plan_digest, first.plan_digest);
    assert!(second.snapshot_boundary > first.snapshot_boundary);
    assert!(second.affected_event_count > first.affected_event_count);
    assert_ne!(second.plan_digest, first.plan_digest);
    assert!(!second.matches_confirmation(&confirmation));

    let changed_policy = RetentionPolicy::before(timestamp(1_767_225_601));
    let changed = journal.retention_plan(&changed_policy).expect("changed plan");
    assert_ne!(changed.plan_digest, first.plan_digest);
}

#[test]
fn invalid_policy_cannot_plan_all_rows_implicitly_or_mix_root_sources() {
    let (_directory, journal, _policy) = journal("retention-invalid");
    let empty = RetentionPolicy::default();
    assert!(journal.retention_plan(&empty).is_err());

    let mut invalid = RetentionPolicy::before(timestamp(1_767_225_608));
    invalid.source = Some(EventSource::Browser);
    invalid.root_id = Some(RootId::try_from("workspace-demo").expect("root"));
    assert!(journal.retention_plan(&invalid).is_err());
}

#[test]
fn deletion_requires_the_exact_confirmation_and_removes_only_selected_rows() {
    let journal = Journal::in_memory(DeterministicKeyProvider::from_seed("retention-delete"))
        .expect("journal");
    let policy = PolicyProfile::fixture_default();
    let origin = IngestionOrigin::fixture_instance("fixture-retention").expect("origin");
    for (index, seconds) in [(1_u128, 100_i64), (2, 200), (3, 300)] {
        journal
            .ingest(
                &origin,
                &lifecycle_event_with_cursor(
                    index,
                    seconds,
                    &policy,
                    &format!("seq-0-{index}"),
                    None,
                ),
                &policy,
            )
            .expect("event");
    }
    let plan = journal.retention_plan(&RetentionPolicy::before(timestamp(250))).expect("plan");
    assert_eq!(plan.affected_event_count, 2);
    let receipt = journal.delete_retention(&plan, &plan.confirm()).expect("delete");
    assert_eq!(receipt.requested_event_count, 2);
    assert_eq!(receipt.deleted_event_count, 2);
    assert_eq!(receipt.remaining_event_count, 1);
    assert!(!receipt.compaction_performed);
    assert!(receipt.external_copies_untouched);
    receipt.validate().expect("valid deletion receipt");
    assert_eq!(journal.events().expect("events").len(), 1);
    assert!(journal.integrity_check().expect("integrity").integrity_ok);

    let stale = journal.delete_retention(&plan, &plan.confirm());
    assert!(matches!(stale, Err(ghostrace::GhostraceError::RetentionConfirmationMismatch)));
}

#[test]
fn deletion_refuses_an_unselected_child_without_partial_mutation() {
    let journal = Journal::in_memory(DeterministicKeyProvider::from_seed("retention-parent"))
        .expect("journal");
    let policy = PolicyProfile::fixture_default();
    let origin = IngestionOrigin::fixture_instance("fixture-retention").expect("origin");
    let parent = lifecycle_event_with_cursor(11, 100, &policy, "seq-0-1", None);
    journal.ingest(&origin, &parent, &policy).expect("parent");
    let child = lifecycle_event_with_cursor(12, 300, &policy, "seq-0-2", Some(parent.event_id));
    journal.ingest(&origin, &child, &policy).expect("child");
    let plan = journal.retention_plan(&RetentionPolicy::before(timestamp(250))).expect("plan");
    assert_eq!(plan.affected_event_count, 1);
    let error = journal.delete_retention(&plan, &plan.confirm()).expect_err("child refusal");
    assert!(matches!(error, ghostrace::GhostraceError::RetentionDeletionRefused(_)));
    assert_eq!(journal.events().expect("events").len(), 2);
}

#[test]
fn integrity_report_is_bounded_and_supplies_recovery_guidance() {
    let (_directory, journal, _policy) = journal("retention-integrity");
    let report = journal.integrity_check().expect("integrity report");
    assert!(report.integrity_ok);
    assert_eq!(report.schema_version, 1);
    assert_eq!(report.user_version, 5);
    assert_eq!(report.migration_count, 6);
    assert_eq!(report.integrity_messages, vec!["ok"]);
    assert!(report.foreign_key_violations.is_empty());
    assert_eq!(report.recovery_guidance.len(), 4);
    report.validate().expect("valid report");
}
