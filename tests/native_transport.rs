//! Pure Chromium/Safari transport normalization fixtures.
//!
//! These tests do not install an extension, invoke a browser, open a network
//! listener, or claim Safari runtime support. They prove that two bounded outer
//! envelopes deliver the same protocol body and identity shape.

use ghostrace::{
    encode_chromium_stream, encode_frame, encode_safari_envelope, normalize_chromium_stream,
    normalize_safari_envelope, validate_chromium_caller_origin, NativeMessagingError,
    NativeTransportError, TransportIdentity, MAX_CHROMIUM_STREAM_BYTES, MAX_CHROMIUM_STREAM_FRAMES,
    MAX_NATIVE_FRAME_BYTES,
};
use serde::Deserialize;

const CHROMIUM_ORIGIN: &str = "chrome-extension://abcdefghijklmnopabcdefghijklmnop/";
const SAFARI_BUNDLE: &str = "com.alisinadevelo.ghostrace.extension";
const SAFARI_PROFILE: &str = "default";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TransportCorpus {
    schema_version: u32,
    contract_id: String,
    scope: String,
    network_listener: bool,
    cases: Vec<TransportCase>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TransportCase {
    id: String,
    body: String,
    chromium_origin: String,
    safari_bundle_id: String,
    safari_profile_id: String,
    expected: String,
}

fn heartbeat(seq: u64) -> Vec<u8> {
    format!(r#"{{"type":"heartbeat","seq":{seq},"mac":"00"}}"#).into_bytes()
}

#[test]
fn chromium_and_safari_profiles_normalize_the_same_body() {
    let body = heartbeat(2);
    let chromium = normalize_chromium_stream(
        &encode_chromium_stream(&[body.as_slice()]).expect("Chromium frame"),
        CHROMIUM_ORIGIN,
    )
    .expect("Chromium normalization");
    let safari = normalize_safari_envelope(
        &encode_safari_envelope(SAFARI_BUNDLE, SAFARI_PROFILE, &body).expect("Safari envelope"),
    )
    .expect("Safari normalization");

    assert_eq!(chromium[0].body, body);
    assert_eq!(safari.body, body);
    assert_eq!(
        chromium[0].identity,
        TransportIdentity::Chromium { caller_origin: CHROMIUM_ORIGIN.to_owned() }
    );
    assert_eq!(
        safari.identity,
        TransportIdentity::Safari {
            extension_bundle_id: SAFARI_BUNDLE.to_owned(),
            profile_id: SAFARI_PROFILE.to_owned(),
        }
    );
}

#[test]
fn chromium_fixture_drains_many_frames_incrementally() {
    let bodies = (0..8).map(heartbeat).collect::<Vec<_>>();
    let references = bodies.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let normalized = normalize_chromium_stream(
        &encode_chromium_stream(&references).expect("Chromium frames"),
        CHROMIUM_ORIGIN,
    )
    .expect("Chromium normalization");
    assert_eq!(normalized.len(), bodies.len());
    assert_eq!(
        normalized.iter().map(|frame| &frame.body).collect::<Vec<_>>(),
        bodies.iter().collect::<Vec<_>>()
    );
}

#[test]
fn chromium_stream_rejects_bad_origin_and_truncation() {
    assert_eq!(
        validate_chromium_caller_origin("chrome-extension://not-an-id/"),
        Err(NativeTransportError::InvalidChromiumOrigin)
    );
    let body = heartbeat(2);
    let framed = encode_chromium_stream(&[body.as_slice()]).expect("frame");
    assert_eq!(
        normalize_chromium_stream(&framed[..framed.len() - 1], CHROMIUM_ORIGIN),
        Err(NativeTransportError::NativeMessaging(NativeMessagingError::Truncated))
    );
}

#[test]
fn chromium_stream_limits_cover_encode_and_normalize_collection() {
    let too_many =
        (0..=MAX_CHROMIUM_STREAM_FRAMES).map(|seq| heartbeat(seq as u64)).collect::<Vec<_>>();
    let too_many_references = too_many.iter().map(Vec::as_slice).collect::<Vec<_>>();
    assert_eq!(
        encode_chromium_stream(&too_many_references),
        Err(NativeTransportError::ChromiumStreamTooManyFrames)
    );

    let maximal_body = vec![b'x'; MAX_NATIVE_FRAME_BYTES];
    let too_many_bytes =
        vec![maximal_body.as_slice(); MAX_CHROMIUM_STREAM_BYTES / (MAX_NATIVE_FRAME_BYTES + 4) + 1];
    assert_eq!(
        encode_chromium_stream(&too_many_bytes),
        Err(NativeTransportError::ChromiumStreamTooLarge)
    );

    let oversized_input = vec![0u8; MAX_CHROMIUM_STREAM_BYTES + 1];
    assert_eq!(
        normalize_chromium_stream(&oversized_input, CHROMIUM_ORIGIN),
        Err(NativeTransportError::ChromiumStreamTooLarge)
    );

    let mut many_frames = Vec::new();
    for seq in 0..=MAX_CHROMIUM_STREAM_FRAMES {
        many_frames.extend(encode_frame(&heartbeat(seq as u64)).expect("fixture frame"));
    }
    assert!(many_frames.len() <= MAX_CHROMIUM_STREAM_BYTES);
    assert_eq!(
        normalize_chromium_stream(&many_frames, CHROMIUM_ORIGIN),
        Err(NativeTransportError::ChromiumStreamTooManyFrames)
    );
}

#[test]
fn safari_envelope_rejects_unknown_fields_versions_and_identities() {
    let body = heartbeat(2);
    let encoded = encode_safari_envelope(SAFARI_BUNDLE, SAFARI_PROFILE, &body).expect("envelope");
    let mut unknown = String::from_utf8(encoded).expect("UTF-8 envelope");
    unknown.insert(unknown.len() - 1, ',');
    unknown.insert_str(unknown.len() - 1, r#""extra":true"#);
    assert_eq!(
        normalize_safari_envelope(unknown.as_bytes()),
        Err(NativeTransportError::MalformedSafariEnvelope)
    );

    let encoded_version =
        encode_safari_envelope(SAFARI_BUNDLE, SAFARI_PROFILE, &body).expect("version envelope");
    let version = String::from_utf8(encoded_version).expect("UTF-8 envelope").replacen(
        "\"schema_version\":1",
        "\"schema_version\":2",
        1,
    );
    assert_eq!(
        normalize_safari_envelope(version.as_bytes()),
        Err(NativeTransportError::UnsupportedSafariVersion)
    );

    assert_eq!(
        encode_safari_envelope("com.example.bad/id", SAFARI_PROFILE, &body),
        Err(NativeTransportError::InvalidSafariIdentity)
    );
}

#[test]
fn both_profiles_preserve_the_native_frame_bound() {
    let body = vec![b'x'; MAX_NATIVE_FRAME_BYTES + 1];
    assert_eq!(
        encode_safari_envelope(SAFARI_BUNDLE, SAFARI_PROFILE, &body),
        Err(NativeTransportError::NativeMessaging(NativeMessagingError::FrameTooLarge))
    );
    assert!(encode_chromium_stream(&[body.as_slice()]).is_err());
}

#[test]
fn checked_in_differential_corpus_stays_pure_and_normalizes_every_case() {
    let corpus =
        serde_json::from_str::<TransportCorpus>(include_str!("fixtures/native-transport-v1.json"))
            .expect("transport corpus");
    assert_eq!(corpus.schema_version, 1);
    assert_eq!(corpus.contract_id, "ghostrace-native-transport-v1");
    assert_eq!(corpus.scope, "pure_fixture_only");
    assert!(!corpus.network_listener);
    assert!(!corpus.cases.is_empty());

    for case in corpus.cases {
        assert_eq!(case.expected, "normalized", "{}", case.id);
        let body = case.body.into_bytes();
        let chromium_stream =
            encode_chromium_stream(&[body.as_slice()]).expect("Chromium fixture frame");
        let chromium = normalize_chromium_stream(&chromium_stream, &case.chromium_origin)
            .expect("Chromium fixture normalization");
        let safari_envelope =
            encode_safari_envelope(&case.safari_bundle_id, &case.safari_profile_id, &body)
                .expect("Safari fixture envelope");
        let safari =
            normalize_safari_envelope(&safari_envelope).expect("Safari fixture normalization");
        assert_eq!(chromium.len(), 1, "{}", case.id);
        assert_eq!(chromium[0].body, safari.body, "{}", case.id);
    }
}
