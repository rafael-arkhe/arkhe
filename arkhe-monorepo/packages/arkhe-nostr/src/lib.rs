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
//!    their primary `sig`; it is a complementary Arkhe-side seal.

use arkhe_pqc::sign::{quantum_verify, QuantumSigner, SignatureError};
use arkhe_pqc::auth_kem::PqcIdentity;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sha3::Sha3_256;

/// Arkhe Nostr event kinds (whitepaper §3.1).
pub mod kinds {
    pub const ANNOUNCE: u16 = 38000;
    pub const HANDOVER: u16 = 38001;
    pub const ECHO: u16 = 38002;
    pub const ATTEST: u16 = 38003;
    pub const SHADOW: u16 = 38004;
    pub const GOVERNANCE: u16 = 38005;
    pub const PQCHANDSHAKE: u16 = 38006;
}

/// Transport routing mode (whitepaper §6.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportMode {
    NostrPrimary,
    Hybrid,
    UdpFallback,
}

impl TransportMode {
    /// Select a mode from the fraction of healthy relays (whitepaper §6.2).
    pub fn from_health(healthy: f64) -> Self {
        if healthy > 0.8 {
            TransportMode::NostrPrimary
        } else if healthy > 0.3 {
            TransportMode::Hybrid
        } else {
            TransportMode::UdpFallback
        }
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .filter_map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
        .collect()
}

/// Derive a 32-byte synthetic Nostr `pubkey` (hex) from an Arkhe PQC identity
/// (whitepaper §2.2). This is `SHA3-256(ml_dsa_pk || ml_kem_pk)` truncated to
/// 32 bytes, **not** a secp256k1 key — it cannot produce a BIP-340 `sig`, so
/// standard relays treat it as non-signing. Arkhe-side verification uses the
/// ML-DSA key directly.
pub fn pqc_to_pubkey(identity: &PqcIdentity) -> String {
    let mut h = Sha3_256::new();
    h.update(&identity.dsa_public);
    h.update(&identity.kem_public);
    let digest = h.finalize();
    hex(&digest[..32])
}

/// Encode a 32-byte raw value as a bech32 string with the given human-readable
/// part (e.g. "npub"). Verbatim bech32 requires a checksum; this returns the
/// hex encoding so callers can render `npub1...` only via a real bech32 lib.
/// The whitepaper's inline `bech32::encode` helper assumes a crate that is not
/// part of this pure-Rust dependency floor, so we expose the canonical hex form.
pub fn to_bech32_hrp(hrp: &str, payload: &[u8]) -> String {
    format!("{hrp}1{}", hex(payload))
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
    hex(&Sha256::digest(canonical.as_bytes())[..])
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
        Self { id, pubkey: pubkey.into(), created_at, kind, tags, content: content.into() }
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

impl From<NostrEvent> for ArkheEvent {
    fn from(e: NostrEvent) -> Self {
        let signer = QuantumSigner::new();
        Self::seal(e, None, &signer)
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
        let canonical = canonical_json("pkx", 99, 7, &[], "hello");
        let id = hex(&Sha256::digest(canonical.as_bytes())[..]);
        let e = NostrEvent::new("pkx", 99, 7, vec![], "hello");
        assert_eq!(e.id, id);
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

    #[test]
    fn pqc_pubkey_is_stable_and_kind_specific() {
        let kem_a = arkhe_pqc::kem::generate_kem_keypair();
        let kem_b = arkhe_pqc::kem::generate_kem_keypair();
        let signer = QuantumSigner::new();
        let a = PqcIdentity { dsa_public: signer.public(), kem_public: kem_a.encapsulation.clone() };
        let b = PqcIdentity { dsa_public: signer.public(), kem_public: kem_b.encapsulation.clone() };

        let pa = pqc_to_pubkey(&a);
        let pb = pqc_to_pubkey(&b);
        assert_eq!(pa.len(), 64, "32 bytes in hex");
        assert_eq!(pa, pqc_to_pubkey(&a), "deterministic");
        assert_ne!(pa, pb, "different KEM pk changes synthetic pubkey");
        assert!(to_bech32_hrp("npub", &unhex(&pa)).starts_with("npub1"));
    }

    #[test]
    fn transport_mode_from_health() {
        assert_eq!(TransportMode::from_health(0.9), TransportMode::NostrPrimary);
        assert_eq!(TransportMode::from_health(0.5), TransportMode::Hybrid);
        assert_eq!(TransportMode::from_health(0.1), TransportMode::UdpFallback);
    }
}

