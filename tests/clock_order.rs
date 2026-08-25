use std::{fs, path::PathBuf};

use chrono::{DateTime, Utc};
use ghostrace::{
    analyze_temporal_observations, export_journal, EventKind, EventPayload, EventSource, Evidence,
    ExportManifest, IngestionOrigin, Journal, PolicyProfile, TemporalEvidenceBasis,
    TemporalObservation, ORDERING_CONTRACT_VERSION, TEMPORAL_OBSERVATION_SCHEMA_VERSION,
};
use serde::Deserialize;
use tempfile::tempdir;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
struct TemporalFixture {
    schema_version: u32,
    scenarios: Vec<TemporalScenario>,
}

#[derive(Debug, Deserialize)]
struct TemporalScenario {
    id: String,
    observations: Vec<TemporalObservation>,
}

fn fixture() -> TemporalFixture {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/temporal-ordering-v1.json");
    serde_json::from_str(&fs::read_to_string(path).expect("temporal fixture"))
        .expect("fixture JSON")
}

fn timestamp(value: &str) -> DateTime<Utc> {
    value.parse().expect("timestamp")
}

#[test]
fn temporal_fixture_covers_clock_adjustments_and_missing_source_time() {
    let fixture = fixture();
    assert_eq!(fixture.schema_version, TEMPORAL_OBSERVATION_SCHEMA_VERSION);
    let ids = fixture.scenarios.iter().map(|scenario| scenario.id.as_str()).collect::<Vec<_>>();
    assert_eq!(
        ids,
        vec![
            "clock_rollback",
            "leap_adjustment",
            "sleep",
            "equal_timestamps",
            "delayed_batch",
            "missing_source_time",
        ]
    );

    let rollback = &fixture.scenarios[0].observations;
    let rollback_analysis = analyze_temporal_observations(rollback);
    assert!(rollback_analysis.warnings.iter().any(|warning| warning.contains("clock rollback")));

    let leap = &fixture.scenarios[1].observations;
    assert!(analyze_temporal_observations(leap).warnings.is_empty());
    assert!(leap[0].source_observed_at < leap[1].source_observed_at);

    for scenario in [&fixture.scenarios[2], &fixture.scenarios[4]] {
        let analysis = analyze_temporal_observations(&scenario.observations);
        assert!(analysis.warnings.iter().any(|warning| warning.contains("ingest lag")));
    }

    let equal = &fixture.scenarios[3].observations;
    let equal_analysis = analyze_temporal_observations(equal);
    assert!(equal_analysis.warnings.iter().any(|warning| warning.contains("ingest sequence")));
    assert!(equal_analysis.decisions.iter().any(|decision| decision.ambiguous));

    let missing = &fixture.scenarios[5].observations;
    let missing_analysis = analyze_temporal_observations(missing);
    assert_eq!(missing_analysis.decisions[0].basis, TemporalEvidenceBasis::IngestSequence);
    assert!(missing_analysis
        .warnings
        .iter()
        .any(|warning| warning.contains("source observation time is missing")));
}

#[test]
fn ordering_contract_is_versioned_and_total_with_equal_timestamps() {
    let first = TemporalObservation {
        event_id: Uuid::from_u128(1),
        source_observed_at: Some(timestamp("2026-01-01T00:00:00Z")),
        ingested_at: timestamp("2026-01-01T00:00:01Z"),
        monotonic_sequence: Some(1),
        ingest_seq: 7,
    };
    let second = TemporalObservation {
        event_id: Uuid::from_u128(2),
        source_observed_at: first.source_observed_at,
        ingested_at: timestamp("2026-01-01T00:00:02Z"),
        monotonic_sequence: Some(2),
        ingest_seq: 8,
    };
    assert_eq!(first.stable_order_key().contract_version, ORDERING_CONTRACT_VERSION);
    assert!(first.stable_order_key() < second.stable_order_key());
    assert_eq!(
        analyze_temporal_observations(&[first, second])
            .decisions
            .into_iter()
            .nth(1)
            .expect("decision")
            .basis,
        TemporalEvidenceBasis::IngestSequence
    );
}

