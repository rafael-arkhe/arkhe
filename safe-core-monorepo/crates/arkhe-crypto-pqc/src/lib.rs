//! Hybrid post-quantum cryptography for Arkhe Web3 security (FI-W01, FI-W02).
//!
//! Pairs a classical primitive with its FIPS-standardized post-quantum
//! counterpart so that breaking either scheme alone is not enough to forge a
//! signature or recover a shared secret:
//!
//! - [`signature`] — ML-DSA-65 (FIPS 204) + Ed25519 hybrid signing.
//! - [`kem`] — ML-KEM-1024 (FIPS 203) key encapsulation, with HKDF-SHA256
//!   derivation of a usable symmetric key from the shared secret.
//! - [`context`] — FI-004: context-bound signing (`sign(msg, context)`
//!   where `context = hash(domain, timestamp, nonce, policy_version)`).
//!
//! Uses the pure-Rust `fips204`/`fips203` crates (no C toolchain dependency),
//! whose module names (`ml_dsa_65`, `ml_kem_1024`) match the FIPS-final
//! algorithm names directly.

#![deny(unsafe_code)]

pub mod context;
pub mod kem;
pub mod signature;

pub use context::{sign_with_context, verify_with_context, SigningContext};
pub use kem::{
    encaps_key_from_bytes, generate_kem_keypair, kem_decapsulate, kem_encapsulate, KemError,
    KemKeypair, SymmetricKey,
};
pub use signature::{
    generate_hybrid_keypair, HybridKeyPair, HybridSignature, HybridSigningKey,
    HybridVerifyingKey, PqcSignatureError,
};
