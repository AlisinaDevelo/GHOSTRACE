# ADR 0007: Native reconnect recovery

- **Status:** Proposed
- **Date:** 2026-10-11
- **Scope:** Native-messaging acknowledgment and reconnect semantics

## Context

Resetting the protocol cursor to one on reconnect changes the gap count for a
later outstanding delivery. For example, after sequence two is acknowledged,
sequence three may commit while its reply is lost. Retrying sequence three
then invents one missing message, conflicting with the durable delivery.

## Decision

The extension uses stop-and-wait, with one outstanding input. Each successfully
processed authenticated navigation, heartbeat, or policy refusal returns an
opaque processed-sequence receipt. Navigation waits for the validated durable
service acknowledgment; a control receipt proves processing only. Errors and
uncertain delivery issue no receipt.

The receipt contains a version byte, an eight-byte sequence, and a 32-byte MAC,
encoded as exactly 82 lowercase hex characters. Its domain-separated MAC key
derives from host-only pairing-store custody, independently of the extension
credential, and binds the pairing ID, delivery nonce, and processed sequence.
It contains no URL or private-page data and is not journaled.

The extension retains the latest successful receipt and supplies it in the
optional `hello.resume_receipt` field on reconnect with the same delivery nonce.
Only a verified host receipt can restore the cursor. A fresh host nonce still
requires a new message MAC; expiry, revocation, and conflicting durable payload
or gap checks continue to apply. Pipelining and stale-receipt transcript
reconstruction are unsupported.

The updated Rust reply parser accepts old v1 replies with no receipt as `None`,
which authorizes no resume cursor. Older strict reply parsers reject the added
response field and require migration. No extension/client is shipped by this
slice; bidirectional old-binary compatibility is not claimed.
