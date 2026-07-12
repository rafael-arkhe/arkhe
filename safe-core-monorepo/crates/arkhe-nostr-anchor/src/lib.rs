//! FI-032 — Nostr identity roots: a real NIP-01 canonical event-id
//! computation and real BIP-340 Schnorr signing (via `k256`), anchoring an
//! `arkhe_identity::Gdid` to a Nostr-format identity-root event.
//!
//! # Why a separate keypair from the rest of this workspace
//! Nostr's wire format requires x-only secp256k1 keys specifically
//! (BIP-340). This workspace's existing identity keys
//! (`arkhe-crypto-pqc::HybridSigningKey`: Ed25519 + ML-DSA-65) cannot be
//! reinterpreted as secp256k1 keys — there is no valid conversion between
//! unrelated curves/schemes. A `NostrIdentity` is therefore a distinct
//! keypair, anchored *to* a `Gdid` (via the event's `content` field), not
//! derived *from* the workspace's hybrid PQC identity.
//!
//! # What this is not
//! There is no relay client here — no WebSocket connection, no `connect`/
//! `publish` methods. This crate produces and verifies real, spec-shaped
//! NIP-01 events; sending them to a relay is a distinct transport concern,
//! deliberately out of scope (matching `arkhe-network`'s own precedent of
//! not doing live sockets — see that crate's README).
//!
//! An earlier, unrelated prototype elsewhere in this repository tree
//! (`arkhe-os/libs/nostr`) computed event IDs via SHA3-256 (not the spec's
//! SHA-256) and hardcoded `sig: "0".repeat(128)` — never a real signature.
//! This crate is a from-scratch, spec-correct implementation, not a port
//! of that code.

#![forbid(unsafe_code)]

use arkhe_identity::Gdid;
use k256::schnorr::signature::{Signer, Verifier};
use k256::schnorr::{Signature, SigningKey, VerifyingKey};
use serde::Serialize;
use sha2::{Digest, Sha256};

/// Custom Nostr event kind for an Arkhe identity-root anchor. NIP-01
/// reserves kind numbering 30000-39999 for "parameterized replaceable
/// events" (an app-defined `d` tag identifies the replaceable slot) —
/// this follows that convention without claiming a formally registered
/// NIP number.
pub const KIND_ARKHE_IDENTITY_ROOT: u16 = 30000;

/// Errors from building or verifying a [`NostrEvent`].
#[derive(Debug, thiserror::Error)]
pub enum NostrAnchorError {
    /// `pubkey` field is not valid hex, or not 32 bytes once decoded.
    #[error("invalid public key: {0}")]
    InvalidPublicKey(String),
    /// `sig` bytes do not form a well-formed BIP-340 signature.
    #[error("invalid signature encoding")]
    InvalidSignature,
    /// The event's own `id` does not match the hash of its other fields —
    /// the event has been tampered with, or was constructed incorrectly.
    #[error("event id does not match its own fields (tampered?)")]
    IdMismatch,
    /// `sig` does not verify against `id` under `pubkey`.
    #[error("signature does not verify")]
    SignatureInvalid,
}

/// A real secp256k1/BIP-340 Nostr identity keypair.
pub struct NostrIdentity {
    signing_key: SigningKey,
}

impl NostrIdentity {
    /// Generates a fresh Nostr identity keypair.
    pub fn generate() -> Self {
        Self { signing_key: SigningKey::random(&mut rand::thread_rng()) }
    }

    /// The 32-byte x-only public key, hex-encoded — the `pubkey` field of
    /// every NIP-01 event this identity signs.
    pub fn pubkey_hex(&self) -> String {
        hex::encode(self.signing_key.verifying_key().to_bytes())
    }

