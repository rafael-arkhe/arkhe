//! ML-KEM-1024 encapsulation of a CHK convergent key (FI-032), for sharing
//! a specific stored object with a specific recipient agent.
//!
//! The `ChkKey` in [`crate::chk`] is convergent by design:
//! `hash(plaintext)`. Anyone who already has the plaintext can trivially
//! derive it — that's the intended deduplication property, not a flaw. But
//! handing that key to a recipient *without* the plaintext (so they can
//! decrypt a chunk they don't already hold) needs real confidentiality in
//! transit. This module wraps the 32-byte `ChkKey` in a ChaCha20-Poly1305
//! AEAD ciphertext, keyed by a symmetric key HKDF-derived (inside
//! `arkhe-crypto-pqc`) from an ML-KEM-1024 (FIPS 203) shared secret
//! encapsulated against the recipient's public key. Only the holder of the
//! matching decapsulation key can recover the `ChkKey`.
//!
//! This directly closes the gap [`crate::chk`]'s own doc comment
//! discloses ("not AEAD") — but only for this specific point-to-point
//! key-sharing path, not for local at-rest storage using `chk.rs` alone.

use arkhe_crypto_pqc::kem::{encaps_key_from_bytes, kem_decapsulate, kem_encapsulate, KemError, KemKeypair};
use chacha20poly1305::aead::Aead;
use chacha20poly1305::{ChaCha20Poly1305, Key, KeyInit, Nonce};
use fips203::ml_kem_1024;
use sha2::{Digest, Sha256};

use crate::chk::ChkKey;

/// AEAD nonce length (ChaCha20-Poly1305).
pub const NONCE_LEN: usize = 12;

/// Errors from encapsulating or recovering a `ChkKey`.
#[derive(Debug, thiserror::Error)]
pub enum EncapsulationError {
    /// The underlying ML-KEM-1024 operation failed (bad key bytes, etc.).
    #[error("ML-KEM-1024 error: {0}")]
    Kem(#[from] KemError),
    /// AEAD encryption of the `ChkKey` failed.
    #[error("AEAD encryption of the CHK key failed")]
    Encrypt,
    /// AEAD decryption failed — wrong recipient key, or a tampered
    /// ciphertext. ML-KEM's implicit-rejection semantics mean a wrong
    /// decapsulation key does not error on its own; it silently derives a
    /// different (useless) symmetric key, so the failure only surfaces
    /// here, at AEAD tag verification.
    #[error("AEAD decryption failed — wrong recipient key or tampered ciphertext")]
    Decrypt,
}

/// A `ChkKey`, encrypted for one specific recipient. Safe to send over an
/// untrusted channel: only the holder of the matching `KemKeypair`'s
/// decapsulation key can recover the `ChkKey` from this.
#[derive(Debug, Clone)]
pub struct EncapsulatedChkKey {
    /// ML-KEM-1024 ciphertext — the recipient decapsulates this to derive
    /// the same symmetric key the sender used.
    pub kem_ciphertext: [u8; ml_kem_1024::CT_LEN],
    /// AEAD nonce. Derived (not random) — see `derive_nonce`.
    pub nonce: [u8; NONCE_LEN],
    /// The `ChkKey`'s 32 bytes, AEAD-encrypted (includes the 16-byte tag).
    pub ciphertext: Vec<u8>,
}

/// Encapsulates `key` for a recipient identified by their serialized
/// ML-KEM-1024 public (encapsulation) key bytes — as received over the
/// wire from `KemKeypair::encaps_key_bytes`, not a live `KemKeypair` (the
/// sender must never hold the recipient's secret decapsulation key).
pub fn encapsulate_chk_key(
    key: &ChkKey,
    recipient_encaps_key_bytes: [u8; ml_kem_1024::EK_LEN],
) -> Result<EncapsulatedChkKey, EncapsulationError> {
    let recipient_key = encaps_key_from_bytes(recipient_encaps_key_bytes)?;
    let (symmetric_key, kem_ciphertext) = kem_encapsulate(&recipient_key)?;

    let cipher = ChaCha20Poly1305::new(Key::from_slice(&symmetric_key));
    // A random nonce would need a CSPRNG dependency this crate doesn't
    // otherwise need. `try_encaps` draws fresh internal randomness on
    // every call (confirmed by arkhe-crypto-pqc's own KEM tests: repeated
    // encapsulations against the same public key never produce the same
    // ciphertext), so the KEM ciphertext is already unique per call —
    // deriving the nonce from it avoids (symmetric_key, nonce) reuse
    // without adding a new randomness source.
    let nonce_bytes = derive_nonce(&kem_ciphertext);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher.encrypt(nonce, key.0.as_slice()).map_err(|_| EncapsulationError::Encrypt)?;

    Ok(EncapsulatedChkKey { kem_ciphertext, nonce: nonce_bytes, ciphertext })
}

/// Recovers the `ChkKey` using the recipient's own `KemKeypair`.
pub fn decapsulate_chk_key(
    recipient: &KemKeypair,
    encapsulated: &EncapsulatedChkKey,
) -> Result<ChkKey, EncapsulationError> {
    let symmetric_key = kem_decapsulate(recipient, &encapsulated.kem_ciphertext)?;
    let cipher = ChaCha20Poly1305::new(Key::from_slice(&symmetric_key));
    let nonce = Nonce::from_slice(&encapsulated.nonce);
    let plaintext =
        cipher.decrypt(nonce, encapsulated.ciphertext.as_slice()).map_err(|_| EncapsulationError::Decrypt)?;
    let key_bytes: [u8; 32] = plaintext.try_into().map_err(|_| EncapsulationError::Decrypt)?;
    Ok(ChkKey(key_bytes))
}

fn derive_nonce(kem_ciphertext: &[u8]) -> [u8; NONCE_LEN] {
    let mut h = Sha256::new();
    h.update(b"arkhe-storage/chk-key-encapsulation/nonce/v1");
    h.update(kem_ciphertext);
    let digest = h.finalize();
    let mut nonce = [0u8; NONCE_LEN];
    nonce.copy_from_slice(&digest[..NONCE_LEN]);
    nonce
}

#[cfg(test)]
mod tests {
    use super::*;
    use arkhe_crypto_pqc::kem::generate_kem_keypair;

