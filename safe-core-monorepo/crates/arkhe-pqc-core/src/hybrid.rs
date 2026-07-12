//! Hybrid Ed25519 + ML-DSA-65 signatures via `pqcrypto-dilithium`.
//!
//! **Known limitation of this crate (not present in `arkhe-crypto-pqc`):**
//! `pqcrypto_dilithium::dilithium3::SecretKey` does not implement `Zeroize`.
//! Confirmed directly: `#[derive(Zeroize, ZeroizeOnDrop)]` on a struct
//! containing it fails to compile —
//! `pqcrypto_dilithium::dilithium3::SecretKey: Zeroize` is not satisfied.
//! Its own trait, `pqcrypto_traits::sign::SecretKey`, only exposes
//! `as_bytes(&self) -> &[u8]` (confirmed: no mutable or owned accessor
//! exists on that trait at all) — so there is **no way to zero this type's
//! memory through its public API**. An earlier draft of this file both
//! derived `ZeroizeOnDrop` *and* hand-wrote `impl Drop { self.zeroize() }`
//! on the same struct, which doesn't compile either way (conflicting `Drop`
//! impls — confirmed via `error[E0119]`) — and even if it did compile, the
//! derive itself doesn't work on this field, so it would've been exactly
//! the "looks handled but isn't" gap already found and removed once in
//! `arkhe-crypto-pqc`'s history. No `Drop` impl is written here instead of
//! a fake one. This is a genuine advantage of the pure-Rust `fips204` crate
//! (used in `arkhe-crypto-pqc`), whose `PrivateKey` derives
//! `Zeroize, ZeroizeOnDrop` natively — worth weighing if this crate's
//! zeroization gap matters for your use case.
//!
//! Also fixed here: the earlier draft defined `MLDSA65_SIG_SIZE = 4595`
//! next to `pqcrypto_dilithium::dilithium5` — `dilithium5` is ML-DSA-87
//! (NIST level 5), not ML-DSA-65 (level 3, = `dilithium3`), and 4595 isn't
//! even dilithium5's real size (it's 4627, confirmed by compiling
//! `dilithium5::signature_bytes()`). This file uses `dilithium3`
//! throughout (confirmed 3309 bytes — matching
//! `arkhe_crypto_pqc::signature`'s `fips204::ml_dsa_65::SIG_LEN` exactly)
//! and stores signatures as the real `DetachedSignature` type rather than a
//! hand-maintained size constant, so this class of bug can't recur.

use ed25519_dalek::{Signature as Ed25519Signature, Signer as _, SigningKey, Verifier as _, VerifyingKey};
use pqcrypto_dilithium::dilithium3::{
    detached_sign, keypair, verify_detached_signature, DetachedSignature, PublicKey as MlDsaPublicKey,
    SecretKey as MlDsaSecretKey,
};
use pqcrypto_traits::sign::PublicKey as _;
use rand::rngs::OsRng;

#[derive(Debug, thiserror::Error)]
pub enum PqcSignatureError {
    #[error("Ed25519 signature is invalid")]
    Ed25519Invalid,
    #[error("ML-DSA-65 signature is invalid")]
    MlDsaInvalid,
}

/// A hybrid signature: fixed-size Ed25519 signature (64 bytes) + a detached
/// ML-DSA-65 (`dilithium3`) signature, stored as the real
/// `DetachedSignature` type (which knows its own length) rather than a
/// hardcoded byte-size constant.
#[derive(Clone)]
pub struct HybridSignature {
    pub ed_sig: [u8; 64],
    pub ml_sig: DetachedSignature,
}

/// Hybrid keypair: Ed25519 + ML-DSA-65 (`dilithium3`), both halves. See the
/// module doc comment for why `ml_dsa_sk` cannot be zeroized on drop.
pub struct HybridKeyPair {
    ed25519_sk: SigningKey,
    pub ed25519_pk: VerifyingKey,
    ml_dsa_sk: MlDsaSecretKey,
    pub ml_dsa_pk: MlDsaPublicKey,
}

impl HybridKeyPair {
    pub fn generate() -> Self {
        let mut rng = OsRng;
        let ed25519_sk = SigningKey::generate(&mut rng);
        let ed25519_pk = ed25519_sk.verifying_key();
        let (ml_dsa_pk, ml_dsa_sk) = keypair();
        Self { ed25519_sk, ed25519_pk, ml_dsa_sk, ml_dsa_pk }
    }

    /// Signs `msg` under both schemes.
    pub fn sign(&self, msg: &[u8]) -> HybridSignature {
        let ed_sig: Ed25519Signature = self.ed25519_sk.sign(msg);
        let ml_sig = detached_sign(msg, &self.ml_dsa_sk);
        HybridSignature { ed_sig: ed_sig.to_bytes(), ml_sig }
    }

    /// Verifies `sig` over `msg`. **Both** sub-signatures must be valid.
    pub fn verify(&self, msg: &[u8], sig: &HybridSignature) -> Result<(), PqcSignatureError> {
        let ed_sig = Ed25519Signature::from_bytes(&sig.ed_sig);
        self.ed25519_pk.verify(msg, &ed_sig).map_err(|_| PqcSignatureError::Ed25519Invalid)?;
        verify_detached_signature(&sig.ml_sig, msg, &self.ml_dsa_pk)
            .map_err(|_| PqcSignatureError::MlDsaInvalid)?;
        Ok(())
    }

    pub fn public_key_bytes(&self) -> Vec<u8> {
        let mut bytes = self.ed25519_pk.to_bytes().to_vec();
        bytes.extend_from_slice(self.ml_dsa_pk.as_bytes());
        bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pqcrypto_traits::sign::DetachedSignature as _;

    #[test]
    fn sign_and_verify_roundtrip() {
        let kp = HybridKeyPair::generate();
        let msg = b"hybrid pqc test message";
        let sig = kp.sign(msg);
        assert!(kp.verify(msg, &sig).is_ok());
    }

    #[test]
    fn tampered_message_fails_verify() {
        let kp = HybridKeyPair::generate();
        let sig = kp.sign(b"original message");
        assert!(kp.verify(b"tampered message", &sig).is_err());
    }

    #[test]
    fn tampered_ed25519_half_fails_verify() {
        let kp = HybridKeyPair::generate();
        let msg = b"hybrid pqc test message";
        let mut sig = kp.sign(msg);
        sig.ed_sig[0] ^= 0xFF;
        assert!(kp.verify(msg, &sig).is_err());
    }

    #[test]
    fn ml_dsa_signature_is_dilithium3_length() {
        // Locks in the fix: dilithium3 (ML-DSA-65) is 3309 bytes, not
        // dilithium5's 4627 and not the previous draft's wrong 4595.
        let kp = HybridKeyPair::generate();
        let sig = kp.sign(b"length check");
        assert_eq!(sig.ml_sig.as_bytes().len(), pqcrypto_dilithium::dilithium3::signature_bytes());
        assert_eq!(sig.ml_sig.as_bytes().len(), 3309);
    }
}
