#![deny(unsafe_code)]
//! Quantum-safe cryptography layer for the Arkhe Timechain.

pub mod kem;
pub mod sign;

pub mod auth_kem;
pub mod handshake;
pub mod wallet;

pub use auth_kem::{
    auth_decapsulate, auth_encapsulate, AuthCiphertext, AuthError, PqcIdentity, PqcKeyMaterial,
};
pub use handshake::{confirm_ss, initiator_finalize_ss, initiator_hello, responder_ack, verify_confirm};
pub use kem::{generate_kem_keypair, quantum_decapsulate, quantum_encapsulate};
pub use sign::{quantum_sign, quantum_verify, QuantumSignature, QuantumSigner};
pub use wallet::{identity_fingerprint, KeyStatus, PqcWallet, WalletEntry};

#[cfg(test)]
mod tests;
