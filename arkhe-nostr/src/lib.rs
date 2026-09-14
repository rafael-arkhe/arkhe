//! Nostr-compatible event semantics with an Arkhe PQC attestation layer.
//!
//! # Scope and honesty
//! A native Nostr `event.sig` is BIP-340 Schnorr over secp256k1. That is outside
//! this crate's pure-Rust, no-C dependency floor and is NOT the same as an
//! ML-DSA lattice signature. So this crate:
//!
//! 1. Produces a **NIP-01-shaped event**: canonical JSON framing and a
//!    root-reproducible `id = SHA-256([0, pubkey, created_at, kind, tags, content])`
//!    that Nostr tooling can parse and re-derive.
//! 2. Attaches an **Arkhe attestation** (ML-DSA over the event `id`) that the
//!    Arkhe network can verify. Standard Nostr relays will not accept this as
//!    their demo `sig`; it is a complementary Arkhe-side seal.

use arkhe_pqc::sign::{quantum_verify, QuantumSigner, SignatureError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .filter_map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
        .collect()
}

/// NIP-01 canonical serialization used to compute an event `id`.
pub fn canonical_json(
    pubkey: &str,
    created_at: i64,
    kind: u16,
    tags: &[Vec<String>],
    content: &str,
) -> String {
    serde_json::json!([0, pubkey, created_at, kind, tags, content]).to_string()
}

/// Compute the NIP-01 event id (hex) for a canonical field set.
pub fn event_id(
    pubkey: &str,
    created_at: i64,
    kind: u16,
    tags: &[Vec<String>],
    content: &str,
) -> String {
    let canonical = canonical_json(pubkey, created_at, kind, tags, content);
    hex(Sha256::digest(canonical.as_bytes()))
}

/// A NIP-01-shaped Nostr event.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NostrEvent {
    pub id: String,
    pub pubkey: String,
    pub created_at: i64,
    pub kind: u16,
    pub tags: Vec<Vec<String>>,
    pub content: String,
}

impl NostrEvent {
    pub fn new(pubkey: &str, created_at: i64, kind: u16, tags: Vec<Vec<String>>, content: &str) -> Self {
        let id = event_id(pubkey, created_at, kind, &tags, content);
        Self { id, pubkey: pubkey.into(), created_at, kind, tags, content }
    }

    /// Re-derive and check the NIP-01 id independent of the stored one.
    pub fn verify_id(&self) -> bool {
        event_id(&self.pubkey, self.created_at, self.kind, &self.tags, &self.content) == self.id
    }
}

/// Arkhe ML-DSA attestation over an event id.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Attestation {
    pub signature: Vec<u8>,
    pub dsa_public_hex: String,
}

/// A Nostr-shaped event sealed with an Arkhe (ML-DSA) attestation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ArkheEvent {
    pub event: NostrEvent,
    pub attestation: Attestation,
    pub agent: Option<String>,
}

impl ArkheEvent {
    /// Seal an event: sign its NIP-01 id with an ML-DSA signer.
    pub fn seal(event: NostrEvent, agent: Option<String>, signer: &QuantumSigner) -> Self {
        let id_bytes = unhex(&event.id);
        let sig = signer.sign(&id_bytes);
        Self {
            event,
            attestation: Attestation {
                signature: sig.signature.clone(),
                dsa_public_hex: hex(&sig.public),
            },
            agent,
        }
    }

    /// Verify the event id framing and the ML-DSA attestation.
    pub fn verify(&self, dsa_public: &[u8]) -> Result<(), SignatureError> {
        if !self.event.verify_id() {
            return Err(SignatureError::Mismatch);
        }
        let id_bytes = unhex(&self.event.id);
        quantum_verify(dsa_public, &id_bytes, &self.attestation.signature)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arkhe_pqc::sign::QuantumSigner;

    #[test]
    fn event_id_changes_with_fields() {
        let a = event_id("pk", 100, 1, &[], "orig");
        let b = event_id("pk", 100, 1, &[], "tampered");
        assert_ne!(a, b);
    }

    #[test]
    fn nostr_event_id_reproducible() {
        let e1 = NostrEvent::new("pkx", 99, 7, vec![], "hello");
        // same inputs => same canonical id
        let canonical = canonical_json("pkx", 99, 7, &[], "hello");
        assert_eq!(e1.id, hex(Sha256::digest(canonical.as_bytes())));
    }

    #[test]
    fn seal_and_verify_attestation() {
        let signer = QuantumSigner::new();
        let event = NostrEvent::new("pk", 99, 7, vec![], "seed state");
        let arkhe = ArkheEvent::seal(event, Some("S15".into()), &signer);
        arkhe.verify(&signer.public()).expect("seal verifies");
    }

    #[test]
    fn mismatched_public_key_fails() {
        let alice = QuantumSigner::new();
        let bob = QuantumSigner::new();
        let event = NostrEvent::new("pk", 1, 1, vec![], "m");
        let arkhe = ArkheEvent::seal(event, None, &alice);
        assert!(arkhe.verify(&bob.public()).is_err());
    }
}