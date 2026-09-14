//! Content-Hash Key (CHK) convergent encryption.
//!
//! A CHK blob is encrypted under a key whose key is *derived from the
//! plaintext's own content hash*, so identical plaintexts share a
//! `content_hash` (enabling deduplication of ciphertext by content id) while
//! still producing distinct ciphertexts under random nonces.
//!
//! Unlike classic convergent encryption — where the ciphertext itself is a
//! deterministic function of the content, and therefore leaks content
//! equality to anyone who can see two stored blobs — this scheme deliberately
//! decouples the two: the *key* is convergent (so the blob is deduplicable
//! and verifiable by possession of the content), the *ciphertext is not*
//! (`nonce` is fresh per call, so the wire bytes differ every time). It keeps
//! identical plaintexts mapping to distinct ciphertexts that still share a
//! `content_hash` for dedup indexing. This module does not touch the
//! KEM/signature modules: CHK needs no keypair, only the plaintext.
//!
//! Layout of one blob:
//!
//! 1. `content_hash = BLAKE3(plaintext)` (32 bytes) — content id + key material.
//! 2. `key = HKDF-SHA256(ikm = content_hash, info = "arkhe-crypto-pqc/chk/xchacha/v1")`.
//! 3. `nonce = random 24 bytes` (XChaCha20 extended nonce).
//! 4. `ct = XChaCha20-Poly1305(key, nonce, plaintext)` (AEAD, 16-byte tag).
//! 5. Stored ciphertext = `nonce || ct`.
//!
//! # Security limit of this design (labelled, not hidden)
//!
//! The key is a **public function of the plaintext**: anyone who holds (or can
//! guess) the content can recompute `content_hash`, rederive `key`, and decrypt
//! the blob. This is inherent to the content-addressed-key idea and is not
//! fixable by any choice of primitive — a CHK blob therefore gives **no
//! confidentiality against a party that can enumerate the plaintext** (the
//! standard confirmation-of-guessing weakness of convergent encryption). The
//! random nonce does not mitigate that; what it buys is narrower and real: a
//! passive observer cannot tell *whether two blobs hold the same content* by
//! comparing ciphertexts.
//!
//! What CHK *does* give, and what it is for: a blob whose claimed content id is
//! bound to its plaintext (integrity / tamper detection) that can be
//! deduplicated and verified by any party that already has the content. Note
//! that the AEAD tag alone does *not* bind `content_hash` to the plaintext,
//! precisely because the key is public: an attacker holding a plaintext can
//! encrypt it under `HKDF(any_hash)` and produce a blob that authenticates. The
//! explicit re-hash in [`chk_decrypt`]/[`chk_verify`] is what closes that gap,
//! and it is load-bearing, not redundant.
//!
//! # Serialization
//!
//! No `serde` derive here, matching [`crate::kem`] and [`crate::signature`]:
//! neither module serializes, and this crate does not depend on `serde`.
//! (A derive would also need `serde-big-array` for the `[u8; NONCE_LEN]`
//! field, since `serde` only derives arrays up to 32 elements out of the box.)
//!
//! # Duplicate implementation in this workspace (reported, not unified)
//!
//! `arkhe-storage/src/chk.rs` implements the same *concept* with different
//! guarantees and is **not** interoperable with this one:
//!
//! | | `arkhe-crypto-pqc::chk` (this module) | `arkhe-storage::chk` |
//! |---|---|---|
//! | Content digest | BLAKE3 | SHA-256 |
//! | Confidentiality | XChaCha20-Poly1305 AEAD | SHA-256 counter-mode keystream (explicitly **not** AEAD) |
//! | Integrity | Poly1305 tag **plus** content-hash re-check | none on the block; only a Merkle root over chunk addresses |
//! | Ciphertext | non-convergent (fresh nonce) | convergent (deterministic, byte-identical for equal content) |
//! | Unit | one blob | chunk + Merkle tree |
//!
//! Because the digests differ, a `content_hash` from one module is not a valid
//! id for the other even for identical bytes. This is a documented duplication
//! for a human to resolve; this module does not unify them.

