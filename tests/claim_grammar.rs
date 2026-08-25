use std::path::PathBuf;

use ghostrace::{
    explain, read_fixture, render_claim, ClaimLocale, ClaimTemplateId, DeterministicKeyProvider,
    EventKind, EventPayload, Evidence, FileOperation, GapBehavior, Journal, PolicyProfile,
    ProhibitedImplication, RequiredFact, CLAIM_GRAMMAR_VERSION,
};

fn fixture() -> Vec<ghostrace::EventEnvelope> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/causal-chain.jsonl");
    read_fixture(&path).expect("causal fixture")
}

fn template_ids() -> [ClaimTemplateId; 12] {
    [
        ClaimTemplateId::FilesystemObservation,
        ClaimTemplateId::FrontmostObservation,
        ClaimTemplateId::ShellStartedObservation,
        ClaimTemplateId::ShellFinishedObservation,
        ClaimTemplateId::GitSnapshotObservation,
        ClaimTemplateId::BrowserNavigationObservation,
        ClaimTemplateId::BrowserBookmarkObservation,
        ClaimTemplateId::CollectorStartedObservation,
        ClaimTemplateId::CollectorStoppedObservation,
        ClaimTemplateId::CoverageGap,
        ClaimTemplateId::PolicyDenied,
        ClaimTemplateId::SourceError,
    ]
}

#[test]
fn every_template_declares_facts_prohibitions_evidence_and_gap_behavior() {
    for id in template_ids() {
        let descriptor = id.descriptor();
        assert_eq!(descriptor.id, id);
        assert!(descriptor.required_facts.contains(&RequiredFact::EventId));
        assert!(descriptor.required_facts.contains(&RequiredFact::Source));
        assert!(descriptor.required_facts.contains(&RequiredFact::EventKind));
        assert!(descriptor.required_facts.contains(&RequiredFact::ObservedAt));
        assert!(descriptor.required_facts.contains(&RequiredFact::EvidenceLevel));
        assert!(descriptor.required_facts.contains(&RequiredFact::NormalizedPayload));
        assert!(descriptor.prohibited_implications.contains(&ProhibitedImplication::Intent));
        assert!(descriptor.prohibited_implications.contains(&ProhibitedImplication::Completeness));
        assert!(descriptor
            .prohibited_implications
            .contains(&ProhibitedImplication::ProcessAttribution));
    }
    assert!(ClaimTemplateId::FilesystemObservation
        .descriptor()
        .prohibited_implications
        .contains(&ProhibitedImplication::RenameIdentity));
}

#[test]
fn rendering_preserves_ids_evidence_and_localized_meaning() {
    for event in fixture() {
        let english = render_claim(&event, ClaimLocale::En, false).expect("English claim");
        let british = render_claim(&event, ClaimLocale::EnGb, false).expect("English UK claim");
        assert_eq!(english.grammar_version, CLAIM_GRAMMAR_VERSION);
        assert_eq!(english.template, ClaimTemplateId::for_kind(event.kind));
        assert_eq!(english.evidence, event.evidence);
        assert_eq!(english.citations, vec![event.event_id]);
        assert_eq!(british.grammar_version, english.grammar_version);
        assert_eq!(british.template, english.template);
        assert_eq!(british.evidence, english.evidence);
        assert_eq!(british.citations, english.citations);
        assert_eq!(english.locale.tag(), "en");
        assert_eq!(british.locale.tag(), "en-GB");
        assert!(!english.text.is_empty());
        assert!(!british.text.is_empty());
        assert!(english.text.contains(&event.event_id.to_string()));
        assert!(british.text.contains(&event.event_id.to_string()));
        assert!(!english.text.to_ascii_lowercase().contains("caused"));
        assert!(!english.text.to_ascii_lowercase().contains("intent"));
        assert!(!english.text.to_ascii_lowercase().contains("complete history"));
        assert!(!english.text.to_ascii_lowercase().contains("process attribution"));
        assert!(!english.text.contains("fixture_secret"));
        assert!(!british.text.contains("fixture_secret"));
    }
}

#[test]
fn rename_claim_never_invents_old_to_new_identity() {
    let mut event = fixture()
        .into_iter()
        .find(|event| event.kind == EventKind::FilesystemChanged)
        .expect("filesystem event");
    if let EventPayload::FilesystemChanged(payload) = &mut event.payload {
        payload.operation = FileOperation::Renamed;
    } else {
        panic!("expected filesystem payload");
    }
    let claim = render_claim(&event, ClaimLocale::En, false).expect("rename claim");
    assert_eq!(claim.template, ClaimTemplateId::FilesystemObservation);
    assert!(claim.text.contains("not established"));
    assert!(!claim.text.to_ascii_lowercase().contains("renamed from"));
    assert!(!claim.text.to_ascii_lowercase().contains("renamed to"));
    assert!(!claim.text.to_ascii_lowercase().contains("old path"));
    assert!(!claim.text.to_ascii_lowercase().contains("new path"));
}

#[test]
fn gap_behavior_is_explicit_and_explanation_uses_the_grammar() {
    let events = fixture();
    let ordinary = events
        .iter()
        .find(|event| event.kind == EventKind::FilesystemChanged)
        .expect("filesystem event");
    let limited = render_claim(ordinary, ClaimLocale::En, true).expect("limited claim");
    assert_eq!(limited.gap_behavior, GapBehavior::LimitInterpretation);
    assert!(limited.gap_limited);
    assert!(limited.text.contains("recorded gap"));

    let gap = events.iter().find(|event| event.kind == EventKind::Gap).expect("gap event");
    let explicit = render_claim(gap, ClaimLocale::En, true).expect("gap claim");
    assert_eq!(explicit.gap_behavior, GapBehavior::ExplicitStatus);
    assert!(!explicit.gap_limited);

    let journal =
        Journal::in_memory(DeterministicKeyProvider::from_seed("claim-grammar")).expect("journal");
    ghostrace::ingest_fixture(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/causal-chain.jsonl"),
        &journal,
        &PolicyProfile::fixture_default(),
    )
    .expect("ingest");
    let explanation =
        explain(&journal, "00000000-0000-4000-8000-000000000008".parse().expect("target"))
            .expect("explanation");
    assert!(explanation
        .statements
        .iter()
        .all(|statement| statement.grammar_version == CLAIM_GRAMMAR_VERSION));
    assert!(explanation.statements.iter().any(|statement| {
        statement.evidence == Evidence::Unknown
            && statement.gap_behavior == GapBehavior::ExplicitStatus
    }));
    assert!(explanation.statements.iter().any(|statement| statement.gap_limited));
}
