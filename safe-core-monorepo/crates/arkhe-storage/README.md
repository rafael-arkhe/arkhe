# arkhe-storage

FI-032 — content-addressed storage for agent-produced artifacts: convergent
CHK encryption, Merkle-verified chunked objects, and ML-KEM-1024
encapsulation of a `ChkKey` for sharing a stored object with one specific
recipient agent.

## Status

16/16 tests pass (`chk`: 12, `encapsulation`: 4). Verified:
`../docs/verification/README.md` (run from `safe-core-monorepo/`).

## Provenance

`chk.rs` is ported from `arkhe-workspace-v5/arkhe/crates/arkhe-storage` — a
sibling, unrelated Cargo workspace with its own real, tested (12/12)
implementation. This is the first time that code has been type-checked or
tested as part of `safe-core-monorepo`. Its `Cargo.toml` declared
`arkhe-core`/`arkhe-identity` as dependencies its code never actually used
(confirmed by reading the full source) — dropped here rather than carried
over as dead weight.

`encapsulation.rs` is new.

## What's actually here

- **`chk.rs`** — `chk_encode`/`chk_decode` (convergent CHK: the key is
  `hash(plaintext)`, so identical content naturally deduplicates),
  `ChunkStore`/`InMemoryStore` (rejects a write whose declared address
  doesn't match the ciphertext's hash), `merkle_root` (deterministic,
  order- and value-sensitive), `store_object`/`load_object` (chunk, encrypt,
  store, and — on load — reverify the Merkle root before returning any
  byte).
- **`encapsulation.rs`** (FI-032, new) — `encapsulate_chk_key`/
  `decapsulate_chk_key`: wraps a 32-byte `ChkKey` in a ChaCha20-Poly1305
  AEAD ciphertext, keyed by an HKDF-derived symmetric key from an
  ML-KEM-1024 (FIPS 203) shared secret encapsulated against a specific
  recipient's public key (`arkhe-crypto-pqc`'s `kem` module). Only the
  holder of the matching decapsulation key can recover the `ChkKey`. The
  AEAD nonce is derived (via SHA-256) from the KEM ciphertext rather than
  drawn from a new CSPRNG dependency — safe because `try_encaps` draws
  fresh internal randomness on every call, so the KEM ciphertext (and
  therefore the derived nonce) is already unique per encapsulation;
  `two_encapsulations_of_the_same_key_use_different_kem_ciphertexts_and_nonces`
  confirms this directly rather than asserting it from a comment.

Building this required a real, previously-missing piece in
`arkhe-crypto-pqc`: `KemKeypair::encaps_key_bytes` let you serialize *your
own* public key to send to someone else, but there was no way to
reconstruct a *recipient's* public key from bytes you received — every
existing caller already held a live `KemKeypair`, so nothing had needed
this yet. Added `arkhe_crypto_pqc::kem::encaps_key_from_bytes` (with its
own roundtrip test) to close that gap.

## Cryptographic limits (labeled, not hidden)

- `chk.rs`'s keystream is deterministic (SHA-256 counter mode) — **not
  AEAD**. There is no MAC-based integrity check for local, at-rest storage
  using `chk.rs` alone; `ChunkStore::put` only rejects an
  address/ciphertext mismatch, and `load_object` only rejects a tampered
  Merkle root. `encapsulation.rs` adds real AEAD, but only for the
  cross-agent key-sharing path — it does not retroactively change anything
  about storing/reading chunks locally through `chk.rs` directly.
- The convergent `ChkKey` is *supposed* to be derivable by anyone who
  already has the plaintext — that is what gives deduplication. It is not
  a secret with respect to the plaintext; `encapsulation.rs` protects it
  only *in transit* to a recipient who does not yet have the plaintext.

## What's explicitly not here

- **No P2P transport, no network I/O.** This crate is data model and
  cryptography only. See `arkhe-network`'s `chunk_share` module for the
  message-level exchange protocol built on top of this — which is itself
  not a live socket/transport layer either (see that crate's README).
- **No persistent/on-disk store.** `InMemoryStore` is the only
  `ChunkStore` implementation here; a real deployment would need a
  disk- or database-backed one implementing the same trait.
