//! FI-W01 — Hybrid post-quantum + classical digital signatures.
//!
//! Combines Ed25519 (classical, fast, universally supported) with ML-DSA-65
//! (FIPS 204, lattice-based, quantum-resistant): a `HybridSignature` is only
//! accepted if *both* sub-signatures verify. This is the standard hybrid
//! transition posture recommended while classical schemes are phased out —
//! see NIST SP 800-208 and the CNSA 2.0 migration guidance.

use ed25519_dalek::{Signature as Ed25519Signature, Signer as _, SigningKey, Verifier as _, VerifyingKey};
use fips204::ml_dsa_65;
use fips204::traits::{SerDes, Signer as MlDsaSigner, Verifier as MlDsaVerifier};
use rand::rngs::OsRng;

/// Empty domain-separation context appended to every ML-DSA signature. Kept
/// as a named constant (rather than an inline `&[]`) so a future revision
/// can bind a real context string without touching call sites.
const ML_DSA_CONTEXT: &[u8] = &[];

#[derive(Debug, thiserror::Error)]
pub enum PqcSignatureError {
    #[error("ML-DSA-65 key generation failed")]
    MlDsaKeygen,
    #[error("ML-DSA-65 signing failed")]
    MlDsaSign,
    #[error("Ed25519 signature is invalid")]
    Ed25519Invalid,
    #[error("ML-DSA-65 signature is invalid")]
    MlDsaInvalid,
    #[error("malformed key or signature bytes")]
    Malformed,
}

/// Hybrid signing key: Ed25519 secret scalar + ML-DSA-65 private key.
///
/// No manual `Drop` needed here: `ed25519_dalek::SigningKey` already
/// implements `Drop` + `ZeroizeOnDrop` internally (see
/// `ed25519-dalek/src/signing.rs`), and `fips204::ml_dsa_65::PrivateKey`
/// derives `Zeroize, ZeroizeOnDrop` directly on its struct (see
/// `fips204/src/types.rs`). A wrapper-level `Drop` that zeroized a
/// `.clone()`/`.to_vec()` copy of the bytes would scrub a temporary, not the
/// real secret — that's a no-op that only looks like a security control, so
/// it's deliberately not added here; the zeroization guarantee is real and
/// already inherited from both fields.
pub struct HybridSigningKey {
    ed25519: SigningKey,
    ml_dsa: ml_dsa_65::PrivateKey,
}

/// Hybrid verifying key: Ed25519 public point + ML-DSA-65 public key.
#[derive(Clone)]
pub struct HybridVerifyingKey {
    ed25519: VerifyingKey,
    ml_dsa: ml_dsa_65::PublicKey,
}

pub struct HybridKeyPair {
    pub signing_key: HybridSigningKey,
    pub verifying_key: HybridVerifyingKey,
}

/// A hybrid signature: fixed-size Ed25519 signature (64 bytes) plus a
/// fixed-size ML-DSA-65 signature (`ml_dsa_65::SIG_LEN` bytes, sized by the
/// algorithm's own constant rather than an unsized `Vec<u8>`).
#[derive(Clone)]
pub struct HybridSignature {
    pub ed_sig: [u8; 64],
    pub ml_sig: [u8; ml_dsa_65::SIG_LEN],
}

/// Generates a fresh hybrid keypair using the OS CSPRNG for both schemes.
pub fn generate_hybrid_keypair() -> Result<HybridKeyPair, PqcSignatureError> {
    let mut rng = OsRng;
    let ed25519 = SigningKey::generate(&mut rng);
    let ed_verifying = ed25519.verifying_key();

    let (ml_public, ml_private) = ml_dsa_65::try_keygen().map_err(|_| PqcSignatureError::MlDsaKeygen)?;

    Ok(HybridKeyPair {
        signing_key: HybridSigningKey { ed25519, ml_dsa: ml_private },
        verifying_key: HybridVerifyingKey { ed25519: ed_verifying, ml_dsa: ml_public },
    })
}

impl HybridSigningKey {
    /// Signs `msg` under both schemes. The resulting `HybridSignature` is
    /// only meaningful together — an attacker who can forge one half still
    /// cannot produce a signature that passes [`HybridVerifyingKey::verify`].
    pub fn sign(&self, msg: &[u8]) -> Result<HybridSignature, PqcSignatureError> {
        let ed_sig: Ed25519Signature = self.ed25519.sign(msg);

        let ml_sig = self
            .ml_dsa
            .try_sign(msg, ML_DSA_CONTEXT)
            .map_err(|_| PqcSignatureError::MlDsaSign)?;

        Ok(HybridSignature {
            ed_sig: ed_sig.to_bytes(),
            ml_sig,
        })
    }

