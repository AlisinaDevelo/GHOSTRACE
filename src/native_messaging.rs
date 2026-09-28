//! Strict native-messaging framing and protocol for a browser extension
//! talking to the GHOSTRACE native host.
//!
//! Chromium native messaging frames each message as a 4-byte native-endian
//! length followed by that many bytes of UTF-8 JSON. Every byte arriving here
//! is attacker-shaped, so the decoder rejects an oversized length before
//! allocating, and every message passes a bounded structural scan (nesting,
//! element count) before strict typed deserialization. The session enforces a
//! hello-first handshake, exact protocol version, strictly increasing
//! sequence numbers, an idle deadline, a message rate, and a clean shutdown.
//! Errors are fixed values and never echo message content.

use std::{collections::VecDeque, time::Duration};

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// The only protocol version this host speaks.
pub const NATIVE_MESSAGING_PROTOCOL_VERSION: u32 = 1;
/// Largest inbound frame body accepted from an extension.
pub const MAX_NATIVE_FRAME_BYTES: usize = 64 * 1024;
/// Most bytes the decoder buffers: two maximal frames with their prefixes.
pub const MAX_NATIVE_DECODER_BUFFER: usize = 2 * (4 + MAX_NATIVE_FRAME_BYTES);
/// Deepest JSON nesting accepted in a message.
pub const MAX_NATIVE_MESSAGE_DEPTH: usize = 8;
/// Most JSON values (objects, arrays, scalars) accepted in one message.
pub const MAX_NATIVE_MESSAGE_VALUES: usize = 256;
/// Longest silence allowed between messages in an open session.
pub const NATIVE_SESSION_IDLE_TIMEOUT: Duration = Duration::from_secs(120);
/// Most messages accepted within one rate window.
pub const MAX_NATIVE_MESSAGES_PER_WINDOW: usize = 200;
/// Rate window length.
pub const NATIVE_RATE_WINDOW: Duration = Duration::from_secs(10);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Error)]
pub enum NativeMessagingError {
    #[error("frame length exceeds the protocol bound")]
    FrameTooLarge,
    #[error("frame is empty")]
    EmptyFrame,
    #[error("buffered input exceeds the decoder bound; drain frames before pushing more")]
    BufferFull,
    #[error("stream ended inside a frame")]
    Truncated,
    #[error("data arrived after the session ended")]
    TrailingData,
    #[error("frame is not valid UTF-8")]
    InvalidUtf8,
    #[error("message JSON is malformed")]
    MalformedJson,
    #[error("message nesting exceeds the protocol bound")]
    TooDeep,
    #[error("message has too many values")]
    TooManyValues,
    #[error("message type or fields are not defined by the protocol")]
    UnknownMessage,
    #[error("the first message must be hello")]
    HelloRequired,
    #[error("hello was sent twice")]
    DuplicateHello,
    #[error("the protocol version is not supported")]
    UnsupportedVersion,
    #[error("the sequence number repeats or moves backwards")]
    Replay,
    #[error("the session was idle past its deadline")]
    Timeout,
    #[error("messages arrived faster than the session allows")]
    RateLimited,
}

/// Incremental length-prefixed frame decoder.
#[derive(Debug, Default)]
pub struct FrameDecoder {
    buffer: Vec<u8>,
}

impl FrameDecoder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Append received bytes. The buffer never holds more than one prefix and
    /// one maximum-size body.
    pub fn push(&mut self, bytes: &[u8]) -> Result<(), NativeMessagingError> {
        // Never hold more than two maximal frames, whatever the chunk size or
        // decoder state; the caller drains complete frames between pushes.
        if self.buffer.len().saturating_add(bytes.len()) > MAX_NATIVE_DECODER_BUFFER {
            return Err(NativeMessagingError::BufferFull);
        }
        self.buffer.extend_from_slice(bytes);
        if self.buffer.len() >= 4 {
            frame_length(&self.buffer)?;
        }
        Ok(())
    }

    /// Take the next complete frame body, if one is buffered.
    pub fn next_frame(&mut self) -> Result<Option<Vec<u8>>, NativeMessagingError> {
        if self.buffer.len() < 4 {
            return Ok(None);
        }
        let length = frame_length(&self.buffer)?;
        if self.buffer.len() < 4 + length {
            return Ok(None);
        }
        let body = self.buffer[4..4 + length].to_vec();
        self.buffer.drain(..4 + length);
        Ok(Some(body))
    }

    /// Call at end of stream: any buffered partial frame is truncation.
    pub fn finish(&self) -> Result<(), NativeMessagingError> {
        if self.buffer.is_empty() {
            Ok(())
        } else {
            Err(NativeMessagingError::Truncated)
        }
    }
}

