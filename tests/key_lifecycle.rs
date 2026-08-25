use chacha20poly1305::{
    aead::{Aead, KeyInit},
    XChaCha20Poly1305, XNonce,
};
use ghostrace::{
    decrypt_payload, encrypt_payload, CiphertextEnvelope, DestructionConfirmation,
    DestructionReason, DestructionScope, DeterministicKeyProvider, KeyAlgorithm, KeyLifecycleError,
    KeyRing, KeyRotation, RotationPhase, KEY_LIFECYCLE_SCHEMA_JSON,
};
use serde_json::{json, Value};

fn key(byte: u8) -> [u8; 32] {
    [byte; 32]
}

fn lifecycle_validator() -> jsonschema::Validator {
    let schema: Value =
        serde_json::from_str(KEY_LIFECYCLE_SCHEMA_JSON).expect("key lifecycle schema JSON");
    jsonschema::options().build(&schema).expect("valid key lifecycle schema")
}

#[test]
fn envelope_records_algorithm_and_generation_without_key_material() {
    let provider = DeterministicKeyProvider::with_generation(key(0x11), 7).expect("provider");
    let encoded = encrypt_payload(&provider, b"event-aad", b"payload").expect("encrypt");
    assert!(encoded.starts_with(b"GRCE"));
    let envelope = CiphertextEnvelope::decode(&encoded).expect("decode envelope");
    assert_eq!(envelope.schema_version, 1);
    assert_eq!(envelope.algorithm, KeyAlgorithm::XChaCha20Poly1305);
    assert_eq!(envelope.key_generation, 7);
    assert_eq!(decrypt_payload(&provider, b"event-aad", &encoded).expect("decrypt"), b"payload");

    let value = serde_json::to_value(&envelope).expect("envelope JSON");
    assert!(value.get("key").is_none());
    assert!(value.get("key_material").is_none());
    assert!(value.get("secret").is_none());
    assert!(lifecycle_validator().is_valid(&value));
}

#[test]
fn legacy_nonce_ciphertext_remains_readable_during_migration() {
    let provider = DeterministicKeyProvider::new(key(0x21));
    let cipher = XChaCha20Poly1305::new_from_slice(&key(0x21)).expect("legacy key");
    let nonce = [9_u8; 24];
    let nonce = XNonce::try_from(&nonce[..]).expect("legacy nonce");
    let ciphertext = cipher
        .encrypt(&nonce, chacha20poly1305::aead::Payload { msg: b"legacy", aad: b"aad" })
        .expect("legacy encrypt");
    let mut encoded = nonce.to_vec();
    encoded.extend_from_slice(&ciphertext);
    assert_eq!(decrypt_payload(&provider, b"aad", &encoded).expect("legacy decrypt"), b"legacy");
}

#[test]
fn rotation_is_resumable_and_retires_only_after_verified_commit() {
    let aad = b"event-aad";
    let original = KeyRing::new(1, key(0x31)).expect("ring");
    let old_one = original.encrypt_current(aad, b"one").expect("old one");
    let old_two = original.encrypt_current(aad, b"two").expect("old two");

    let mut rotation = KeyRotation::begin(original, 2, key(0x32), 2).expect("begin rotation");
    assert_eq!(rotation.checkpoint().phase, RotationPhase::Prepared);
    assert_eq!(rotation.key_ring().decrypt(&old_one, aad).expect("old readable"), b"one");
    assert!(matches!(
        rotation.clone().commit(),
        Err(KeyLifecycleError::RotationIncomplete { verified: 0, total: 2 })
    ));

    let new_one = rotation.reencrypt(&old_one, aad).expect("reencrypt first");
    assert_eq!(rotation.checkpoint().verified_records, 1);
    assert_eq!(rotation.checkpoint().phase, RotationPhase::Reencrypting);
    assert_eq!(rotation.key_ring().decrypt(&old_two, aad).expect("old remains readable"), b"two");

    let checkpoint_value = serde_json::to_value(rotation.checkpoint()).expect("checkpoint JSON");
    assert!(lifecycle_validator().is_valid(&checkpoint_value));
    assert!(serde_json::from_value::<Value>(checkpoint_value.clone())
        .expect("checkpoint value")
        .get("key")
        .is_none());
    let checkpoint = serde_json::from_value(checkpoint_value).expect("checkpoint round trip");
    let mut resumed =
        KeyRotation::resume(rotation.key_ring().clone(), checkpoint).expect("resume rotation");
    let new_two = resumed.reencrypt(&old_two, aad).expect("reencrypt second");
    assert!(resumed.is_ready_to_commit());

    let committed = resumed.commit().expect("commit rotation");
    assert_eq!(committed.current_generation(), 2);
    assert!(!committed.contains_generation(1));
    assert_eq!(committed.decrypt(&new_one, aad).expect("new one"), b"one");
    assert_eq!(committed.decrypt(&new_two, aad).expect("new two"), b"two");
    assert!(committed.decrypt(&old_one, aad).is_err());
}

