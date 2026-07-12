# arkhe-nostr-anchor

FI-032 — Nostr identity roots: a real NIP-01 canonical event-id
computation and real BIP-340 Schnorr signing (`k256`), anchoring an
`arkhe_identity::Gdid` to a Nostr-format identity-root event.

## Status

7/7 tests pass. Verified: `../docs/verification/README.md` (run from
`safe-core-monorepo/`).

## What's actually here

- **`NostrIdentity`** — a real secp256k1/BIP-340 keypair (`k256::schnorr`).
  Separate from this workspace's Ed25519 + ML-DSA-65 hybrid identity keys
  (`arkhe-crypto-pqc`) — Nostr's wire format requires x-only secp256k1
  keys specifically; there is no valid conversion from an unrelated
  curve/scheme into one.
- **`compute_event_id`** — the real NIP-01 rule:
  `sha256(canonical_json([0, pubkey, created_at, kind, tags, content]))`.
  Uses `serde_json`'s compact (non-pretty) serializer, which produces no
  inter-token whitespace and does not escape non-ASCII bytes — matching
  the spec's serialization requirements without hand-rolled JSON escaping.
- **`root_identity`/`verify_root_event`** — builds a `NostrEvent` whose
  `content` is the hex-encoded `Gdid` bytes (a real data commitment, not a
  description of one) and whose `sig` is a genuine 64-byte BIP-340
  signature over `id`. `verify_root_event` recomputes `id` from the
  event's other fields first (catches tampering with any field, `id`
  included), then checks `sig` against it.

### A real prior stub, for contrast

An earlier, unrelated prototype elsewhere in this repository tree
(`arkhe-os/libs/nostr`) computed event IDs via SHA3-256 (the NIP-01 spec
requires SHA-256) and hardcoded `sig: "0".repeat(128)` — never a real
signature, and `NostrRelay::connect`/`publish` never opened a socket, just
set a bool and printed. This crate is a from-scratch, spec-correct
implementation of the event/signature layer, not a port of that code, and
does not include a relay client at all — see below.

## What's explicitly not here

- **No relay client.** No WebSocket connection, no `connect`/`publish`
  methods. This crate produces and verifies real, spec-shaped NIP-01
  events; sending them to a relay is a distinct transport concern,
  deliberately out of scope — matching `arkhe-network`'s own precedent of
  not doing live sockets (see that crate's README).
- **Not a general-purpose Nostr client.** No relay subscriptions, no other
  NIP-01 event kinds beyond the one identity-root kind this crate defines,
  no NIP-05/NIP-19 (`npub`/`nsec` bech32 encoding) support.
