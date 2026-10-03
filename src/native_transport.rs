//! Pure transport normalization for the browser native-messaging boundary.
//!
//! The Chromium and Safari implementations have different outer envelopes,
//! but they must deliver the same bounded UTF-8 JSON body to the protocol
//! session. This module intentionally contains no browser API, socket, file,
//! keychain, or journal access. It is suitable for deterministic fixtures and
//! fuzz targets; the native host owns real stdio, deadlines, pairing, and
//! persistence.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::native_messaging::{FrameDecoder, NativeMessagingError, MAX_NATIVE_FRAME_BYTES};

/// Version of the synthetic Safari envelope used by the pure fixture adapter.
pub const SAFARI_TRANSPORT_SCHEMA_VERSION: u32 = 1;
/// Maximum UTF-8 bytes in a synthetic Safari bundle identifier or profile ID.
pub const MAX_SAFARI_ID_BYTES: usize = 128;
/// Maximum number of Chromium frames collected by one pure fixture stream.
pub const MAX_CHROMIUM_STREAM_FRAMES: usize = 256;
/// Maximum aggregate Chromium framing and body bytes collected by one pure
/// fixture stream. The production host processes live stdio incrementally; the
/// pure adapter still needs a finite batch bound before collecting frames.
pub const MAX_CHROMIUM_STREAM_BYTES: usize = 8 * crate::native_messaging::MAX_NATIVE_DECODER_BUFFER;

/// A transport identity supplied outside the protocol body.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "transport", rename_all = "snake_case", deny_unknown_fields)]
pub enum TransportIdentity {
    Chromium { caller_origin: String },
    Safari { extension_bundle_id: String, profile_id: String },
}

/// One normalized body emitted by either transport profile.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NormalizedTransportFrame {
    pub identity: TransportIdentity,
    pub body: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Error)]
pub enum NativeTransportError {
    #[error("the Chromium caller origin is not an exact extension origin")]
    InvalidChromiumOrigin,
    #[error("the Safari transport envelope is malformed")]
    MalformedSafariEnvelope,
    #[error("the Safari transport envelope version is unsupported")]
    UnsupportedSafariVersion,
    #[error("the Safari transport identity is not bounded")]
    InvalidSafariIdentity,
    #[error("the Safari transport payload is not valid UTF-8")]
    InvalidSafariPayload,
    #[error("the Safari transport envelope exceeds its byte bound")]
    SafariEnvelopeTooLarge,
    #[error("the Chromium transport stream exceeds its aggregate byte bound")]
    ChromiumStreamTooLarge,
    #[error("the Chromium transport stream exceeds its frame-count bound")]
    ChromiumStreamTooManyFrames,
    #[error(transparent)]
    NativeMessaging(#[from] NativeMessagingError),
}

/// The bounded fixture envelope standing in for the data delivered to a
/// Safari web-extension native handler. It is not a shipped Safari adapter.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SafariTransportEnvelope {
    pub schema_version: u32,
    pub extension_bundle_id: String,
    pub profile_id: String,
    pub payload: String,
}

/// Validate the caller-origin argument Chromium supplies to a native host.
pub fn validate_chromium_caller_origin(origin: &str) -> Result<(), NativeTransportError> {
    const PREFIX: &str = "chrome-extension://";
    let Some(id) = origin.strip_prefix(PREFIX).and_then(|value| value.strip_suffix('/')) else {
        return Err(NativeTransportError::InvalidChromiumOrigin);
    };
    if id.len() != 32 || !id.bytes().all(|byte| (b'a'..=b'p').contains(&byte)) {
        return Err(NativeTransportError::InvalidChromiumOrigin);
    }
    Ok(())
}

/// Decode a complete synthetic Chromium stdio byte stream into normalized
/// protocol bodies. The production host uses the same `FrameDecoder` while
/// reading fixed-size chunks and enforcing an I/O deadline.
pub fn normalize_chromium_stream(
    stream: &[u8],
    caller_origin: &str,
) -> Result<Vec<NormalizedTransportFrame>, NativeTransportError> {
    validate_chromium_caller_origin(caller_origin)?;
    if stream.len() > MAX_CHROMIUM_STREAM_BYTES {
        return Err(NativeTransportError::ChromiumStreamTooLarge);
    }
    let mut decoder = FrameDecoder::new();
    let mut frames = Vec::with_capacity(stream.len().min(MAX_CHROMIUM_STREAM_FRAMES));
    let mut frame_count = 0usize;
    // Exercise the same bounded incremental path as the host. A fixture may
    // contain several frames, so feeding the whole stream at once would make
    // an otherwise valid batch fail merely because it is larger than the
    // decoder's in-memory window.
    for chunk in stream.chunks(4096) {
        decoder.push(chunk)?;
        while let Some(body) = decoder.next_frame()? {
            frame_count = frame_count
                .checked_add(1)
                .ok_or(NativeTransportError::ChromiumStreamTooManyFrames)?;
            if frame_count > MAX_CHROMIUM_STREAM_FRAMES {
                return Err(NativeTransportError::ChromiumStreamTooManyFrames);
            }
            frames.push(NormalizedTransportFrame {
                identity: TransportIdentity::Chromium { caller_origin: caller_origin.to_owned() },
                body,
            });
        }
    }
    decoder.finish()?;
    Ok(frames)
}