    /// Builds and signs a real NIP-01 event of any `kind` — the general
    /// case [`root_identity`] is a thin wrapper around, for callers that
    /// need Nostr events this crate doesn't have a dedicated constructor
    /// for (e.g. Blossom/BUD-01 authorization events, kind `24242`).
    pub fn sign_event(&self, created_at: u64, kind: u16, tags: Vec<Vec<String>>, content: String) -> NostrEvent {
        let pubkey = self.pubkey_hex();
        let id = compute_event_id(&pubkey, created_at, kind, &tags, &content);
        let signature: Signature = self.signing_key.sign(&id);
        NostrEvent { id, pubkey, created_at, kind, tags, content, sig: signature.to_bytes() }
    }
}

/// A Nostr NIP-01 event. `id` is
/// `sha256(canonical_json([0, pubkey, created_at, kind, tags, content]))`
/// per the spec; `sig` is a genuine BIP-340 Schnorr signature over `id`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NostrEvent {
    /// 32-byte event id (SHA-256 of the canonical serialization).
    pub id: [u8; 32],
    /// Hex-encoded 32-byte x-only public key of the signer.
    pub pubkey: String,
    /// Unix timestamp, seconds.
    pub created_at: u64,
    /// Event kind. See [`KIND_ARKHE_IDENTITY_ROOT`].
    pub kind: u16,
    /// NIP-01 tags — `[["d", "arkhe-gdid-root"]]` for events produced by
    /// [`root_identity`].
    pub tags: Vec<Vec<String>>,
    /// Event content. For an identity-root event, the hex-encoded `Gdid`.
    pub content: String,
    /// 64-byte BIP-340 Schnorr signature over `id`.
    pub sig: [u8; 64],
}

#[derive(Serialize)]
struct EventForId<'a>(u8, &'a str, u64, u16, &'a Vec<Vec<String>>, &'a str);

/// NIP-01 canonical event-id computation: `sha256` of the compact
/// (whitespace-free) JSON array `[0, pubkey, created_at, kind, tags,
/// content]`. `serde_json`'s default (non-pretty) serializer produces no
/// inter-token whitespace and does not escape non-ASCII bytes, matching
/// the spec's serialization rules.
pub fn compute_event_id(pubkey: &str, created_at: u64, kind: u16, tags: &Vec<Vec<String>>, content: &str) -> [u8; 32] {
    let tuple = EventForId(0, pubkey, created_at, kind, tags, content);
    let json = serde_json::to_vec(&tuple).expect("EventForId is a plain data tuple; serialization cannot fail");
    let mut h = Sha256::new();
    h.update(&json);
    h.finalize().into()
}

/// Builds and signs a Nostr identity-root event anchoring `gdid` under
/// `identity`. `content` is the hex-encoded `Gdid` bytes — a real,
/// data-model commitment, not a description of one.
pub fn root_identity(identity: &NostrIdentity, gdid: &Gdid, created_at: u64) -> NostrEvent {
    let content = hex::encode(gdid.as_bytes());
    let tags = vec![vec!["d".to_string(), "arkhe-gdid-root".to_string()]];
    identity.sign_event(created_at, KIND_ARKHE_IDENTITY_ROOT, tags, content)
}

/// Verifies a [`NostrEvent`]: recomputes `id` from its other fields (catches
/// tampering with any field, `id` included) and checks `sig` against `id`
/// under `pubkey`. Despite the name, this check is fully generic — nothing
/// here assumes an identity-root event's shape specifically — so
/// [`verify_event`] is the same function under a name that reads better at
/// call sites verifying some other kind of event (e.g. a Blossom/BUD-01
/// authorization).
pub fn verify_root_event(event: &NostrEvent) -> Result<(), NostrAnchorError> {
    let recomputed = compute_event_id(&event.pubkey, event.created_at, event.kind, &event.tags, &event.content);
    if recomputed != event.id {
        return Err(NostrAnchorError::IdMismatch);
    }

    let pubkey_bytes = hex::decode(&event.pubkey).map_err(|e| NostrAnchorError::InvalidPublicKey(e.to_string()))?;
    let verifying_key = VerifyingKey::from_bytes(&pubkey_bytes)
        .map_err(|_| NostrAnchorError::InvalidPublicKey("not a valid x-only secp256k1 point".to_string()))?;
    let signature = Signature::try_from(event.sig.as_slice()).map_err(|_| NostrAnchorError::InvalidSignature)?;

    verifying_key.verify(&event.id, &signature).map_err(|_| NostrAnchorError::SignatureInvalid)
}

