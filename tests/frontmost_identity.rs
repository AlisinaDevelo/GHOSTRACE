//! Frontmost-application identity and session semantics.

use ghostrace::{
    FrontmostApp, FrontmostAppKind, FrontmostAppLocation, FrontmostBasis, FrontmostNormalizer,
    FrontmostObservation, FrontmostRawObservation, FrontmostRecord, FrontmostSessionTracker,
    FrontmostSigningIdentity, FrontmostTransition, FrontmostUnknownReason,
    FRONTMOST_IDENTITY_CORPUS_JSON, FRONTMOST_SCHEMA_JSON, FRONTMOST_TRANSIENT_DWELL_MS,
};
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    schema_version: u32,
    program: String,
    privacy: Privacy,
    salt_hex: String,
    transient_dwell_ms: u64,
    identity_cases: Vec<IdentityCase>,
    session_sequences: Vec<Sequence>,
    rejected_inputs: Vec<Rejected>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Privacy {
    synthetic_only: bool,
    user_data_included: bool,
    network_required: bool,
    retains_titles_or_documents: bool,
    retains_process_ids: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IdentityCase {
    id: String,
    raw: Value,
    expected: ExpectedIdentity,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExpectedIdentity {
    identity: String,
    #[serde(default)]
    reason: Option<FrontmostUnknownReason>,
    #[serde(default)]
    bundle_id: Option<String>,
    #[serde(default)]
    signing_class: Option<String>,
    #[serde(default)]
    team_id: Option<String>,
    #[serde(default)]
    kind: Option<FrontmostAppKind>,
    #[serde(default)]
    location: Option<FrontmostAppLocation>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Sequence {
    id: String,
    steps: Vec<(Value, ExpectedStep)>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExpectedStep {
    emitted: bool,
    #[serde(default)]
    dwell_ms: Option<u64>,
    #[serde(default)]
    transient: Option<bool>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Rejected {
    id: String,
    raw: Value,
    sentinel: String,
}

fn corpus() -> Corpus {
    serde_json::from_str(FRONTMOST_IDENTITY_CORPUS_JSON).expect("frontmost corpus")
}

fn salt(corpus: &Corpus) -> [u8; 32] {
    let mut salt = [0u8; 32];
    for (index, byte) in salt.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&corpus.salt_hex[index * 2..index * 2 + 2], 16).expect("hex");
    }
    salt
}

fn raw(value: &Value) -> FrontmostRawObservation {
    FrontmostRawObservation::parse(&value.to_string()).expect("raw observation")
}

fn validator() -> jsonschema::Validator {
    let schema: Value = serde_json::from_str(FRONTMOST_SCHEMA_JSON).expect("schema");
    jsonschema::options().should_validate_formats(true).build(&schema).expect("valid schema")
}

fn signing_class(signing: &FrontmostSigningIdentity) -> (&'static str, Option<&str>) {
    match signing {
        FrontmostSigningIdentity::Developer { team_id } => ("developer", Some(team_id.as_str())),
        FrontmostSigningIdentity::Platform => ("platform", None),
        FrontmostSigningIdentity::AdHoc => ("ad_hoc", None),
        FrontmostSigningIdentity::Unsigned => ("unsigned", None),
        FrontmostSigningIdentity::Unknown => ("unknown", None),
    }
}

#[test]
fn corpus_is_synthetic_and_covers_required_application_classes() {
    let corpus = corpus();
    assert_eq!(corpus.schema_version, 1);
    assert_eq!(corpus.program, "ghostrace-frontmost-identity-v1");
    assert!(corpus.privacy.synthetic_only);
    assert!(!corpus.privacy.user_data_included);
    assert!(!corpus.privacy.network_required);
    assert!(!corpus.privacy.retains_titles_or_documents);
    assert!(!corpus.privacy.retains_process_ids);
    assert_eq!(corpus.transient_dwell_ms, FRONTMOST_TRANSIENT_DWELL_MS);
    let ids = corpus.identity_cases.iter().map(|case| case.id.as_str()).collect::<Vec<_>>();
    for required in [
        "unsigned_bundle",
        "translocated_download",
        "helper_accessory",
        "command_line_tool",
        "bundle_without_identity",
        "no_process",
    ] {
        assert!(ids.contains(&required), "missing {required}");
    }
    assert!(corpus.session_sequences.iter().any(|sequence| sequence.id == "rapid_switching"));
}

#[test]
fn identity_cases_normalize_to_their_expected_outcomes() {
    let corpus = corpus();
    let normalizer = FrontmostNormalizer::new(salt(&corpus));
    for case in &corpus.identity_cases {
        let app = normalizer.normalize(&raw(&case.raw));
        let expected = &case.expected;
        match (&app, expected.identity.as_str()) {
            (
                FrontmostApp::Known { bundle_id, signing, kind, location, launch_instance, .. },
                "known",
            ) => {
                assert_eq!(
                    bundle_id.as_ref().map(|id| id.as_str().to_owned()),
                    expected.bundle_id,
                    "{}",
                    case.id
                );
                let (class, team) = signing_class(signing);
                assert_eq!(Some(class), expected.signing_class.as_deref(), "{}", case.id);
                assert_eq!(team, expected.team_id.as_deref(), "{}", case.id);
                assert_eq!(Some(*kind), expected.kind, "{}", case.id);
                assert_eq!(Some(*location), expected.location, "{}", case.id);
                assert!(launch_instance.as_str().starts_with("sha256:"));
            }
            (FrontmostApp::Unknown { reason }, "unknown") => {
                assert_eq!(Some(*reason), expected.reason, "{}", case.id)
            }
            _ => panic!("{} normalized to the wrong identity class", case.id),
        }
    }
}

#[test]
fn session_sequences_produce_bounded_dwell_and_transient_marks() {
    let corpus = corpus();
    let validator = validator();
    for sequence in &corpus.session_sequences {
        let mut tracker = FrontmostSessionTracker::new(FrontmostNormalizer::new(salt(&corpus)));
        for (index, (step, expected)) in sequence.steps.iter().enumerate() {
            let records = tracker.observe(&raw(step));
            assert_eq!(!records.is_empty(), expected.emitted, "{} step {index}", sequence.id);
            let Some(FrontmostRecord::App(observation)) = records.last().cloned() else { continue };
            assert_eq!(observation.dwell_ms, expected.dwell_ms, "{} step {index}", sequence.id);
            assert_eq!(
                Some(observation.transient),
                expected.transient,
                "{} step {index}",
                sequence.id
            );
            let json = serde_json::to_value(&observation).expect("observation JSON");
            assert!(validator.is_valid(&json), "{} step {index}", sequence.id);
            let round_trip: FrontmostObservation = serde_json::from_value(json).expect("parse");
            assert_eq!(round_trip, observation);
        }
    }
}

#[test]
fn titles_documents_urls_accessibility_menus_and_screens_are_rejected_without_echo() {
    let corpus = corpus();
    assert!(corpus.rejected_inputs.len() >= 6);
    for case in &corpus.rejected_inputs {
        let error = FrontmostRawObservation::parse(&case.raw.to_string())
            .expect_err(&format!("{} must be rejected", case.id));
        assert!(!error.to_string().contains(&case.sentinel), "{}", case.id);
    }
}

#[test]
fn launch_instances_are_salted_and_never_expose_the_process_id() {
    let corpus = corpus();
    let case = &corpus.identity_cases[0];
    let observation = raw(&case.raw);
    let first = FrontmostNormalizer::new([1; 32]).normalize(&observation);
    let again = FrontmostNormalizer::new([1; 32]).normalize(&observation);
    let other_salt = FrontmostNormalizer::new([2; 32]).normalize(&observation);
    assert_eq!(first, again);
    assert_ne!(first, other_salt);

    let mut relaunched = observation.clone();
    relaunched.process_started_micros += 1;
    assert_ne!(FrontmostNormalizer::new([1; 32]).normalize(&relaunched), first);

    let text = serde_json::to_string(&first).expect("JSON");
    assert!(!text.contains(&observation.process_id.to_string()));
    assert!(!text.contains(&observation.process_started_micros.to_string()));
}

#[test]
fn every_normalized_identity_validates_against_the_schema() {
    let corpus = corpus();
    let validator = validator();
    let normalizer = FrontmostNormalizer::new(salt(&corpus));
    for case in &corpus.identity_cases {
        let raw = raw(&case.raw);
        let observation = FrontmostObservation {
            schema_version: 1,
            transition: FrontmostTransition::Activated,
            observed_at: raw.observed_at,
            app: normalizer.normalize(&raw),
            dwell_ms: None,
            transient: false,
            basis: FrontmostBasis::Direct,
        };
        let json = serde_json::to_value(&observation).expect("JSON");
        assert!(validator.is_valid(&json), "{}", case.id);
    }
    let mut injected = serde_json::to_value(FrontmostObservation {
        schema_version: 1,
        transition: FrontmostTransition::Activated,
        observed_at: raw(&corpus.identity_cases[0].raw).observed_at,
        app: normalizer.normalize(&raw(&corpus.identity_cases[0].raw)),
        dwell_ms: None,
        transient: false,
        basis: FrontmostBasis::Direct,
    })
    .expect("JSON");
    injected["window_title"] = Value::from("private");
    assert!(!validator.is_valid(&injected));
    assert!(serde_json::from_value::<FrontmostObservation>(injected).is_err());
}

#[test]
fn bundle_name_and_version_are_kept_only_when_plain_and_bounded() {
    let normalizer = FrontmostNormalizer::new([7; 32]);
    let observe = |name: Option<&str>, version: Option<&str>, bundled: bool| {
        let raw = raw(&serde_json::json!({
            "transition": "activated",
            "observed_at": "2026-01-01T09:00:00Z",
            "bundle_identifier": "com.example.editor",
            "bundle_name": name,
            "bundle_version": version,
            "bundled": bundled,
            "activation_policy": "regular",
            "translocated": false,
            "signing": null,
            "process_id": 501,
            "process_started_micros": 1_767_254_400_000_000_i64,
        }));
        let app = normalizer.normalize(&raw);
        let json = serde_json::to_value(serde_json::json!({
            "schema_version": 1, "transition": "activated", "observed_at": "2026-01-01T09:00:00Z",
            "app": app, "dwell_ms": null, "transient": false, "basis": "direct",
        }))
        .expect("json");
        assert!(validator().is_valid(&json), "{json}");
        match app {
            FrontmostApp::Known { name, version, .. } => (name, version),
            FrontmostApp::Unknown { .. } => panic!("expected a known app"),
        }
    };
    let kept = |name: &str, version: &str| (Some(name.to_owned()), Some(version.to_owned()));
    assert_eq!(observe(Some("Editor"), Some("2.4.1"), true), kept("Editor", "2.4.1"));
    assert_eq!(
        observe(Some("  Editor Pro  "), Some("16.0 (1234)"), true),
        kept("Editor Pro", "16.0 (1234)")
    );
    assert_eq!(observe(Some("Éditeur 日本"), Some("1.0b3"), true), kept("Éditeur 日本", "1.0b3"));
    assert_eq!(observe(None, None, true), (None, None));
    // An unbundled executable has no bundle name even if one was supplied.
    assert_eq!(observe(Some("Editor"), Some("1.0"), false), (None, None));
    for bad_name in [
        "",
        "   ",
        "Users/alice/secret",
        "back\\slash",
        "line\nbreak",
        "tab\there",
        "rtl\u{202E}txt.exe",
        "zero\u{200B}width",
    ] {
        assert_eq!(observe(Some(bad_name), None, true).0, None, "{bad_name:?}");
    }
    assert_eq!(observe(Some(&"x".repeat(65)), None, true).0, None);
    assert_eq!(observe(Some(&"x".repeat(64)), None, true).0, Some("x".repeat(64)));
    for bad_version in ["", "beta", "1.0; rm -rf", "1.0\n2.0", "１.０", &"1".repeat(33)] {
        assert_eq!(observe(None, Some(bad_version), true).1, None, "{bad_version:?}");
    }
}

#[test]
fn records_without_name_or_version_serialize_as_before() {
    let normalizer = FrontmostNormalizer::new([7; 32]);
    let app = normalizer.normalize(&raw(&serde_json::json!({
        "transition": "activated",
        "observed_at": "2026-01-01T09:00:00Z",
        "bundle_identifier": "com.example.editor",
        "bundled": true,
        "activation_policy": "regular",
        "translocated": false,
        "signing": null,
        "process_id": 501,
        "process_started_micros": 1_767_254_400_000_000_i64,
    })));
    let json = serde_json::to_string(&app).expect("json");
    assert!(!json.contains("\"name\"") && !json.contains("\"version\""), "{json}");
    let back: FrontmostApp = serde_json::from_str(&json).expect("round trip");
    assert_eq!(back, app);
}
