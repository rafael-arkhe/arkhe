//! FI-W02 — ML-KEM-1024 (FIPS 203) key encapsulation, with HKDF-SHA256
//! derivation of a usable symmetric key from the raw shared secret.
//!
//! The original (fabricated) implementation report stopped at the raw KEM
//! shared secret and never derived a symmetric key from it — this module
//! closes that gap explicitly.

use fips203::ml_kem_1024;
use fips203::traits::{Decaps, Encaps, KeyGen, SerDes};
use hkdf::Hkdf;
use sha2::Sha256;

pub const SYMMETRIC_KEY_LEN: usize = 32;
pub type SymmetricKey = [u8; SYMMETRIC_KEY_LEN];

#[derive(Debug, thiserror::Error)]
pub enum KemError {
    #[error("ML-KEM-1024 key generation failed")]
    Keygen,
    #[error("ML-KEM-1024 encapsulation failed")]
    Encapsulate,
    #[error("ML-KEM-1024 decapsulation failed")]
    Decapsulate,
    #[error("malformed ciphertext or key bytes")]
    Malformed,
}

/// ML-KEM-1024 keypair. `fips203::ml_kem_1024::DecapsKey` (the secret half)
/// already derives `Zeroize, ZeroizeOnDrop` on its own struct (see
/// `fips203/src/types.rs`), so no wrapper-level `Drop` is added here — see
/// the equivalent note on [`crate::signature::HybridSigningKey`] for why a
/// manual `Drop` zeroizing a cloned copy would be a no-op, not a real fix.
pub struct KemKeypair {
    encaps_key: ml_kem_1024::EncapsKey,
    decaps_key: ml_kem_1024::DecapsKey,
}

impl KemKeypair {
    pub fn encaps_key(&self) -> &ml_kem_1024::EncapsKey {
        &self.encaps_key
    }

    pub fn encaps_key_bytes(&self) -> [u8; ml_kem_1024::EK_LEN] {
        self.encaps_key.clone().into_bytes()
    }
}

/// Generates a fresh ML-KEM-1024 keypair.
pub fn generate_kem_keypair() -> Result<KemKeypair, KemError> {
    let (encaps_key, decaps_key) = ml_kem_1024::KG::try_keygen().map_err(|_| KemError::Keygen)?;
    Ok(KemKeypair { encaps_key, decaps_key })
}

/// Reconstructs a recipient's ML-KEM-1024 public (encapsulation) key from
/// bytes received over the wire — the counterpart to
/// [`KemKeypair::encaps_key_bytes`]. Needed by any sender who only has the
/// recipient's serialized public key (not their live `KemKeypair`, which
/// they never should — it holds the recipient's secret decapsulation key).
pub fn encaps_key_from_bytes(bytes: [u8; ml_kem_1024::EK_LEN]) -> Result<ml_kem_1024::EncapsKey, KemError> {
    ml_kem_1024::EncapsKey::try_from_bytes(bytes).map_err(|_| KemError::Malformed)
}

/// Encapsulates against `encaps_key` (the recipient's public key), returning
/// a derived symmetric key and the ciphertext to send to the recipient.
pub fn kem_encapsulate(
    encaps_key: &ml_kem_1024::EncapsKey,
) -> Result<(SymmetricKey, [u8; ml_kem_1024::CT_LEN]), KemError> {
    let (shared_secret, ciphertext) = encaps_key.try_encaps().map_err(|_| KemError::Encapsulate)?;
    let symmetric_key = derive_symmetric_key(&shared_secret.into_bytes());
    Ok((symmetric_key, ciphertext.into_bytes()))
}

/// Decapsulates `ciphertext` using the recipient's own keypair, returning
/// the same derived symmetric key [`kem_encapsulate`] produced.
pub fn kem_decapsulate(
    keypair: &KemKeypair,
    ciphertext: &[u8; ml_kem_1024::CT_LEN],
) -> Result<SymmetricKey, KemError> {
    let ct = ml_kem_1024::CipherText::try_from_bytes(*ciphertext).map_err(|_| KemError::Malformed)?;
    let shared_secret = keypair
        .decaps_key
        .try_decaps(&ct)
        .map_err(|_| KemError::Decapsulate)?;
    Ok(derive_symmetric_key(&shared_secret.into_bytes()))
}

/// HKDF-SHA256 over the raw ML-KEM shared secret — the shared secret itself
/// must not be used directly as a symmetric key without this step.
fn derive_symmetric_key(shared_secret: &[u8]) -> SymmetricKey {
    let hk = Hkdf::<Sha256>::new(None, shared_secret);
    let mut okm = [0u8; SYMMETRIC_KEY_LEN];
    hk.expand(b"arkhe-crypto-pqc/ml-kem-1024/v1", &mut okm)
        .expect("32 bytes is a valid HKDF-SHA256 output length");
    okm
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encaps_decaps_shared_secret_matches() {
        let keypair = generate_kem_keypair().unwrap();
        let (sender_key, ciphertext) = kem_encapsulate(keypair.encaps_key()).unwrap();
        let receiver_key = kem_decapsulate(&keypair, &ciphertext).unwrap();
        assert_eq!(sender_key, receiver_key);
    }

    #[test]
    fn tampered_ciphertext_yields_different_key() {
        let keypair = generate_kem_keypair().unwrap();
        let (sender_key, mut ciphertext) = kem_encapsulate(keypair.encaps_key()).unwrap();
        ciphertext[0] ^= 0xFF;
        // ML-KEM decapsulation is implicit-rejection: it never errors on a
        // tampered ciphertext, it just derives a different (useless) key.
        let receiver_key = kem_decapsulate(&keypair, &ciphertext).unwrap();
        assert_ne!(sender_key, receiver_key);
    }

    #[test]
    fn encaps_key_survives_a_bytes_roundtrip() {
        let keypair = generate_kem_keypair().unwrap();
        let bytes = keypair.encaps_key_bytes();
        let reconstructed = encaps_key_from_bytes(bytes).unwrap();
        let (sender_key, ciphertext) = kem_encapsulate(&reconstructed).unwrap();
        let receiver_key = kem_decapsulate(&keypair, &ciphertext).unwrap();
        assert_eq!(sender_key, receiver_key);
    }

    #[test]
    fn different_keypairs_derive_different_keys() {
        let keypair_a = generate_kem_keypair().unwrap();
        let keypair_b = generate_kem_keypair().unwrap();
        let (key_a, _) = kem_encapsulate(keypair_a.encaps_key()).unwrap();
        let (key_b, _) = kem_encapsulate(keypair_b.encaps_key()).unwrap();
        assert_ne!(key_a, key_b);
    }
}