fn frame_length(buffer: &[u8]) -> Result<usize, NativeMessagingError> {
    let length = u32::from_ne_bytes(buffer[..4].try_into().expect("four bytes")) as usize;
    if length == 0 {
        return Err(NativeMessagingError::EmptyFrame);
    }
    if length > MAX_NATIVE_FRAME_BYTES {
        return Err(NativeMessagingError::FrameTooLarge);
    }
    Ok(length)
}

/// Encode an outbound frame with the native-endian length prefix.
pub fn encode_frame(body: &[u8]) -> Result<Vec<u8>, NativeMessagingError> {
    if body.is_empty() {
        return Err(NativeMessagingError::EmptyFrame);
    }
    if body.len() > MAX_NATIVE_FRAME_BYTES {
        return Err(NativeMessagingError::FrameTooLarge);
    }
    let mut frame = (body.len() as u32).to_ne_bytes().to_vec();
    frame.extend_from_slice(body);
    Ok(frame)
}

/// Messages an extension may send. Unknown types and fields are refused.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ExtensionMessage {
    /// Opens a session; carries the pairing identity and a fresh client
    /// nonce (64 lowercase hex characters).
    Hello {
        protocol_version: u32,
        seq: u64,
        pairing_id: uuid::Uuid,
        extension_id: String,
        extension_key_digest: String,
        permissions_digest: String,
        client_nonce: String,
    },
    /// Every message after `hello` carries `mac`, the hex HMAC over
    /// [`ExtensionMessage::mac_input`] under the session key.
    Navigation {
        seq: u64,
        url: String,
        private_context: bool,
        transition: NavigationTransition,
        mac: String,
    },
    Heartbeat {
        seq: u64,
        mac: String,
    },
    Goodbye {
        seq: u64,
        mac: String,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NavigationTransition {
    Committed,
    Replaced,
    Redirected,
    HistoryUpdated,
}

impl ExtensionMessage {
    pub fn seq(&self) -> u64 {
        match self {
            Self::Hello { seq, .. }
            | Self::Navigation { seq, .. }
            | Self::Heartbeat { seq, .. }
            | Self::Goodbye { seq, .. } => *seq,
        }
    }

    /// The transmitted MAC, absent only for `hello`.
    pub fn mac(&self) -> Option<&str> {
        match self {
            Self::Hello { .. } => None,
            Self::Navigation { mac, .. }
            | Self::Heartbeat { mac, .. }
            | Self::Goodbye { mac, .. } => Some(mac),
        }
    }

    /// The canonical bytes a MAC covers. The browser serializes messages
    /// itself, so the MAC is defined over these typed fields rather than over
    /// JSON text; the extension builds the identical string.
    pub fn mac_input(&self) -> Option<Vec<u8>> {
        let text = match self {
            Self::Hello { .. } => return None,
            Self::Navigation { seq, url, private_context, transition, .. } => format!(
                "ghostrace-nm-v1\nnavigation\n{seq}\n{}\n{}\n{url}",
                u8::from(*private_context),
                transition.as_str()
            ),
            Self::Heartbeat { seq, .. } => format!("ghostrace-nm-v1\nheartbeat\n{seq}"),
            Self::Goodbye { seq, .. } => format!("ghostrace-nm-v1\ngoodbye\n{seq}"),
        };
        Some(text.into_bytes())
    }
}

impl NavigationTransition {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Committed => "committed",
            Self::Replaced => "replaced",
            Self::Redirected => "redirected",
            Self::HistoryUpdated => "history_updated",
        }
    }
}

/// Parse one frame body into a typed message after bounding its structure.
pub fn parse_message(body: &[u8]) -> Result<ExtensionMessage, NativeMessagingError> {
    let text = std::str::from_utf8(body).map_err(|_| NativeMessagingError::InvalidUtf8)?;
    scan_structure(text)?;
    serde_json::from_str::<ExtensionMessage>(text).map_err(|error| {
        if error.is_data() {
            NativeMessagingError::UnknownMessage
        } else {
            NativeMessagingError::MalformedJson
        }
    })
}