/// Alias for [`verify_root_event`] — see its doc comment for why this is
/// the same, fully generic function under a more general name.
pub fn verify_event(event: &NostrEvent) -> Result<(), NostrAnchorError> {
    verify_root_event(event)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_identity_produces_a_self_verifying_event() {
        let identity = NostrIdentity::generate();
        let gdid = Gdid::from_raw([42u8; 32]);
        let event = root_identity(&identity, &gdid, 1_720_000_000);
        assert_eq!(event.pubkey, identity.pubkey_hex());
        assert_eq!(event.content, hex::encode(gdid.as_bytes()));
        assert!(verify_root_event(&event).is_ok());
    }

    #[test]
    fn event_id_is_deterministic_for_the_same_fields() {
        let a = compute_event_id("pk", 1, 30000, &vec![vec!["d".to_string(), "x".to_string()]], "content");
        let b = compute_event_id("pk", 1, 30000, &vec![vec!["d".to_string(), "x".to_string()]], "content");
        assert_eq!(a, b);
    }

    #[test]
    fn event_id_changes_with_any_field() {
        let base = compute_event_id("pk", 1, 30000, &vec![], "content");
        assert_ne!(base, compute_event_id("pk2", 1, 30000, &vec![], "content"));
        assert_ne!(base, compute_event_id("pk", 2, 30000, &vec![], "content"));
        assert_ne!(base, compute_event_id("pk", 1, 30001, &vec![], "content"));
        assert_ne!(base, compute_event_id("pk", 1, 30000, &vec![], "different"));
    }

    #[test]
    fn tampered_content_is_rejected_as_id_mismatch() {
        let identity = NostrIdentity::generate();
        let gdid = Gdid::from_raw([1u8; 32]);
        let mut event = root_identity(&identity, &gdid, 1_720_000_000);
        event.content = "tampered".to_string();
        assert!(matches!(verify_root_event(&event), Err(NostrAnchorError::IdMismatch)));
    }

    #[test]
    fn tampered_signature_is_rejected() {
        let identity = NostrIdentity::generate();
        let gdid = Gdid::from_raw([2u8; 32]);
        let mut event = root_identity(&identity, &gdid, 1_720_000_000);
        event.sig[0] ^= 0xFF;
        assert!(matches!(verify_root_event(&event), Err(NostrAnchorError::SignatureInvalid)));
    }

    #[test]
    fn event_signed_by_one_identity_does_not_verify_under_a_different_pubkey_claim() {
        let identity_a = NostrIdentity::generate();
        let identity_b = NostrIdentity::generate();
        let gdid = Gdid::from_raw([3u8; 32]);
        let mut event = root_identity(&identity_a, &gdid, 1_720_000_000);
        // Claim identity_b's pubkey while keeping identity_a's signature —
        // must be rejected (either id mismatch, since pubkey feeds the
        // hash, or signature invalid; either is a correct rejection).
        event.pubkey = identity_b.pubkey_hex();
        assert!(verify_root_event(&event).is_err());
    }

    #[test]
    fn different_gdids_produce_different_content() {
        let identity = NostrIdentity::generate();
        let a = root_identity(&identity, &Gdid::from_raw([1u8; 32]), 1);
        let b = root_identity(&identity, &Gdid::from_raw([2u8; 32]), 1);
        assert_ne!(a.content, b.content);
        assert_ne!(a.id, b.id);
    }
}
