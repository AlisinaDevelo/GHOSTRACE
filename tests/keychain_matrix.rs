use jsonschema::Validator;
use serde_json::Value;

const MATRIX: &str = include_str!("../docs/evidence/0056-key-availability-matrix.json");
const SCHEMA: &str = include_str!("../schemas/key-availability-matrix-v1.json");

fn validator() -> Validator {
    let schema: Value = serde_json::from_str(SCHEMA).expect("matrix schema JSON");
    jsonschema::options().build(&schema).expect("matrix schema is valid")
}

fn matrix() -> Value {
    serde_json::from_str(MATRIX).expect("matrix JSON")
}

#[test]
fn matrix_covers_every_lifecycle_transition_and_privacy_invariants() {
    let value = matrix();
    let schema_validator = validator();
    let errors = schema_validator.iter_errors(&value).collect::<Vec<_>>();
    assert!(errors.is_empty(), "invalid key availability matrix: {errors:?}");
    let names = value["transitions"]
        .as_array()
        .expect("transitions")
        .iter()
        .map(|row| row["name"].as_str().expect("transition name"))
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        names,
        [
            "fast-user-switch",
            "launchd-restart",
            "lock",
            "login/unlocked",
            "logout",
            "sleep",
            "wake"
        ]
        .into_iter()
        .collect()
    );
    assert_eq!(value["privacy"]["fallback_key"], false);
    assert_eq!(value["privacy"]["plaintext_queue"], false);
    assert_eq!(value["privacy"]["silent_loss"], false);
}