/// Count nesting depth and values without building a tree, so a deeply
/// nested or very wide payload is refused before serde allocates for it.
fn scan_structure(text: &str) -> Result<(), NativeMessagingError> {
    let mut depth = 0usize;
    let mut values = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    let mut in_scalar = false;
    for byte in text.bytes() {
        if in_string {
            match (escaped, byte) {
                (true, _) => escaped = false,
                (false, b'\\') => escaped = true,
                (false, b'"') => in_string = false,
                _ => {}
            }
            continue;
        }
        let scalar_byte = byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'+' | b'.');
        if scalar_byte {
            if !in_scalar {
                in_scalar = true;
                values += 1;
            }
            continue;
        }
        in_scalar = false;
        match byte {
            b'"' => {
                in_string = true;
                values += 1;
            }
            b'{' | b'[' => {
                depth += 1;
                values += 1;
                if depth > MAX_NATIVE_MESSAGE_DEPTH {
                    return Err(NativeMessagingError::TooDeep);
                }
            }
            b'}' | b']' => depth = depth.saturating_sub(1),
            _ => {}
        }
        if values > MAX_NATIVE_MESSAGE_VALUES {
            return Err(NativeMessagingError::TooManyValues);
        }
    }
    if in_string {
        return Err(NativeMessagingError::MalformedJson);
    }
    Ok(())
}

/// What the session concluded from one accepted message.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SessionEvent {
    /// The message is accepted in sequence.
    Accepted(ExtensionMessage),
    /// The message is accepted, but `missing` earlier messages never arrived
    /// and must be recorded as a gap.
    AcceptedAfterGap { message: ExtensionMessage, missing: u64 },
    /// The extension ended the session cleanly.
    Closed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SessionState {
    AwaitingHello,
    Open,
    Closed,
}

/// Per-connection protocol state. Time is supplied by the caller so the
/// deadline and rate rules are deterministic under test.
#[derive(Debug)]
pub struct ProtocolSession {
    state: SessionState,
    last_seq: u64,
    last_activity: Option<Duration>,
    recent: VecDeque<Duration>,
}

impl Default for ProtocolSession {
    fn default() -> Self {
        Self::new()
    }
}

impl ProtocolSession {
    pub fn new() -> Self {
        Self {
            state: SessionState::AwaitingHello,
            last_seq: 0,
            last_activity: None,
            recent: VecDeque::new(),
        }
    }

    /// Process one frame body received at monotonic time `now`.
    pub fn receive(
        &mut self,
        body: &[u8],
        now: Duration,
    ) -> Result<SessionEvent, NativeMessagingError> {
        if self.state == SessionState::Closed {
            return Err(NativeMessagingError::TrailingData);
        }
        if self
            .last_activity
            .is_some_and(|last| now.saturating_sub(last) > NATIVE_SESSION_IDLE_TIMEOUT)
        {
            self.state = SessionState::Closed;
            return Err(NativeMessagingError::Timeout);
        }
        while self.recent.front().is_some_and(|at| now.saturating_sub(*at) >= NATIVE_RATE_WINDOW) {
            self.recent.pop_front();
        }
        if self.recent.len() >= MAX_NATIVE_MESSAGES_PER_WINDOW {
            return Err(NativeMessagingError::RateLimited);
        }
        let message = parse_message(body)?;
        self.recent.push_back(now);
        self.last_activity = Some(now);

        match (&self.state, &message) {
            (
                SessionState::AwaitingHello,
                ExtensionMessage::Hello { protocol_version, seq, .. },
            ) => {
                if *protocol_version != NATIVE_MESSAGING_PROTOCOL_VERSION {
                    self.state = SessionState::Closed;
                    return Err(NativeMessagingError::UnsupportedVersion);
                }
                if *seq != 1 {
                    return Err(NativeMessagingError::Replay);
                }
                self.last_seq = 1;
                self.state = SessionState::Open;
                return Ok(SessionEvent::Accepted(message));
            }
            (SessionState::AwaitingHello, _) => return Err(NativeMessagingError::HelloRequired),
            (SessionState::Open, ExtensionMessage::Hello { .. }) => {
                // A second hello is a renegotiation attempt, including a
                // downgrade; the session ends and must be re-paired.
                self.state = SessionState::Closed;
                return Err(NativeMessagingError::DuplicateHello);
            }
            _ => {}
        }

        let seq = message.seq();
        if seq <= self.last_seq {
            return Err(NativeMessagingError::Replay);
        }
        let missing = seq - self.last_seq - 1;
        self.last_seq = seq;
        if matches!(message, ExtensionMessage::Goodbye { .. }) {
            self.state = SessionState::Closed;
            return Ok(SessionEvent::Closed);
        }
        Ok(if missing == 0 {
            SessionEvent::Accepted(message)
        } else {
            SessionEvent::AcceptedAfterGap { message, missing }
        })
    }
}
