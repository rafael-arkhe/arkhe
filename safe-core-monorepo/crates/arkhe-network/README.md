# arkhe-network

FI-071 (authenticated messages) / FI-075 (unique nonces) / FI-077 (no
message causes a panic) / FI-078 (TLS 1.3+) — a deliberately **minimal**
slice, not a full P2P/transport stack.

## Status

14/14 tests pass (up from 11 — `handler.rs`/FI-077 dispatch-level added this
pass). Verified: `../docs/verification/README.md` (run from
`safe-core-monorepo/`).

## What's actually here

- **`message.rs`** (FI-071 + FI-077, parsing level) — `Message`/`SignedMessage`,
  signed via `arkhe-crypto-pqc`'s hybrid signatures. `Message::decode` never
  panics on malformed input — every slicing operation is preceded by an
  explicit bounds check, confirmed (not just claimed) by
  `decode_never_panics_on_arbitrary_bytes`, which runs 8 malformed byte
  sequences through `std::panic::catch_unwind`.
- **`handler.rs`** (FI-077, dispatch level) — `MessageHandler` trait +
  `dispatch()`, which runs a handler inside `catch_unwind` and converts a
  panic into `Err(DispatchError::Panic)` instead of letting it propagate.
  `message.rs` already covers panic-safety in *parsing*; this covers
  whatever a handler actually *does* with an already-decoded message —
  arbitrary caller code, which can panic for reasons parsing-level
  hardening can't prevent (an internal `unwrap()`, an index out of bounds
  in handler-specific logic, ...). `panicking_handler_does_not_abort_dispatch`
  confirms this directly: it wraps the whole `dispatch()` call in its own
  `catch_unwind` and asserts *that* doesn't panic either.
- **`nonce.rs`** (FI-075) — `MessageNonceTracker`, the same
  strictly-monotonic-per-sender pattern as FI-007
  (`arkhe-web3-security::wallets::signature::NonceTracker`, formally proven
  in Lean), reimplemented locally rather than depending on
  `arkhe-web3-security` — a generic networking primitive depending on a
  Web3-specific invariants crate would be backwards layering for ~15 lines
  of logic.
- **`tls.rs`** (FI-078) — `tls_1_3_only_client_config`, a real
  `rustls::ClientConfig` built via
  `ClientConfig::builder_with_protocol_versions(&[&rustls::version::TLS13])`
  — TLS 1.2 and earlier are refused at the protocol-negotiation level, not
  merely deprioritized.

## What's explicitly not here

- **No socket/connection code.** This builds message types and a TLS
  *config*, not a listener/dialer. No actual bytes cross a network in this
  crate.
- **No server-side TLS config.** `rustls::ServerConfig::with_single_cert`
  needs real certificate/key material, which this pass doesn't fabricate.
  Wiring that up needs a certificate provisioning story (e.g. `rcgen` for
  dev/test certs, a real CA-issued cert for production) — a distinct piece
  of work.
- **No peer discovery, no transport, no P2P swarm.** "arkhe-network" is a
  suggestive name for what could grow here, not a claim that a network
  layer exists yet.

If a real transport/P2P layer is wanted, that's a substantially larger,
separate piece of work than this pass.