    pub fn verifying_key(&self) -> HybridVerifyingKey {
        HybridVerifyingKey {
            ed25519: self.ed25519.verifying_key(),
            ml_dsa: self.ml_dsa.get_public_key(),
        }
    }
}

impl HybridVerifyingKey {
    /// Verifies `sig` over `msg`. **Both** sub-signatures must be valid —
    /// this is the entire point of a hybrid scheme: a single broken
    /// primitive (classical or post-quantum) must not be enough to forge a
    /// signature.
    pub fn verify(&self, msg: &[u8], sig: &HybridSignature) -> Result<(), PqcSignatureError> {
        let ed_sig = Ed25519Signature::from_bytes(&sig.ed_sig);
        self.ed25519
            .verify(msg, &ed_sig)
            .map_err(|_| PqcSignatureError::Ed25519Invalid)?;

        let ml_valid = self.ml_dsa.verify(msg, &sig.ml_sig, ML_DSA_CONTEXT);
        if !ml_valid {
            return Err(PqcSignatureError::MlDsaInvalid);
        }

        Ok(())
    }

    pub fn ed25519_bytes(&self) -> [u8; 32] {
        self.ed25519.to_bytes()
    }

    pub fn ml_dsa_bytes(&self) -> [u8; ml_dsa_65::PK_LEN] {
        self.ml_dsa.clone().into_bytes()
    }

    /// Verifies only the ML-DSA-65 half of `sig`, ignoring the Ed25519 half.
    /// For callers operating under a PQC-only policy that don't require the
    /// classical signature to also be present/valid — see
    /// `arkhe-web3-security`'s `WalletPolicy::PqcOnly`.
    pub fn verify_ml_dsa_only(&self, msg: &[u8], sig: &HybridSignature) -> bool {
        self.ml_dsa.verify(msg, &sig.ml_sig, ML_DSA_CONTEXT)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_and_verify_roundtrip() {
        let kp = generate_hybrid_keypair().unwrap();
        let msg = b"hybrid pqc test message";
        let sig = kp.signing_key.sign(msg).unwrap();
        assert!(kp.verifying_key.verify(msg, &sig).is_ok());
    }

    #[test]
    fn tampered_message_fails_verify() {
        let kp = generate_hybrid_keypair().unwrap();
        let sig = kp.signing_key.sign(b"original message").unwrap();
        assert!(kp.verifying_key.verify(b"tampered message", &sig).is_err());
    }

    #[test]
    fn tampered_ed25519_half_fails_verify() {
        let kp = generate_hybrid_keypair().unwrap();
        let msg = b"hybrid pqc test message";
        let mut sig = kp.signing_key.sign(msg).unwrap();
        sig.ed_sig[0] ^= 0xFF;
        assert!(kp.verifying_key.verify(msg, &sig).is_err());
    }

    #[test]
    fn tampered_ml_dsa_half_fails_verify() {
        let kp = generate_hybrid_keypair().unwrap();
        let msg = b"hybrid pqc test message";
        let mut sig = kp.signing_key.sign(msg).unwrap();
        sig.ml_sig[0] ^= 0xFF;
        assert!(kp.verifying_key.verify(msg, &sig).is_err());
    }

    #[test]
    fn verify_ml_dsa_only_ignores_tampered_ed25519_half() {
        let kp = generate_hybrid_keypair().unwrap();
        let msg = b"hybrid pqc test message";
        let mut sig = kp.signing_key.sign(msg).unwrap();
        sig.ed_sig[0] ^= 0xFF;
        // Full hybrid verify rejects (ed25519 half is broken)...
        assert!(kp.verifying_key.verify(msg, &sig).is_err());
        // ...but the ML-DSA-only check still accepts, since only the
        // Ed25519 half was tampered with.
        assert!(kp.verifying_key.verify_ml_dsa_only(msg, &sig));
    }

    #[test]
    fn verify_ml_dsa_only_rejects_tampered_ml_dsa_half() {
        let kp = generate_hybrid_keypair().unwrap();
        let msg = b"hybrid pqc test message";
        let mut sig = kp.signing_key.sign(msg).unwrap();
        sig.ml_sig[0] ^= 0xFF;
        assert!(!kp.verifying_key.verify_ml_dsa_only(msg, &sig));
    }

    #[test]
    fn signature_derived_from_signing_key_matches_verifying_key() {
        let kp = generate_hybrid_keypair().unwrap();
        let derived = kp.signing_key.verifying_key();
        assert_eq!(derived.ed25519_bytes(), kp.verifying_key.ed25519_bytes());
        assert_eq!(derived.ml_dsa_bytes(), kp.verifying_key.ml_dsa_bytes());
    }
}