use blake3;
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use hkdf::Hkdf;
use rand::rngs::OsRng;
use rand::RngCore;
use sha2::Sha256;
use zeroize::Zeroize;

/// HKDF `info` string binding derived keys to this module, this cipher and this
/// revision. Changing it invalidates every existing blob.
const CHK_INFO: &[u8] = b"arkhe-crypto-pqc/chk/xchacha/v1";

/// Length of the XChaCha20 extended nonce, in bytes. Also the length of
/// [`ChkEncrypted::nonce`].
pub const NONCE_LEN: usize = 24;

/// Length of the content hash (BLAKE3-256), in bytes. Also the length of
/// [`ChkEncrypted::content_hash`].
pub const CONTENT_HASH_LEN: usize = 32;

#[derive(Debug, thiserror::Error)]
pub enum ChkError {
    #[error("HKDF expansion failed")]
    KeyDerivation,
    #[error("cipher construction failed")]
    CipherInit,
    #[error("AEAD encryption failed")]
    Encrypt,
    #[error("AEAD decryption failed (bad key or tampered ciphertext)")]
    Decrypt,
    /// The blob decrypted, but its plaintext does not hash to the
    /// `content_hash` the blob claims.
    ///
    /// This is not reachable by tampering with a well-formed blob (changing
    /// `content_hash` changes the derived key, so the AEAD tag fails first and
    /// the error is [`ChkError::Decrypt`]). It *is* reachable by an attacker who
    /// holds the plaintext and re-encrypts it under `HKDF(other_hash)` — see
    /// the module-level note on why the AEAD tag alone cannot bind
    /// `content_hash` to the plaintext. This variant is the content-address
    /// integrity check firing; it did not exist under this name in the earlier
    /// draft of this module (which reported a hash mismatch through its generic
    /// decryption error), and it is broken out here so callers can tell
    /// "authentication failed" from "authenticated but mis-addressed".
    #[error("content hash mismatch (blob is not addressed by its own plaintext)")]
    ContentHashMismatch,
}

/// A CHK-encrypted blob: `nonce || ciphertext||tag`, plus the content id.
///
/// # Not serialized
/// `Debug`/`Clone` only — see the module-level serialization note.
#[derive(Debug, Clone)]
pub struct ChkEncrypted {
    /// BLAKE3 hash of the plaintext (content id + key derivation input).
    pub content_hash: [u8; CONTENT_HASH_LEN],
    /// A random nonce (not a zero nonce) is safe here because the key is
    /// unique per content hash: a 24-byte XChaCha20 nonce has no realistic
    /// collision risk under random selection, and a fresh nonce per call is
    /// exactly what keeps the ciphertext non-convergent even when the key (and
    /// therefore the plaintext) repeats.
    pub nonce: [u8; NONCE_LEN],
    /// XChaCha20-Poly1305 output over the plaintext, including its 16-byte
    /// Poly1305 tag. The in-memory blob is `nonce (24B) || AEAD ciphertext
    /// (incl. 16B tag)`; the nonce has its own field here so an on-wire encoder
    /// cannot forget it.
    pub ciphertext: Vec<u8>,
}

/// Derives the CHK symmetric key for `content_hash` via HKDF-SHA256.
///
/// The "secret" input keying material is the content hash itself, which is why
/// this key is *not* confidential — see the module-level security limit.
fn derive_key(content_hash: &[u8; CONTENT_HASH_LEN]) -> Result<[u8; 32], ChkError> {
    let hk = Hkdf::<Sha256>::new(None, content_hash);
    let mut okm = [0u8; 32];
    hk.expand(CHK_INFO, &mut okm)
        .map_err(|_| ChkError::KeyDerivation)?;
    Ok(okm)
}

