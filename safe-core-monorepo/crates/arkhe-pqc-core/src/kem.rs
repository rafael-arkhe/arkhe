//! ML-KEM-1024 key encapsulation via `pqcrypto-mlkem`.
//!
//! Same zeroization caveat as `hybrid.rs`:
//! `pqcrypto_mlkem::mlkem1024::SecretKey` does not implement `Zeroize`
//! either (same underlying reason — it's a bare PQClean byte-array
//! wrapper), so no `Drop` impl is written here for the same reasons
//! documented in `hybrid.rs`'s module doc comment.

use pqcrypto_mlkem::mlkem1024::{
    decapsulate as pqc_decapsulate, encapsulate as pqc_encapsulate, keypair, Ciphertext, PublicKey, SecretKey,
    SharedSecret,
};

/// ML-KEM-1024 keypair. See the module doc comment on why `secret_key`
/// cannot be zeroized on drop with this crate choice.
pub struct KemKeyPair {
    pub public_key: PublicKey,
    secret_key: SecretKey,
}

impl KemKeyPair {
    pub fn generate() -> Self {
        let (public_key, secret_key) = keypair();
        Self { public_key, secret_key }
    }
}

/// Encapsulates against `public_key`, returning the shared secret and the
/// ciphertext to send to the key's owner.
pub fn encapsulate(public_key: &PublicKey) -> (SharedSecret, Ciphertext) {
    pqc_encapsulate(public_key)
}

/// Decapsulates `ciphertext` using `keypair`'s secret key, recovering the
/// same shared secret [`encapsulate`] produced.
pub fn decapsulate(ciphertext: &Ciphertext, keypair: &KemKeyPair) -> SharedSecret {
    pqc_decapsulate(ciphertext, &keypair.secret_key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pqcrypto_mlkem::mlkem1024::ciphertext_bytes;
    use pqcrypto_traits::kem::{Ciphertext as _, SharedSecret as _};

    #[test]
    fn encaps_decaps_shared_secret_matches() {
        let keypair = KemKeyPair::generate();
        let (shared_a, ciphertext) = encapsulate(&keypair.public_key);
        let shared_b = decapsulate(&ciphertext, &keypair);
        assert_eq!(shared_a.as_bytes(), shared_b.as_bytes());
    }

    #[test]
    fn ciphertext_has_expected_length() {
        let keypair = KemKeyPair::generate();
        let (_, ciphertext) = encapsulate(&keypair.public_key);
        assert_eq!(ciphertext.as_bytes().len(), ciphertext_bytes());
    }

    #[test]
    fn different_keypairs_yield_different_shared_secrets() {
        let kp_a = KemKeyPair::generate();
        let kp_b = KemKeyPair::generate();
        let (shared_a, _) = encapsulate(&kp_a.public_key);
        let (shared_b, _) = encapsulate(&kp_b.public_key);
        assert_ne!(shared_a.as_bytes(), shared_b.as_bytes());
    }
}
