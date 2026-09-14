//! AOTB encoder/verifier over the canonical [`AotbPayload`].
//!
//! Sync policy = **soft sync**: the verifier accepts any `sequence >= expected`
//! (optical frames get dropped by blur/occlusion, so gaps are normal) but
//! rejects `sequence < expected` (replay / time-travel). This is the one policy
//! that must hold identically in the TLM, the RTL verifier, and any on-chain
//! pallet.

use crate::payload::{AotbPayload, DOMAIN_NODES};
use ed25519_dalek::{Signer, SigningKey, Verifier, VerifyingKey};

pub const SENTINEL: u64 = u64::MAX;

#[derive(Debug, Clone, PartialEq)]
pub struct AotbFrame {
    pub payload: AotbPayload,
    pub signature: [u8; 64],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerifyError {
    BadSignature,
    Replay,
    Sentinel,
    SessionMismatch,
    NonceMismatch,
}

pub struct AotbEncoder {
    key: SigningKey,
    session_id: [u8; 16],
    next_sequence: u64,
    proof_hash: [u8; 32],
}

impl AotbEncoder {
    pub fn new(key: SigningKey, session_id: [u8; 16], proof_hash: [u8; 32]) -> Self {
        Self { key, session_id, next_sequence: 0, proof_hash }
    }

    pub fn next_frame(
        &mut self,
        domain_values: [f64; DOMAIN_NODES],
        weights: [u8; DOMAIN_NODES],
    ) -> AotbFrame {
        let sequence = self.next_sequence;
        self.next_sequence += 1;
        let payload = AotbPayload {
            session_id: self.session_id,
            sequence,
            nonce: sequence,
            proof_hash: self.proof_hash,
            domain_values,
            weights,
        };
        let signature = self.key.sign(&payload.to_canonical_bytes()).to_bytes();
        AotbFrame { payload, signature }
    }
}

pub struct AotbVerifier {
    key: VerifyingKey,
    session_id: [u8; 16],
    next_sequence: u64,
}

impl AotbVerifier {
    pub fn new(key: VerifyingKey, session_id: [u8; 16]) -> Self {
        Self { key, session_id, next_sequence: 0 }
    }

    /// Soft-sync verification. On success advances the window past the accepted
    /// sequence (so a later replay of the same or earlier frame is rejected).
    pub fn verify(&mut self, frame: &AotbFrame) -> Result<(), VerifyError> {
        let p = &frame.payload;
        if p.nonce == SENTINEL {
            return Err(VerifyError::Sentinel);
        }
        if p.session_id != self.session_id {
            return Err(VerifyError::SessionMismatch);
        }
        if p.nonce != p.sequence {
            return Err(VerifyError::NonceMismatch);
        }
        if p.sequence < self.next_sequence {
            return Err(VerifyError::Replay); // soft sync: gaps ok, backwards not
        }
        let sig = ed25519_dalek::Signature::from_slice(&frame.signature)
            .map_err(|_| VerifyError::BadSignature)?;
        self.key
            .verify(&p.to_canonical_bytes(), &sig)
            .map_err(|_| VerifyError::BadSignature)?;
        self.next_sequence = p.sequence + 1;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kp() -> (SigningKey, VerifyingKey) {
        let sk = SigningKey::from_bytes(&[7u8; 32]);
        let vk = sk.verifying_key();
        (sk, vk)
    }

    #[test]
    fn sign_and_verify_roundtrip() {
        let (sk, vk) = kp();
        let mut enc = AotbEncoder::new(sk, [4; 16], [5; 32]);
        let mut ver = AotbVerifier::new(vk, [4; 16]);
        let f = enc.next_frame([1.0; DOMAIN_NODES], [100; DOMAIN_NODES]);
        assert_eq!(ver.verify(&f), Ok(()));
    }

    #[test]
    fn replay_is_rejected() {
        let (sk, vk) = kp();
        let mut enc = AotbEncoder::new(sk, [4; 16], [5; 32]);
        let mut ver = AotbVerifier::new(vk, [4; 16]);
        let f = enc.next_frame([0.0; DOMAIN_NODES], [100; DOMAIN_NODES]);
        assert_eq!(ver.verify(&f), Ok(()));
        assert_eq!(ver.verify(&f), Err(VerifyError::Replay));
    }

    #[test]
    fn dropped_frames_are_accepted_soft_sync() {
        let (sk, vk) = kp();
        let mut enc = AotbEncoder::new(sk, [4; 16], [5; 32]);
        let mut ver = AotbVerifier::new(vk, [4; 16]);
        let f0 = enc.next_frame([0.0; DOMAIN_NODES], [100; DOMAIN_NODES]);
        let _f1 = enc.next_frame([0.0; DOMAIN_NODES], [100; DOMAIN_NODES]); // dropped
        let f2 = enc.next_frame([0.0; DOMAIN_NODES], [100; DOMAIN_NODES]);
        assert_eq!(ver.verify(&f0), Ok(()));
        assert_eq!(ver.verify(&f2), Ok(())); // gap accepted
        assert_eq!(ver.verify(&f2), Err(VerifyError::Replay)); // but not twice
    }

    #[test]
    fn tampered_payload_fails_signature() {
        let (sk, vk) = kp();
        let mut enc = AotbEncoder::new(sk, [4; 16], [5; 32]);
        let mut ver = AotbVerifier::new(vk, [4; 16]);
        let mut f = enc.next_frame([1.0; DOMAIN_NODES], [100; DOMAIN_NODES]);
        f.payload.domain_values[0] = 2.0; // tamper after signing
        assert_eq!(ver.verify(&f), Err(VerifyError::BadSignature));
    }
}