#[test]
fn database_and_export_use_the_same_stable_order() {
    let policy = PolicyProfile::fixture_default();
    let journal = Journal::in_memory(ghostrace::DeterministicKeyProvider::from_seed("0078-order"))
        .expect("journal");
    let origin = IngestionOrigin::fixture_instance("fixture-ordering").expect("origin");
    let observed = [
        timestamp("2026-01-01T00:00:02Z"),
        timestamp("2026-01-01T00:00:01Z"),
        timestamp("2026-01-01T00:00:01Z"),
    ];
    for (index, observed_at) in observed.into_iter().enumerate() {
        let event_id = Uuid::from_u128(100 + index as u128);
        let event = ghostrace::EventEnvelope::new(
            &origin,
            event_id,
            observed_at,
            observed_at,
            EventSource::Lifecycle,
            if index == 2 { EventKind::CollectorStopped } else { EventKind::CollectorStarted },
            if index == 2 {
                EventPayload::CollectorStopped(ghostrace::CollectorLifecyclePayload {
                    collector: EventSource::Lifecycle,
                    instance_label: "fixture-ordering".try_into().expect("label"),
                })
            } else {
                EventPayload::CollectorStarted(ghostrace::CollectorLifecyclePayload {
                    collector: EventSource::Lifecycle,
                    instance_label: "fixture-ordering".try_into().expect("label"),
                })
            },
            None,
            policy.id.clone(),
            policy.version,
            Evidence::Direct,
            None,
        )
        .expect("event");
        journal.ingest(&origin, &event, &policy).expect("ingest");
    }

    let ordered = journal.ordered_events().expect("ordered events");
    let ordered_ids = ordered.iter().map(|stored| stored.event.event_id).collect::<Vec<_>>();
    assert_eq!(ordered_ids, vec![Uuid::from_u128(101), Uuid::from_u128(102), Uuid::from_u128(100)]);

    let request = ghostrace::QueryRequest::for_policy(&policy).expect("request");
    let page = journal.query_page(&request, None).expect("query");
    assert_eq!(
        page.events.iter().map(|stored| stored.event.event_id).collect::<Vec<_>>(),
        ordered_ids
    );

    let output = tempdir().expect("tempdir").path().join("ordered.jsonl");
    let manifest: ExportManifest = export_journal(&journal, &output, false).expect("export");
    assert_eq!(manifest.ordering_contract_version, ORDERING_CONTRACT_VERSION);
    let exported_ids = fs::read_to_string(output)
        .expect("export text")
        .lines()
        .skip(1)
        .map(|line| {
            serde_json::from_str::<serde_json::Value>(line).expect("record")["event"]["event_id"]
                .as_str()
                .expect("event id")
                .parse()
                .expect("UUID")
        })
        .collect::<Vec<Uuid>>();
    assert_eq!(exported_ids, ordered_ids);
}

#[test]
fn explanation_labels_ingest_fallback_as_temporal_ambiguity() {
    let policy = PolicyProfile::fixture_default();
    let journal =
        Journal::in_memory(ghostrace::DeterministicKeyProvider::from_seed("0078-explain"))
            .expect("journal");
    let origin = IngestionOrigin::fixture_instance("fixture-explanation-order").expect("origin");
    let observed_at = timestamp("2026-01-01T18:00:00Z");
    let first_id = Uuid::from_u128(201);
    let first = ghostrace::EventEnvelope::new(
        &origin,
        first_id,
        observed_at,
        observed_at,
        EventSource::Lifecycle,
        EventKind::CollectorStarted,
        EventPayload::CollectorStarted(ghostrace::CollectorLifecyclePayload {
            collector: EventSource::Lifecycle,
            instance_label: "fixture-explanation-order".try_into().expect("label"),
        }),
        None,
        policy.id.clone(),
        policy.version,
        Evidence::Direct,
        None,
    )
    .expect("first event");
    journal.ingest(&origin, &first, &policy).expect("first ingest");
    let second_id = Uuid::from_u128(202);
    let second = ghostrace::EventEnvelope::new(
        &origin,
        second_id,
        observed_at,
        observed_at,
        EventSource::Lifecycle,
        EventKind::CollectorStopped,
        EventPayload::CollectorStopped(ghostrace::CollectorLifecyclePayload {
            collector: EventSource::Lifecycle,
            instance_label: "fixture-explanation-order".try_into().expect("label"),
        }),
        None,
        policy.id.clone(),
        policy.version,
        Evidence::Direct,
        Some(first_id),
    )
    .expect("second event");
    journal.ingest(&origin, &second, &policy).expect("second ingest");

    let explanation = ghostrace::explain(&journal, second_id).expect("explanation");
    assert!(explanation.coverage.warnings.iter().any(|warning| {
        warning.contains("temporal ambiguity") && warning.contains("ingest sequence")
    }));
}
