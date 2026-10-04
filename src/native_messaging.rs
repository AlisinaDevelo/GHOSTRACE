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
/// Most fields accepted in one JSON object, including the tagged message type.
pub const MAX_NATIVE_MESSAGE_FIELDS: usize = 8;
/// Largest decoded string accepted in one message field.
pub const MAX_NATIVE_MESSAGE_STRING_BYTES: usize = 8 * 1024;
/// Total decoded string budget for one message, including object keys.
pub const MAX_NATIVE_MESSAGE_STRING_BUDGET_BYTES: usize = 16 * 1024;
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
    #[error("message has too many object fields")]
    TooManyFields,
    #[error("message repeats an object field")]
    DuplicateField,
    #[error("a message string exceeds the field bound")]
    StringTooLong,
    #[error("message strings exceed the allocation budget")]
    StringBudgetExceeded,
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

    /// Append received bytes. The buffer never exceeds the bounded decoder
    /// window, including its allocated capacity.
    pub fn push(&mut self, bytes: &[u8]) -> Result<(), NativeMessagingError> {
        // Never hold more than two maximal frames, whatever the chunk size or
        // decoder state; the caller drains complete frames between pushes.
        let new_len =
            self.buffer.len().checked_add(bytes.len()).ok_or(NativeMessagingError::BufferFull)?;
        if new_len > MAX_NATIVE_DECODER_BUFFER {
            return Err(NativeMessagingError::BufferFull);
        }
        // Reject the complete chunk's resource budget before inspecting its
        // prefix. Neither check allocates or retains an untrusted body.
        pending_frame_length(&self.buffer, bytes)?;
        // `extend_from_slice` is permitted to grow geometrically. Choose a
        // bounded geometric target ourselves, then reserve that exact target
        // fallibly so growth remains amortized without ever jumping over the
        // advertised decoder bound.
        if self.buffer.capacity() > MAX_NATIVE_DECODER_BUFFER {
            return Err(NativeMessagingError::BufferFull);
        }
        if self.buffer.capacity() < new_len {
            let target_capacity = self
                .buffer
                .capacity()
                .saturating_mul(2)
                .max(new_len)
                .min(MAX_NATIVE_DECODER_BUFFER);
            let additional = target_capacity
                .checked_sub(self.buffer.len())
                .ok_or(NativeMessagingError::BufferFull)?;
            self.buffer
                .try_reserve_exact(additional)
                .map_err(|_| NativeMessagingError::BufferFull)?;
        }
        self.buffer.extend_from_slice(bytes);
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

    /// Whether bytes remain buffered after the last complete frame.
    pub fn has_buffered_data(&self) -> bool {
        !self.buffer.is_empty()
    }

    /// Current allocation capacity, exposed so resource-bound tests can pin the
    /// distinction between buffered length and allocated capacity.
    pub fn buffered_capacity(&self) -> usize {
        self.buffer.capacity()
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

fn pending_frame_length(
    buffer: &[u8],
    incoming: &[u8],
) -> Result<Option<usize>, NativeMessagingError> {
    if buffer.len() >= 4 {
        return frame_length(buffer).map(Some);
    }
    let needed = 4 - buffer.len();
    if incoming.len() < needed {
        return Ok(None);
    }
    let mut prefix = [0u8; 4];
    prefix[..buffer.len()].copy_from_slice(buffer);
    prefix[buffer.len()..].copy_from_slice(&incoming[..needed]);
    frame_length(&prefix).map(Some)
}

/// Encode an outbound frame with the native-endian length prefix.
pub fn encode_frame(body: &[u8]) -> Result<Vec<u8>, NativeMessagingError> {
    if body.is_empty() {
        return Err(NativeMessagingError::EmptyFrame);
    }
    if body.len() > MAX_NATIVE_FRAME_BYTES {
        return Err(NativeMessagingError::FrameTooLarge);
    }
    let total = body.len().checked_add(4).ok_or(NativeMessagingError::BufferFull)?;
    let mut frame = Vec::new();
    frame.try_reserve_exact(total).map_err(|_| NativeMessagingError::BufferFull)?;
    frame.extend_from_slice(&(body.len() as u32).to_ne_bytes());
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
    if body.is_empty() {
        return Err(NativeMessagingError::EmptyFrame);
    }
    if body.len() > MAX_NATIVE_FRAME_BYTES {
        return Err(NativeMessagingError::FrameTooLarge);
    }
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

/// Scan JSON structure without building a tree, so a deeply nested, very wide,
/// or allocation-heavy payload is refused before typed deserialization.
fn scan_structure(text: &str) -> Result<(), NativeMessagingError> {
    let mut scanner = JsonStructureScanner::new(text.as_bytes());
    scanner.skip_whitespace();
    scanner.scan_value(0)?;
    scanner.skip_whitespace();
    if scanner.position != scanner.input.len() {
        return Err(NativeMessagingError::MalformedJson);
    }
    Ok(())
}

struct JsonStructureScanner<'a> {
    input: &'a [u8],
    position: usize,
    values: usize,
    string_budget: usize,
}

impl<'a> JsonStructureScanner<'a> {
    fn new(input: &'a [u8]) -> Self {
        Self { input, position: 0, values: 0, string_budget: 0 }
    }

    fn skip_whitespace(&mut self) {
        while self.position < self.input.len()
            && matches!(self.input[self.position], b' ' | b'\t' | b'\r' | b'\n')
        {
            self.position += 1;
        }
    }

    fn scan_value(&mut self, depth: usize) -> Result<(), NativeMessagingError> {
        self.skip_whitespace();
        self.values = self.values.saturating_add(1);
        if self.values > MAX_NATIVE_MESSAGE_VALUES {
            return Err(NativeMessagingError::TooManyValues);
        }
        let Some(byte) = self.input.get(self.position).copied() else {
            return Err(NativeMessagingError::MalformedJson);
        };
        match byte {
            b'{' => self.scan_object(depth + 1),
            b'[' => self.scan_array(depth + 1),
            b'"' => {
                self.scan_string()?;
                Ok(())
            }
            b't' => self.scan_literal(b"true"),
            b'f' => self.scan_literal(b"false"),
            b'n' => self.scan_literal(b"null"),
            b'-' | b'0'..=b'9' => self.scan_number(),
            _ => Err(NativeMessagingError::MalformedJson),
        }
    }

    fn scan_object(&mut self, depth: usize) -> Result<(), NativeMessagingError> {
        if depth > MAX_NATIVE_MESSAGE_DEPTH {
            return Err(NativeMessagingError::TooDeep);
        }
        self.position += 1;
        self.skip_whitespace();
        if self.consume(b'}') {
            return Ok(());
        }

        let mut fields = 0usize;
        let mut keys = [None; MAX_NATIVE_MESSAGE_FIELDS];
        loop {
            self.skip_whitespace();
            if self.input.get(self.position) != Some(&b'"') {
                return Err(NativeMessagingError::MalformedJson);
            }
            self.values = self.values.saturating_add(1);
            if self.values > MAX_NATIVE_MESSAGE_VALUES {
                return Err(NativeMessagingError::TooManyValues);
            }
            let key_start = self.position;
            self.scan_string()?;
            let key_end = self.position;
            // All v1 field names are plain ASCII identifiers. Refusing an
            // escaped key keeps the pre-scan's duplicate check semantic: a
            // key such as `"ty\\u0070e"` cannot alias `"type"` after serde
            // decodes it.
            if self.input[key_start..key_end].contains(&b'\\') {
                return Err(NativeMessagingError::MalformedJson);
            }
            fields = fields.saturating_add(1);
            if fields > MAX_NATIVE_MESSAGE_FIELDS {
                return Err(NativeMessagingError::TooManyFields);
            }
            if keys[..fields - 1]
                .iter()
                .flatten()
                .any(|(start, end)| self.input[*start..*end] == self.input[key_start..key_end])
            {
                return Err(NativeMessagingError::DuplicateField);
            }
            keys[fields - 1] = Some((key_start, key_end));
            self.skip_whitespace();
            if !self.consume(b':') {
                return Err(NativeMessagingError::MalformedJson);
            }
            self.scan_value(depth)?;
            self.skip_whitespace();
            if self.consume(b'}') {
                return Ok(());
            }
            if !self.consume(b',') {
                return Err(NativeMessagingError::MalformedJson);
            }
        }
    }

    fn scan_array(&mut self, depth: usize) -> Result<(), NativeMessagingError> {
        if depth > MAX_NATIVE_MESSAGE_DEPTH {
            return Err(NativeMessagingError::TooDeep);
        }
        self.position += 1;
        self.skip_whitespace();
        if self.consume(b']') {
            return Ok(());
        }
        loop {
            self.scan_value(depth)?;
            self.skip_whitespace();
            if self.consume(b']') {
                return Ok(());
            }
            if !self.consume(b',') {
                return Err(NativeMessagingError::MalformedJson);
            }
        }
    }

    fn scan_string(&mut self) -> Result<(), NativeMessagingError> {
        if !self.consume(b'"') {
            return Err(NativeMessagingError::MalformedJson);
        }
        let mut bytes = 0usize;
        loop {
            let Some(byte) = self.input.get(self.position).copied() else {
                return Err(NativeMessagingError::MalformedJson);
            };
            self.position += 1;
            let bytes_before = bytes;
            match byte {
                b'"' => return Ok(()),
                b'\\' => {
                    let Some(escaped) = self.input.get(self.position).copied() else {
                        return Err(NativeMessagingError::MalformedJson);
                    };
                    self.position += 1;
                    match escaped {
                        b'"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r' | b't' => {
                            bytes = bytes.saturating_add(1);
                        }
                        b'u' => {
                            if self.position.saturating_add(4) > self.input.len()
                                || !self.input[self.position..self.position + 4]
                                    .iter()
                                    .all(|digit| digit.is_ascii_hexdigit())
                            {
                                return Err(NativeMessagingError::MalformedJson);
                            }
                            self.position += 4;
                            // A Unicode escape expands to at most three UTF-8
                            // bytes for one code point. Over-counting a
                            // surrogate pair is safe for this pre-allocation
                            // budget and keeps the scanner allocation-free.
                            bytes = bytes.saturating_add(3);
                        }
                        _ => return Err(NativeMessagingError::MalformedJson),
                    }
                }
                byte if byte < 0x20 => return Err(NativeMessagingError::MalformedJson),
                _ => bytes = bytes.saturating_add(1),
            }
            if bytes > MAX_NATIVE_MESSAGE_STRING_BYTES {
                return Err(NativeMessagingError::StringTooLong);
            }
            self.string_budget =
                self.string_budget.saturating_add(bytes.saturating_sub(bytes_before));
            if self.string_budget > MAX_NATIVE_MESSAGE_STRING_BUDGET_BYTES {
                return Err(NativeMessagingError::StringBudgetExceeded);
            }
        }
    }

    fn scan_literal(&mut self, literal: &[u8]) -> Result<(), NativeMessagingError> {
        let end = self.position.saturating_add(literal.len());
        if end > self.input.len() || self.input[self.position..end] != *literal {
            return Err(NativeMessagingError::MalformedJson);
        }
        self.position = end;
        Ok(())
    }

    fn scan_number(&mut self) -> Result<(), NativeMessagingError> {
        let start = self.position;
        while let Some(byte) = self.input.get(self.position).copied() {
            if matches!(byte, b' ' | b'\t' | b'\r' | b'\n' | b',' | b']' | b'}') {
                break;
            }
            self.position += 1;
        }
        if self.position == start {
            Err(NativeMessagingError::MalformedJson)
        } else {
            Ok(())
        }
    }

    fn consume(&mut self, expected: u8) -> bool {
        if self.input.get(self.position) == Some(&expected) {
            self.position += 1;
            true
        } else {
            false
        }
    }
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
    started_at: Option<Duration>,
    last_activity: Option<Duration>,
    recent_attempts: VecDeque<Duration>,
}

impl Default for ProtocolSession {
    fn default() -> Self {
        Self::new()
    }
}

impl ProtocolSession {
    pub fn new() -> Self {
        Self::new_with_start(None)
    }

    /// Create a session with a known connection start time. The native host
    /// uses this before blocking on stdin so a peer that never sends `hello`
    /// still reaches the idle deadline.
    pub fn new_at(started_at: Duration) -> Self {
        Self::new_with_start(Some(started_at))
    }

    fn new_with_start(started_at: Option<Duration>) -> Self {
        Self {
            state: SessionState::AwaitingHello,
            last_seq: 0,
            started_at,
            last_activity: None,
            recent_attempts: VecDeque::new(),
        }
    }

    /// Check the connection and session deadline without requiring a frame.
    pub fn check_deadline(&mut self, now: Duration) -> Result<(), NativeMessagingError> {
        if self.state == SessionState::Closed {
            return Err(NativeMessagingError::TrailingData);
        }
        self.started_at.get_or_insert(now);
        let anchor = self.last_activity.or(self.started_at);
        if anchor.is_some_and(|last| now.saturating_sub(last) >= NATIVE_SESSION_IDLE_TIMEOUT) {
            self.state = SessionState::Closed;
            return Err(NativeMessagingError::Timeout);
        }
        Ok(())
    }

    /// Account for one complete inbound frame before parsing or authentication.
    /// The native host calls this for every frame, including malformed and
    /// unauthenticated input, so rejection cannot bypass the rate bound.
    pub fn admit_attempt(&mut self, now: Duration) -> Result<(), NativeMessagingError> {
        self.check_deadline(now)?;
        while self
            .recent_attempts
            .front()
            .is_some_and(|at| now.saturating_sub(*at) >= NATIVE_RATE_WINDOW)
        {
            self.recent_attempts.pop_front();
        }
        if self.recent_attempts.len() >= MAX_NATIVE_MESSAGES_PER_WINDOW {
            return Err(NativeMessagingError::RateLimited);
        }
        self.recent_attempts.push_back(now);
        Ok(())
    }

    /// Process one frame body received at monotonic time `now`.
    pub fn receive(
        &mut self,
        body: &[u8],
        now: Duration,
    ) -> Result<SessionEvent, NativeMessagingError> {
        self.admit_attempt(now)?;
        self.receive_after_admission(body, now)
    }

    /// Process a frame after the caller has already admitted it to the ingress
    /// budget. This is the native-host seam: pairing/MAC validation can happen
    /// after the attempt is counted without charging valid frames twice.
    pub fn receive_after_admission(
        &mut self,
        body: &[u8],
        now: Duration,
    ) -> Result<SessionEvent, NativeMessagingError> {
        self.check_deadline(now)?;
        let message = parse_message(body)?;
        self.started_at.get_or_insert(now);

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
                // Only a successfully admitted hello starts activity. A
                // valid-but-rejected pre-hello frame must not slide the
                // connection-start deadline indefinitely.
                self.last_activity = Some(now);
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
        // Sequence and message validation completed, so this is genuine
        // open-session activity. Replays, malformed messages, and duplicate
        // hellos above never refresh the idle anchor.
        self.last_activity = Some(now);
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

    /// Last sequence number accepted by this connection. The native bridge
    /// uses it to derive a stable delivery identity for reconnect retries.
    pub fn last_sequence(&self) -> u64 {
        self.last_seq
    }
}
