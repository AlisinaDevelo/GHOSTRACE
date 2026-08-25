use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use ghostrace::{
    read_fixture, CiphertextEnvelope, CryptoError, DeterministicKeyProvider, FaultPlan, FaultPoint,
    GhostraceError, IngestionOrigin, Journal, KeyProvider, PolicyProfile,
    CIPHERTEXT_ENVELOPE_VERSION,
};

fn fixture_path() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/causal-chain.jsonl")
}

#[derive(Clone)]
struct MissingKeyProvider;

impl KeyProvider for MissingKeyProvider {
    fn key(&self) -> Result<[u8; 32], CryptoError> {
        Err(CryptoError::KeyProvider("key unavailable".to_owned()))
    }
}

#[derive(Clone)]
struct CountingKeyProvider {
    inner: DeterministicKeyProvider,
    accesses: Arc<AtomicUsize>,
}

impl CountingKeyProvider {
    fn new(accesses: Arc<AtomicUsize>) -> Self {
        Self { inner: DeterministicKeyProvider::from_seed("0008-aead-order"), accesses }
    }
}

impl KeyProvider for CountingKeyProvider {
    fn key(&self) -> Result<[u8; 32], CryptoError> {
        self.accesses.fetch_add(1, Ordering::SeqCst);
        self.inner.key()
    }

    fn key_generation(&self) -> u32 {
        self.inner.key_generation()
    }
}

#[test]
fn missing_key_fails_closed_before_sqlite_insertion() {
    let event = read_fixture(fixture_path()).expect("fixture").remove(0);
    let journal = Journal::in_memory(MissingKeyProvider).expect("journal");
    let error = journal
        .ingest(&IngestionOrigin::fixture(), &event, &PolicyProfile::fixture_default())
        .expect_err("missing key must reject ingestion");

    assert!(matches!(error, GhostraceError::Crypto(CryptoError::KeyProvider(_))));
    assert_eq!(journal.events().expect("events after rollback").len(), 0);
    assert_eq!(journal.diagnostic_count().expect("diagnostics after rollback"), 0);
    let rendered = format!("{error}\n{error:?}");
    assert!(!rendered.contains("fixture_secret"));
    assert!(!rendered.contains("0008-aead-order"));
}

#[test]
fn encryption_runs_before_the_event_insert_boundary_and_payload_is_ciphertext() {
    let event = read_fixture(fixture_path()).expect("fixture").remove(0);
    let accesses = Arc::new(AtomicUsize::new(0));
    let provider = CountingKeyProvider::new(Arc::clone(&accesses));
    let plan = FaultPlan::fail_once(FaultPoint::EventBeforeInsert);
    let journal = Journal::in_memory_with_fault_plan(provider, plan.clone()).expect("journal");
    let error = journal
        .ingest(&IngestionOrigin::fixture(), &event, &PolicyProfile::fixture_default())
        .expect_err("fault must stop before insertion");

    assert!(
        matches!(error, GhostraceError::InjectedFault { point } if point == "event_before_insert")
    );
    assert_eq!(accesses.load(Ordering::SeqCst), 1, "key access precedes insert boundary");
    assert_eq!(plan.fired().len(), 1);
    assert!(journal.events().expect("events after rollback").is_empty());

    let journal = Journal::in_memory(DeterministicKeyProvider::from_seed("0008-aead-storage"))
        .expect("journal");
    journal
        .ingest(&IngestionOrigin::fixture(), &event, &PolicyProfile::fixture_default())
        .expect("ingest");
    let ciphertext = journal.raw_payload_ciphertext(event.event_id).expect("ciphertext");
    assert!(ciphertext.starts_with(b"GRCE"));
    assert!(!ciphertext.windows(b"fixture_secret".len()).any(|window| window == b"fixture_secret"));
    let envelope = CiphertextEnvelope::decode(&ciphertext).expect("envelope");
    assert_eq!(envelope.schema_version, CIPHERTEXT_ENVELOPE_VERSION);
    let restored = journal.event(event.event_id).expect("round trip");
    assert_eq!(restored.event_id, event.event_id);
    assert_eq!(restored.payload, event.payload);
}

#[test]
fn public_envelope_metadata_contains_no_key_material() {
    let provider = DeterministicKeyProvider::from_seed("0008-public-metadata");
    let encoded = ghostrace::encrypt_payload(&provider, b"event-aad", b"payload").expect("encrypt");
    let envelope = CiphertextEnvelope::decode(&encoded).expect("envelope");
    let value = serde_json::to_value(&envelope).expect("envelope JSON");
    for field in ["key", "key_material", "secret"] {
        assert!(value.get(field).is_none(), "unexpected {field} field");
    }
    assert_eq!(envelope.metadata().key_generation, provider.generation());

    #[cfg(target_os = "macos")]
    {
        let provider = ghostrace::MacOsKeychainProvider::new();
        let debug = format!("{provider:?}");
        assert!(debug.contains("<redacted>"));
        assert!(!debug.contains(ghostrace::JOURNAL_KEYCHAIN_SERVICE));
        assert!(!debug.contains(ghostrace::JOURNAL_KEYCHAIN_ACCOUNT));
    }
}
