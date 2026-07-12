//! FI-004 — context-bound signatures: `sign(msg, context)` where `context =
//! hash(domain, timestamp, nonce, policy_version)`. Replaces the
//! EIP-712-style domain-separation pattern already used elsewhere in this
//! workspace (`arkhe-web3-security::wallets::eip712`, which is scoped to
//! Ethereum-style structured data) with a general-purpose version usable
//! anywhere a signature needs to be bound to a specific domain, moment,
//! nonce, and policy version — not just Ethereum contract calls.
//!
//! A signature over `(context_digest || msg)` cannot be replayed under a
//! different domain, timestamp, nonce, or policy version, since any of
//! those changes the digest and therefore the signed payload.

use crate::signature::{HybridKeyPair, HybridSignature, PqcSignatureError};

/// The four fields the catalog's formula binds a signature to. Any change
/// to any field changes [`SigningContext::digest`], and therefore
/// invalidates a signature produced under the old context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SigningContext {
    pub domain: String,
    pub timestamp: u64,
    pub nonce: u64,
    pub policy_version: String,
}

impl SigningContext {
    /// `hash(domain, timestamp, nonce, policy_version)` — BLAKE3 over a
    /// length-prefixed, unambiguous encoding of the four fields (plain
    /// concatenation would let `("ab", "c")` collide with `("a", "bc")`).
    pub fn digest(&self) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        hasher.update(&(self.domain.len() as u64).to_be_bytes());
        hasher.update(self.domain.as_bytes());
        hasher.update(&self.timestamp.to_be_bytes());
        hasher.update(&self.nonce.to_be_bytes());
        hasher.update(&(self.policy_version.len() as u64).to_be_bytes());
        hasher.update(self.policy_version.as_bytes());
        *hasher.finalize().as_bytes()
    }
}

/// Signs `msg` bound to `context`: the actual hybrid signature is produced
/// over `context.digest() || msg`, not `msg` alone.
pub fn sign_with_context(
    keypair: &HybridKeyPair,
    msg: &[u8],
    context: &SigningContext,
) -> Result<HybridSignature, PqcSignatureError> {
    let mut payload = Vec::with_capacity(32 + msg.len());
    payload.extend_from_slice(&context.digest());
    payload.extend_from_slice(msg);
    keypair.signing_key.sign(&payload)
}

/// Verifies `sig` over `msg` under `context`. Fails if `sig` was produced
/// under a different context (different domain/timestamp/nonce/policy
/// version) even if it validly signs the exact same `msg` bytes under that
/// other context — that's the entire point of context-binding.
pub fn verify_with_context(
    keypair: &HybridKeyPair,
    msg: &[u8],
    context: &SigningContext,
    sig: &HybridSignature,
) -> Result<(), PqcSignatureError> {
    let mut payload = Vec::with_capacity(32 + msg.len());
    payload.extend_from_slice(&context.digest());
    payload.extend_from_slice(msg);
    keypair.verifying_key.verify(&payload, sig)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signature::generate_hybrid_keypair;

    fn ctx(nonce: u64) -> SigningContext {
        SigningContext { domain: "arkhe:agent-vm".to_string(), timestamp: 1_000, nonce, policy_version: "v1".to_string() }
    }

    #[test]
    fn sign_and_verify_roundtrip_under_same_context() {
        let kp = generate_hybrid_keypair().unwrap();
        let msg = b"transfer capability";
        let sig = sign_with_context(&kp, msg, &ctx(1)).unwrap();
        assert!(verify_with_context(&kp, msg, &ctx(1), &sig).is_ok());
    }

    #[test]
    fn signature_is_rejected_under_a_different_nonce() {
        let kp = generate_hybrid_keypair().unwrap();
        let msg = b"transfer capability";
        let sig = sign_with_context(&kp, msg, &ctx(1)).unwrap();
        // Same message, same everything except nonce — a replay attempt.
        assert!(verify_with_context(&kp, msg, &ctx(2), &sig).is_err());
    }

    #[test]
    fn signature_is_rejected_under_a_different_domain() {
        let kp = generate_hybrid_keypair().unwrap();
        let msg = b"transfer capability";
        let sig = sign_with_context(&kp, msg, &ctx(1)).unwrap();
        let other_domain = SigningContext { domain: "arkhe:web3-security".to_string(), ..ctx(1) };
        assert!(verify_with_context(&kp, msg, &other_domain, &sig).is_err());
    }

    #[test]
    fn signature_is_rejected_under_a_different_policy_version() {
        let kp = generate_hybrid_keypair().unwrap();
        let msg = b"transfer capability";
        let sig = sign_with_context(&kp, msg, &ctx(1)).unwrap();
        let other_policy = SigningContext { policy_version: "v2".to_string(), ..ctx(1) };
        assert!(verify_with_context(&kp, msg, &other_policy, &sig).is_err());
    }

    #[test]
    fn digest_is_deterministic_for_the_same_fields() {
        assert_eq!(ctx(1).digest(), ctx(1).digest());
    }

    #[test]
    fn no_field_concatenation_ambiguity() {
        // "ab"+"c" vs "a"+"bc" must not collide, thanks to length prefixes.
        let a = SigningContext { domain: "ab".to_string(), timestamp: 0, nonce: 0, policy_version: "c".to_string() };
        let b = SigningContext { domain: "a".to_string(), timestamp: 0, nonce: 0, policy_version: "bc".to_string() };
        assert_ne!(a.digest(), b.digest());
    }
}
