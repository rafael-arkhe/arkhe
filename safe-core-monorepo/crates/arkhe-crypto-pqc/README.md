# arkhe-crypto-pqc

Hybrid post-quantum cryptography: Ed25519 + ML-DSA-65 (FIPS 204) signatures,
ML-KEM-1024 (FIPS 203) key encapsulation with HKDF-SHA256 derivation.

Uses [`fips204`](https://docs.rs/fips204) / [`fips203`](https://docs.rs/fips203)
— pure-Rust implementations, no C toolchain required. See
[`arkhe-pqc-core`](../arkhe-pqc-core) for a separate implementation of the
same idea using `pqcrypto-dilithium`/`pqcrypto-mlkem` (PQClean C bindings)
instead; the two are independent by design, not one superseding the other
— see [ADR-0001](../../docs/decisions/ADR-0001-two-pqc-crates.md) for why.

## Status

10/10 tests pass. Verified: `../docs/verification/README.md` (run from
`safe-core-monorepo/`).

## Zeroization

Both `fips204::ml_dsa_65::PrivateKey` and `fips203::ml_kem_1024::DecapsKey`
derive `Zeroize, ZeroizeOnDrop` natively (confirmed by reading their source
in the cargo registry cache), as does `ed25519_dalek::SigningKey`. No
wrapper-level `Drop` impl is added on top — one was tried and removed; it
zeroized a `.clone()`/`.to_vec()` copy of the secret instead of the real
one, which is a no-op that only looks like a security control. See
[`src/signature.rs`](src/signature.rs)'s doc comment for the full story.

## API

- [`signature::generate_hybrid_keypair`] / [`HybridSigningKey::sign`] /
  [`HybridVerifyingKey::verify`] — hybrid sign/verify, **both** sub-signatures
  must be valid.
- [`kem::generate_kem_keypair`] / [`kem_encapsulate`] / [`kem_decapsulate`] —
  ML-KEM-1024, with the shared secret run through HKDF-SHA256 before use as
  a symmetric key (never used raw).