/// Encrypt `plaintext` with CHK.
///
/// The returned blob carries `BLAKE3(plaintext)` and the ciphertext produced
/// under `HKDF-SHA256(BLAKE3(plaintext))` with a freshly generated nonce, so two
/// calls over equal plaintexts agree on `content_hash` and differ in both
/// `nonce` and `ciphertext`.
pub fn chk_encrypt(plaintext: &[u8]) -> Result<ChkEncrypted, ChkError> {
    let content_hash = *blake3::hash(plaintext).as_bytes();
    let mut key = derive_key(&content_hash)?;

    let cipher = XChaCha20Poly1305::new_from_slice(&key).map_err(|_| ChkError::CipherInit)?;
    key.zeroize();

    let mut nonce = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut nonce);

    let ciphertext = cipher
        .encrypt(XNonce::from_slice(&nonce), plaintext)
        .map_err(|_| ChkError::Encrypt)?;

    Ok(ChkEncrypted {
        content_hash,
        nonce,
        ciphertext,
    })
}

/// Decrypt a CHK blob back to plaintext.
///
/// Returns `Err` — never partially-authenticated bytes — if the AEAD tag fails
/// or if the recovered plaintext does not hash to the blob's `content_hash`.
pub fn chk_decrypt(encrypted: &ChkEncrypted) -> Result<Vec<u8>, ChkError> {
    let mut key = derive_key(&encrypted.content_hash)?;

    let cipher = XChaCha20Poly1305::new_from_slice(&key).map_err(|_| ChkError::CipherInit)?;
    key.zeroize();

    let plaintext = cipher
        .decrypt(
            XNonce::from_slice(&encrypted.nonce),
            encrypted.ciphertext.as_ref(),
        )
        .map_err(|_| ChkError::Decrypt)?;

    // Content-address integrity: the AEAD alone proves only that the ciphertext
    // was made by whoever holds HKDF(content_hash) — and that key is public
    // given the plaintext. Re-hashing is what actually binds the blob's claimed
    // content id to its contents.
    if *blake3::hash(&plaintext).as_bytes() != encrypted.content_hash {
        return Err(ChkError::ContentHashMismatch);
    }

    Ok(plaintext)
}