    #[test]
    fn encapsulate_and_decapsulate_roundtrip() {
        let recipient = generate_kem_keypair().unwrap();
        let key = ChkKey([7u8; 32]);
        let encapsulated = encapsulate_chk_key(&key, recipient.encaps_key_bytes()).unwrap();
        let recovered = decapsulate_chk_key(&recipient, &encapsulated).unwrap();
        assert_eq!(recovered, key);
    }

    #[test]
    fn wrong_recipient_cannot_decapsulate() {
        let recipient = generate_kem_keypair().unwrap();
        let eavesdropper = generate_kem_keypair().unwrap();
        let key = ChkKey([9u8; 32]);
        let encapsulated = encapsulate_chk_key(&key, recipient.encaps_key_bytes()).unwrap();
        assert!(matches!(decapsulate_chk_key(&eavesdropper, &encapsulated), Err(EncapsulationError::Decrypt)));
    }

    #[test]
    fn tampered_ciphertext_fails_decapsulation() {
        let recipient = generate_kem_keypair().unwrap();
        let key = ChkKey([3u8; 32]);
        let mut encapsulated = encapsulate_chk_key(&key, recipient.encaps_key_bytes()).unwrap();
        let last = encapsulated.ciphertext.len() - 1;
        encapsulated.ciphertext[last] ^= 0xFF;
        assert!(matches!(decapsulate_chk_key(&recipient, &encapsulated), Err(EncapsulationError::Decrypt)));
    }

    #[test]
    fn two_encapsulations_of_the_same_key_use_different_kem_ciphertexts_and_nonces() {
        let recipient = generate_kem_keypair().unwrap();
        let key = ChkKey([1u8; 32]);
        let a = encapsulate_chk_key(&key, recipient.encaps_key_bytes()).unwrap();
        let b = encapsulate_chk_key(&key, recipient.encaps_key_bytes()).unwrap();
        assert_ne!(a.kem_ciphertext, b.kem_ciphertext);
        assert_ne!(a.nonce, b.nonce);
    }
}
