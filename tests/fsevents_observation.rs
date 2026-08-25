use ghostrace::{EventEnvelope, FilesystemObservation, RenamePairing, EVENT_SCHEMA_JSON};
use serde_json::json;

#[test]
fn observation_contract_is_path_free_and_strict() {
    assert_eq!(
        serde_json::to_value(FilesystemObservation::SourceCoalesced).expect("coalesced JSON"),
        json!("source_coalesced")
    );
    assert_eq!(
        serde_json::to_value(FilesystemObservation::RepeatedModification).expect("repeated JSON"),
        json!("repeated_modification")
    );
    assert_eq!(
        serde_json::to_value(FilesystemObservation::OwnEvent).expect("own JSON"),
        json!("own_event")
    );
    assert_eq!(
        serde_json::to_value(RenamePairing::Unknown).expect("rename JSON"),
        json!("unknown")
    );
    assert!(
        serde_json::from_value::<FilesystemObservation>(json!("transport_duplicate")).is_err(),
        "transport duplicates are receipt counters, never persisted filesystem events"
    );
    assert!(
        serde_json::from_value::<RenamePairing>(json!("inferred")).is_err(),
        "rename pairing never claims an inferred old-to-new path"
    );
}

#[test]
fn observation_fields_round_trip_through_strict_event_schema() {
    let mut event: serde_json::Value = include_str!("../fixtures/causal-chain.jsonl")
        .lines()
        .nth(3)
        .expect("filesystem fixture")
        .parse()
        .expect("filesystem JSON");
    event["payload"]["data"]["operation"] = json!("renamed");
    event["payload"]["data"]["observation"] = json!("source_coalesced");
    event["payload"]["data"]["rename_pairing"] = json!("unknown");

    let schema: serde_json::Value = serde_json::from_str(EVENT_SCHEMA_JSON).expect("schema JSON");
    let validator = jsonschema::options().build(&schema).expect("valid event schema");
    assert!(validator.is_valid(&event), "schema rejected observation fields: {event}");
    let decoded: EventEnvelope = serde_json::from_value(event.clone()).expect("typed event");
    assert_eq!(serde_json::to_value(decoded).expect("event JSON"), event);

    event["payload"]["data"]["observation"] = json!("transport_duplicate");
    assert!(!validator.is_valid(&event));
    assert!(serde_json::from_value::<EventEnvelope>(event.clone()).is_err());

    event["payload"]["data"]["observation"] = json!("source_coalesced");
    event["payload"]["data"]["rename_pairing"] = json!("inferred");
    assert!(!validator.is_valid(&event));
    assert!(serde_json::from_value::<EventEnvelope>(event).is_err());
}

#[test]
fn rename_pairing_requires_a_renamed_operation() {
    let mut event: serde_json::Value = include_str!("../fixtures/causal-chain.jsonl")
        .lines()
        .nth(3)
        .expect("filesystem fixture")
        .parse()
        .expect("filesystem JSON");
    event["payload"]["data"]["rename_pairing"] = json!("unknown");
    assert!(serde_json::from_value::<EventEnvelope>(event).is_err());
}
