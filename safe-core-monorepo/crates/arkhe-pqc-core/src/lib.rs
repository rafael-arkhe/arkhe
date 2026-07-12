//! Hybrid post-quantum cryptography via `pqcrypto-dilithium`/`pqcrypto-mlkem`
//! (PQClean C bindings).
//!
//! This is a **separate, parallel implementation** to
//! [`arkhe-crypto-pqc`](../arkhe_crypto_pqc/index.html), which uses the
//! pure-Rust `fips204`/`fips203` crates instead. Kept deliberately distinct
//! by explicit choice (not a replacement) — see each module's doc comment
//! for the concrete tradeoffs this crate choice carries, particularly
//! around secret-key zeroization.

#![deny(unsafe_code)]

pub mod hybrid;
pub mod kem;

pub use hybrid::{HybridKeyPair, HybridSignature, PqcSignatureError};
pub use kem::{decapsulate, encapsulate, KemKeyPair};