/// Serialize one bounded synthetic Safari envelope for fixture generation.
pub fn encode_safari_envelope(
    extension_bundle_id: &str,
    profile_id: &str,
    body: &[u8],
) -> Result<Vec<u8>, NativeTransportError> {
    validate_safari_identity(extension_bundle_id, profile_id)?;
    if body.is_empty() {
        return Err(NativeTransportError::NativeMessaging(NativeMessagingError::EmptyFrame));
    }
    if body.len() > MAX_NATIVE_FRAME_BYTES {
        return Err(NativeTransportError::NativeMessaging(NativeMessagingError::FrameTooLarge));
    }
    let payload =
        std::str::from_utf8(body).map_err(|_| NativeTransportError::InvalidSafariPayload)?;
    let envelope = SafariTransportEnvelope {
        schema_version: SAFARI_TRANSPORT_SCHEMA_VERSION,
        extension_bundle_id: extension_bundle_id.to_owned(),
        profile_id: profile_id.to_owned(),
        payload: payload.to_owned(),
    };
    let encoded =
        serde_json::to_vec(&envelope).map_err(|_| NativeTransportError::MalformedSafariEnvelope)?;
    if encoded.len() > MAX_NATIVE_FRAME_BYTES {
        return Err(NativeTransportError::SafariEnvelopeTooLarge);
    }
    Ok(encoded)
}

/// Normalize one bounded synthetic Safari native-handler envelope.
pub fn normalize_safari_envelope(
    encoded: &[u8],
) -> Result<NormalizedTransportFrame, NativeTransportError> {
    if encoded.is_empty() || encoded.len() > MAX_NATIVE_FRAME_BYTES {
        return Err(NativeTransportError::SafariEnvelopeTooLarge);
    }
    let envelope = serde_json::from_slice::<SafariTransportEnvelope>(encoded)
        .map_err(|_| NativeTransportError::MalformedSafariEnvelope)?;
    if envelope.schema_version != SAFARI_TRANSPORT_SCHEMA_VERSION {
        return Err(NativeTransportError::UnsupportedSafariVersion);
    }
    validate_safari_identity(&envelope.extension_bundle_id, &envelope.profile_id)?;
    let body = envelope.payload.into_bytes();
    if body.is_empty() {
        return Err(NativeTransportError::NativeMessaging(NativeMessagingError::EmptyFrame));
    }
    if body.len() > MAX_NATIVE_FRAME_BYTES {
        return Err(NativeTransportError::NativeMessaging(NativeMessagingError::FrameTooLarge));
    }
    Ok(NormalizedTransportFrame {
        identity: TransportIdentity::Safari {
            extension_bundle_id: envelope.extension_bundle_id,
            profile_id: envelope.profile_id,
        },
        body,
    })
}

/// Validate the bounded identity carried by the synthetic Safari envelope.
pub fn validate_safari_identity(
    extension_bundle_id: &str,
    profile_id: &str,
) -> Result<(), NativeTransportError> {
    if !valid_safari_bundle_id(extension_bundle_id)
        || profile_id.is_empty()
        || profile_id.len() > MAX_SAFARI_ID_BYTES
        || !profile_id.is_ascii()
        || !profile_id.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        })
    {
        return Err(NativeTransportError::InvalidSafariIdentity);
    }
    Ok(())
}

fn valid_safari_bundle_id(value: &str) -> bool {
    if value.is_empty() || value.len() > MAX_SAFARI_ID_BYTES || !value.is_ascii() {
        return false;
    }
    value.split('.').all(|label| {
        !label.is_empty()
            && label.len() <= 63
            && label.as_bytes().first().is_some_and(u8::is_ascii_alphanumeric)
            && label.as_bytes().last().is_some_and(u8::is_ascii_alphanumeric)
            && label
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    })
}

/// Build one Chromium fixture stream from bodies without exposing framing
/// details in every differential test.
pub fn encode_chromium_stream(bodies: &[&[u8]]) -> Result<Vec<u8>, NativeTransportError> {
    if bodies.len() > MAX_CHROMIUM_STREAM_FRAMES {
        return Err(NativeTransportError::ChromiumStreamTooManyFrames);
    }

    // Validate each body and calculate the complete output length before
    // allocating the aggregate stream. This prevents a large body slice list
    // or a long batch from driving geometric Vec growth.
    let mut total = 0usize;
    for body in bodies {
        if body.is_empty() {
            return Err(NativeTransportError::NativeMessaging(NativeMessagingError::EmptyFrame));
        }
        if body.len() > MAX_NATIVE_FRAME_BYTES {
            return Err(NativeTransportError::NativeMessaging(NativeMessagingError::FrameTooLarge));
        }
        let framed_len = body
            .len()
            .checked_add(4)
            .and_then(|length| total.checked_add(length))
            .ok_or(NativeTransportError::ChromiumStreamTooLarge)?;
        if framed_len > MAX_CHROMIUM_STREAM_BYTES {
            return Err(NativeTransportError::ChromiumStreamTooLarge);
        }
        total = framed_len;
    }

    let mut stream = Vec::with_capacity(total);
    for body in bodies {
        stream.extend_from_slice(&(body.len() as u32).to_ne_bytes());
        stream.extend_from_slice(body);
    }
    Ok(stream)
}