/// Verify that a plaintext matches a blob's `content_hash`.
///
/// Decrypts, recomputes `BLAKE3(plaintext)` and returns it, so a caller can
/// compare it to a content id it obtained elsewhere (an index, a manifest, a
/// peer announcement). Fails for the same reasons [`chk_decrypt`] fails.
pub fn chk_verify(encrypted: &ChkEncrypted) -> Result<[u8; CONTENT_HASH_LEN], ChkError> {
    let plaintext = chk_decrypt(encrypted)?;
    Ok(*blake3::hash(&plaintext).as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blob_for(plaintext: &[u8]) -> ChkEncrypted {
        chk_encrypt(plaintext).expect("encryption of a valid plaintext cannot fail")
    }

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let plaintext = b"Arkhe P2P message payload with ZK proof data";
        let blob = blob_for(plaintext);
        let decrypted = chk_decrypt(&blob).expect("roundtrip must decrypt");
        assert_eq!(decrypted, plaintext);
    }

    /// The assertion that proves the design: the key is content-convergent
    /// (`content_hash` repeats) but the ciphertext is not (`nonce` and
    /// `ciphertext` differ), so blob bytes do not leak content equality.
    #[test]
    fn identical_plaintexts_share_content_hash_but_not_ciphertext() {
        let plaintext = b"the same bytes, twice";
        let a = blob_for(plaintext);
        let b = blob_for(plaintext);

        assert_eq!(a.content_hash, b.content_hash);
        assert_eq!(a.content_hash, *blake3::hash(plaintext).as_bytes());

        assert_ne!(
            a.nonce, b.nonce,
            "nonce must be random per call, not derived from the content"
        );
        assert_ne!(
            a.ciphertext, b.ciphertext,
            "identical plaintexts must not produce identical ciphertexts"
        );

        // ...and both still round-trip to the same plaintext.
        assert_eq!(chk_decrypt(&a).unwrap(), chk_decrypt(&b).unwrap());
    }

    #[test]
    fn tampered_ciphertext_fails_decrypt() {
        let mut blob = blob_for(b"Original data");
        blob.ciphertext[0] ^= 0xFF;
        assert!(matches!(chk_decrypt(&blob), Err(ChkError::Decrypt)));
    }

    #[test]
    fn tampered_ciphertext_fails_verify() {
        let mut blob = blob_for(b"Original data");
        blob.ciphertext[0] ^= 0xFF;
        assert!(chk_verify(&blob).is_err());
    }

    #[test]
    fn truncated_ciphertext_fails_decrypt() {
        let mut blob = blob_for(b"Original data");
        blob.ciphertext.truncate(4);
        assert!(chk_decrypt(&blob).is_err());
    }

    #[test]
    fn tampered_content_hash_fails_verify() {
        let mut blob = blob_for(b"Original data");
        blob.content_hash[0] ^= 0xFF;
        // The content hash is also the KDF input, so a changed hash yields a
        // different key and the AEAD tag fails before the re-hash is reached.
        assert!(matches!(chk_decrypt(&blob), Err(ChkError::Decrypt)));
        assert!(chk_verify(&blob).is_err());
    }

    /// A blob encrypted by an attacker who knows the plaintext (so they know
    /// the key) under a *different* content hash still authenticates — the AEAD
    /// tag cannot bind the claimed content id. The explicit re-hash in
    /// `chk_decrypt` catches it, which is why `ContentHashMismatch` exists and
    /// is not dead code.
    #[test]
    fn blob_mis_addressed_by_attacker_fails_content_hash_check() {
        let real_plaintext = b"alpha";
        let claimed_hash = *blake3::hash(b"beta").as_bytes();

        let mut key = derive_key(&claimed_hash).unwrap();
        let cipher = XChaCha20Poly1305::new_from_slice(&key).unwrap();
        key.zeroize();
        let mut nonce = [0u8; NONCE_LEN];
        OsRng.fill_bytes(&mut nonce);
        let ciphertext = cipher
            .encrypt(XNonce::from_slice(&nonce), real_plaintext.as_ref())
            .unwrap();

        let forged = ChkEncrypted {
            content_hash: claimed_hash,
            nonce,
            ciphertext,
        };

        // The AEAD is satisfied (the attacker could derive the key)...
        assert!(matches!(
            chk_decrypt(&forged),
            Err(ChkError::ContentHashMismatch)
        ));
        // ...and no bytes are handed back.
        assert!(chk_verify(&forged).is_err());
    }

    #[test]
    fn empty_plaintext_roundtrips() {
        let blob = blob_for(b"");
        assert_eq!(blob.content_hash, *blake3::hash(b"").as_bytes());
        // Empty plaintext => tag only.
        assert_eq!(blob.ciphertext.len(), 16);
        assert_eq!(chk_decrypt(&blob).unwrap(), Vec::<u8>::new());
        assert_eq!(chk_verify(&blob).unwrap(), blob.content_hash);
    }

    #[test]
    fn verify_returns_the_expected_hash() {
        let plaintext = b"verifiable by possession";
        let blob = blob_for(plaintext);
        assert_eq!(
            chk_verify(&blob).expect("verify must succeed"),
            *blake3::hash(plaintext).as_bytes()
        );
    }

    #[test]
    fn different_plaintexts_derive_different_hashes_and_keys() {
        let a = blob_for(b"content A");
        let b = blob_for(b"content B");
        assert_ne!(a.content_hash, b.content_hash);
        assert_ne!(
            *blake3::hash(b"content A").as_bytes(),
            *blake3::hash(b"content B").as_bytes()
        );
        // A blob cannot be decrypted as its neighbour's content.
        let mut mixed = a.clone();
        mixed.content_hash = b.content_hash;
        assert!(chk_decrypt(&mixed).is_err());
    }

    #[test]
    fn key_len_and_constants_are_consistent() {
        assert_eq!(NONCE_LEN, 24);
        assert_eq!(CONTENT_HASH_LEN, 32);
        assert_eq!(blob_for(b"x").nonce.len(), NONCE_LEN);
        assert_eq!(blob_for(b"x").content_hash.len(), CONTENT_HASH_LEN);
    }
}