#[test]
fn destruction_requires_confirmation_and_reports_unrecoverability() {
    let mut ring = KeyRing::new(1, key(0x41)).expect("ring");
    ring.stage_generation(2, key(0x42)).expect("stage");
    let old =
        CiphertextEnvelope::encrypt_with_key(2, key(0x42), b"aad", b"old").expect("old envelope");

    assert!(matches!(
        ring.destroy_generation(
            2,
            DestructionConfirmation::unconfirmed(
                DestructionScope::Generation(2),
                DestructionReason::LostKey,
            ),
        ),
        Err(KeyLifecycleError::ConfirmationRequired)
    ));
    assert!(matches!(
        ring.destroy_generation(
            1,
            DestructionConfirmation::for_generation(1, DestructionReason::UserReset),
        ),
        Err(KeyLifecycleError::CurrentGeneration)
    ));
    assert!(matches!(
        ring.destroy_generation(
            2,
            DestructionConfirmation::for_all(DestructionReason::Compromise),
        ),
        Err(KeyLifecycleError::ConfirmationScope)
    ));

    let receipt = ring
        .destroy_generation(
            2,
            DestructionConfirmation::for_generation(2, DestructionReason::LostKey),
        )
        .expect("destroy old generation");
    assert!(receipt.data_unrecoverable);
    assert_eq!(receipt.destroyed_generations, vec![2]);
    assert!(receipt.explanation.contains("unrecoverable"));
    assert!(ring.decrypt(&old, b"aad").is_err());
    assert!(lifecycle_validator().is_valid(&serde_json::to_value(&receipt).expect("receipt JSON")));

    ring.stage_generation(3, key(0x43)).expect("stage third");
    let reset = ring
        .reset_all(DestructionConfirmation::for_all(DestructionReason::UserReset))
        .expect("reset all");
    assert!(reset.data_unrecoverable);
    assert_eq!(reset.destroyed_generations, vec![1, 3]);
    assert_eq!(ring.current_generation(), 0);
    assert!(ring.current_metadata().is_err());
    assert!(lifecycle_validator().is_valid(&serde_json::to_value(&reset).expect("reset JSON")));
}

#[test]
fn lifecycle_schema_rejects_unknown_fields_and_accepts_public_receipts() {
    let validator = lifecycle_validator();
    let confirmation = DestructionConfirmation::for_generation(4, DestructionReason::Compromise);
    assert!(validator.is_valid(&serde_json::to_value(confirmation).expect("confirmation JSON")));

    let invalid = json!({
        "schema_version": 1,
        "algorithm": "x_cha_cha20_poly1305",
        "key_generation": 4,
        "unexpected": "secret"
    });
    assert!(!validator.is_valid(&invalid));
    assert!(serde_json::from_value::<ghostrace::RotationCheckpoint>(json!({
        "schema_version": 1,
        "from_generation": 1,
        "to_generation": 2,
        "total_records": 1,
        "verified_records": 0,
        "phase": "prepared",
        "unexpected": true
    }))
    .is_err());
}
