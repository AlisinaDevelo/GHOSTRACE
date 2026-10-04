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
struct CountingKeyProvider<K = DeterministicKeyProvider> {
    inner: K,
    accesses: Arc<AtomicUsize>,
}

impl CountingKeyProvider {
    fn new(accesses: Arc<AtomicUsize>) -> Self {
        Self { inner: DeterministicKeyProvider::from_seed("0008-aead-order"), accesses }
    }
}

impl<K: KeyProvider> KeyProvider for CountingKeyProvider<K> {
    fn key(&self) -> Result<[u8; 32], CryptoError> {
        self.accesses.fetch_add(1, Ordering::SeqCst);
        self.inner.key()
    }

    fn key_generation(&self) -> u32 {
        self.inner.key_generation()
    }

    fn key_for_generation(&self, generation: u32) -> Result<[u8; 32], CryptoError> {
        self.accesses.fetch_add(1, Ordering::SeqCst);
        self.inner.key_for_generation(generation)
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
fn in_memory_writes_share_one_key_read_and_do_not_cache_between_transactions() {
    let events = read_fixture(fixture_path()).expect("fixture");
    let accesses = Arc::new(AtomicUsize::new(0));
    let journal =
        Journal::in_memory(CountingKeyProvider::new(Arc::clone(&accesses))).expect("journal");
    let policy = PolicyProfile::fixture_default();
    for event in &events[..2] {
        accesses.store(0, Ordering::SeqCst);
        journal.ingest(&IngestionOrigin::fixture(), event, &policy).expect("insert");
        assert_eq!(
            accesses.load(Ordering::SeqCst),
            1,
            "one read for authentication, encryption and refresh"
        );
    }
    accesses.store(0, Ordering::SeqCst);
    journal.ingest_batch(&IngestionOrigin::fixture(), &events[2..], &policy).expect("batch");
    assert_eq!(accesses.load(Ordering::SeqCst), 1, "batch encryption shares the transaction key");
    assert!(journal.verify_authenticated_state().expect("full verification").valid);
    println!("AUTH_WRITE_KEY_READS storage=memory insert=1 next_transaction=1 batch=1 PASS");
}

#[test]
fn file_backed_writes_share_one_key_read_after_full_preflight() {
    let events = read_fixture(fixture_path()).expect("fixture");
    let accesses = Arc::new(AtomicUsize::new(0));
    let directory = tempfile::tempdir().expect("directory");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700))
            .expect("private directory");
    }
    let path = directory.path().join("single-key-read.sqlite3");
    let provider = CountingKeyProvider::new(Arc::clone(&accesses));
    let journal = Journal::open_fixture(&path, provider.clone()).expect("journal");
    let policy = PolicyProfile::fixture_default();
    journal.ingest(&IngestionOrigin::fixture(), &events[0], &policy).expect("first write");
    assert_eq!(accesses.load(Ordering::SeqCst), 1, "bootstrap and encryption share one read");
    accesses.store(0, Ordering::SeqCst);
    journal.ingest(&IngestionOrigin::fixture(), &events[1], &policy).expect("steady-state write");
    assert_eq!(accesses.load(Ordering::SeqCst), 1, "exactly one read on the file-backed hot path");
    drop(journal);
    let journal = Journal::open_fixture(&path, provider).expect("reopen");
    accesses.store(0, Ordering::SeqCst);
    journal.ingest(&IngestionOrigin::fixture(), &events[2], &policy).expect("reopened writer");
    assert_eq!(accesses.load(Ordering::SeqCst), 1, "full preflight shares the write key");
    accesses.store(0, Ordering::SeqCst);
    journal.ingest_batch(&IngestionOrigin::fixture(), &events[3..], &policy).expect("batch");
    assert_eq!(accesses.load(Ordering::SeqCst), 1, "file-backed batch shares one read");
    assert!(journal.verify_authenticated_state().expect("full verification").valid);
    println!("AUTH_WRITE_KEY_READS storage=file bootstrap=1 steady_insert=1 reopened_preflight=1 batch=1 PASS");
}

#[test]
fn rotation_resolves_old_and_new_generations_once_each() {
    let events = read_fixture(fixture_path()).expect("fixture");
    let accesses = Arc::new(AtomicUsize::new(0));
    let directory = tempfile::tempdir().expect("directory");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700))
            .expect("private directory");
    }
    let path = directory.path().join("key-generation-scope.sqlite3");
    let mut ring = ghostrace::KeyRing::new(1, [0x31; 32]).expect("key ring");
    let journal = Journal::open_fixture(
        &path,
        CountingKeyProvider { inner: ring.clone(), accesses: Arc::clone(&accesses) },
    )
    .expect("journal");
    journal
        .ingest(&IngestionOrigin::fixture(), &events[0], &PolicyProfile::fixture_default())
        .expect("first write");
    drop(journal);
    ring.stage_generation(2, [0x32; 32]).expect("stage generation");
    ring.activate_generation(2).expect("activate generation");
    let journal = Journal::open_fixture(
        &path,
        CountingKeyProvider { inner: ring, accesses: Arc::clone(&accesses) },
    )
    .expect("rotated journal");
    accesses.store(0, Ordering::SeqCst);
    journal
        .ingest(&IngestionOrigin::fixture(), &events[1], &PolicyProfile::fixture_default())
        .expect("rotation write");
    assert_eq!(
        accesses.load(Ordering::SeqCst),
        2,
        "resolve old authentication and new encryption keys once each"
    );
    let envelope = CiphertextEnvelope::decode(
        &journal.raw_payload_ciphertext(events[1].event_id).expect("ciphertext"),
    )
    .expect("envelope");
    assert_eq!(envelope.key_generation, 2);
    assert!(journal.verify_authenticated_state().expect("rotated anchor").valid);
    assert_eq!(
        journal.event(events[0].event_id).expect("old ciphertext").payload,
        events[0].payload
    );
    assert_eq!(
        journal.event(events[1].event_id).expect("new ciphertext").payload,
        events[1].payload
    );
    println!("AUTH_WRITE_KEY_READS rotation_generations=2 backing_reads=2 old_ciphertext=PASS new_ciphertext=PASS");
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
