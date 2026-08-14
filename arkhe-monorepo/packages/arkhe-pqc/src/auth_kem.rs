//! FIPS 203/204 authenticated key encapsulation: KEM-then-Sign.
//! Identities are bound by signing the transcript, so the receiver
//! authenticates the sender before deriving the shared secret.

use crate::kem::{quantum_encapsulate, quantum_decapsulate, QuantumKem, KemError};
use crate::sign::{quantum_verify, QuantumSignature, QuantumSigner, SignatureError};
use sha2::{Digest, Sha256};

#[derive(Debug)]
pub enum AuthError {
    Kem(KemError),
    Signature(SignatureError),
    IdentityMismatch,
}

impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "authenticated kem error: {self:?}")
    }
}

impl std::error::Error for AuthError {}

/// A party's public identity: an ML-DSA verifying key plus an ML-KEM
/// encapsulation key, fingerprinted for transport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PqcIdentity {
    pub dsa_public: Vec<u8>,
    pub kem_public: Vec<u8>,
}

impl PqcIdentity {
    /// Build a binding label from the two public keys (canonical order).
    fn fingerprint(&self) -> Vec<u8> {
        let mut h = Sha256::new();
        h.update(&self.dsa_public);
        h.update(&self.kem_public);
        h.finalize().to_vec()
    }

    /// Human/transport-safe binding tag (hex of the SHA-256 fingerprint).
    pub fn fingerprint_hex(&self) -> String {
        self.fingerprint().iter().map(|b| format!("{b:02x}")).collect()
    }
}

/// A full PQC keypair (decapsulation secret + public identity).
pub struct PqcKeyMaterial {
    pub kem: QuantumKem,
    pub signer: QuantumSigner,
    pub identity: PqcIdentity,
}

impl PqcKeyMaterial {
    /// Generate a fresh ML-DSA signer + ML-KEM keypair and their identity.
    pub fn generate() -> Self {
        let kem = crate::kem::generate_kem_keypair();
        let signer = QuantumSigner::new();
        let identity = PqcIdentity {
            dsa_public: signer.public(),
            kem_public: kem.encapsulation.clone(),
        };
        Self { kem, signer, identity }
    }
}

/// Ciphertext of an authenticated encapsulation, carrying the sender's
/// signature over `label || ciphertext`.
#[derive(Debug, Clone)]
pub struct AuthCiphertext {
    pub ciphertext: Vec<u8>,
    pub signature: QuantumSignature,
    pub label: Vec<u8>,
}

fn bind_label(label: &[u8], ct: &[u8]) -> Vec<u8> {
    let mut m = Vec::with_capacity(label.len() + ct.len());
    m.extend_from_slice(label);
    m.extend_from_slice(ct);
    m
}

/// Authenticated encapsulation: obtain a shared secret with `receiver`
/// while proving possession of `sender`'s signing key.
pub fn auth_encapsulate(
    sender: &QuantumSigner,
    receiver: &PqcIdentity,
    label: &[u8],
) -> Result<(AuthCiphertext, Vec<u8>), AuthError> {
    let (ct, ss) = quantum_encapsulate(&receiver.kem_public).map_err(AuthError::Kem)?;
    let msg = bind_label(label, &ct);
    let signature = sender.sign(&msg);
    Ok((AuthCiphertext { ciphertext: ct, signature, label: label.to_vec() }, ss))
}

/// Authenticated decapsulation: verify that `auth` was produced by the party
/// holding `sender`'s DSA key, then recover the shared secret with `receiver`.
pub fn auth_decapsulate(
    sender_identity: &PqcIdentity,
    receiver_decapsulation_key: &[u8],
    auth: &AuthCiphertext,
) -> Result<Vec<u8>, AuthError> {
    if sender_identity.dsa_public != auth.signature.public {
        return Err(AuthError::IdentityMismatch);
    }
    let msg = bind_label(&auth.label, &auth.ciphertext);
    quantum_verify(&sender_identity.dsa_public, &msg, &auth.signature.signature)
        .map_err(AuthError::Signature)?;
    quantum_decapsulate(receiver_decapsulation_key, &auth.ciphertext).map_err(AuthError::Kem)
}

/// Sign `msg` and bind it to a PQC identity (fresh keypair).
pub fn sign_authenticated(signer: &QuantumSigner, kem_public: &[u8], msg: &[u8]) -> SignedArtifact {
    let signature = signer.sign(msg);
    let identity = PqcIdentity {
        dsa_public: signature.public.clone(),
        kem_public: kem_public.to_vec(),
    };
    SignedArtifact { identity, signature, message: msg.to_vec() }
}

#[derive(Debug, Clone)]
pub struct SignedArtifact {
    pub identity: PqcIdentity,
    pub signature: QuantumSignature,
    pub message: Vec<u8>,
}