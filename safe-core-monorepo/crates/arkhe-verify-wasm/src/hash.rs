//! §2.1 — `verify_sha256`: SHA-256 e a conferência contra um digest declarado.

use sha2::{Digest, Sha256};

use crate::encoding::decode_fixed_hex;
use crate::error::VerifyError;

/// Tamanho do digest SHA-256, em bytes.
pub const SHA256_LEN: usize = 32;

/// SHA-256 de `data`, em bytes.
pub fn sha256_bytes(data: &[u8]) -> [u8; SHA256_LEN] {
    let digest = Sha256::digest(data);
    let mut out = [0u8; SHA256_LEN];
    out.copy_from_slice(digest.as_slice());
    out
}

/// SHA-256 de `data`, em hex minúsculo.
pub fn sha256_hex(data: &[u8]) -> String {
    crate::encoding::encode_hex(&sha256_bytes(data))
}

/// Confere `SHA-256(data)` contra `expected_hex`.
///
/// A comparação dos digests é feita com `==`, que **não** é tempo constante.
/// Aqui isso é deliberado e não é um problema: o digest e a mensagem são
/// dados públicos, e o que uma comparação não-constante revelaria é apenas
/// quantos bytes iniciais coincidem — informação que o verificador já expõe
/// de qualquer forma ao devolver `true`/`false`. A verificação que sim
/// depende de segredo (a assinatura) usa a checagem em tempo constante da
/// `ed25519-dalek`.
pub fn verify_sha256_inner(data: &[u8], expected_hex: &str) -> Result<(), VerifyError> {
    let expected = decode_fixed_hex::<SHA256_LEN>(expected_hex)?;
    if sha256_bytes(data) == expected {
        Ok(())
    } else {
        Err(VerifyError::DigestMismatch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Vetores conhecidos do NIST (FIPS 180-4 / exemplo A.1 do SHA-256).
    const SHA256_ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
    const SHA256_EMPTY: &str =
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

    #[test]
    fn known_vector_abc() {
        assert_eq!(sha256_hex(b"abc"), SHA256_ABC);
        assert_eq!(verify_sha256_inner(b"abc", SHA256_ABC), Ok(()));
    }

    #[test]
    fn known_vector_empty_string() {
        assert_eq!(sha256_hex(b""), SHA256_EMPTY);
        assert_eq!(verify_sha256_inner(b"", SHA256_EMPTY), Ok(()));
    }

    #[test]
    fn rejects_a_wrong_digest_for_the_same_payload() {
        // O digest do payload vazio, comparado contra o payload `abc`.
        assert_eq!(
            verify_sha256_inner(b"abc", SHA256_EMPTY),
            Err(VerifyError::DigestMismatch)
        );
    }

    #[test]
    fn rejects_one_bit_of_difference_in_the_payload() {
        let mut payload = b"arkhe".to_vec();
        let good = sha256_hex(&payload);
        assert_eq!(verify_sha256_inner(&payload, &good), Ok(()));

        payload[0] ^= 0x01;
        assert_eq!(
            verify_sha256_inner(&payload, &good),
            Err(VerifyError::DigestMismatch)
        );
    }

    #[test]
    fn accepts_uppercase_hex_and_0x_prefix() {
        let upper = SHA256_ABC.to_uppercase();
        assert_eq!(verify_sha256_inner(b"abc", &upper), Ok(()));
        assert_eq!(verify_sha256_inner(b"abc", &format!("0x{SHA256_ABC}")), Ok(()));
    }

    #[test]
    fn rejects_a_malformed_declared_digest() {
        assert!(matches!(
            verify_sha256_inner(b"abc", "não é hex"),
            Err(VerifyError::InvalidHex(_))
        ));
        assert!(matches!(
            verify_sha256_inner(b"abc", "abcd"),
            Err(VerifyError::WrongLength {
                expected: 32,
                actual: 2
            })
        ));
    }

    #[test]
    fn digest_is_32_bytes_and_payload_length_independent() {
        assert_eq!(sha256_bytes(b"").len(), SHA256_LEN);
        assert_eq!(sha256_bytes(&[0u8; 1024]).len(), SHA256_LEN);
    }
}
