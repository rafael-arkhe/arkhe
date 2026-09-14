//! §2.1 — `verify_signature`: Ed25519 conferido contra um trust root.

use ed25519_dalek::{Signature, VerifyingKey};

use crate::encoding::{decode_fixed_hex, PUBLIC_KEY_HEX_LEN};
use crate::error::VerifyError;

/// Tamanho de uma assinatura Ed25519, em bytes.
pub const ED25519_SIGNATURE_LEN: usize = 64;

/// Confere uma assinatura Ed25519 com uma chave pública já decodificada.
///
/// Usa `verify_strict`, e não `verify`: a variante estrita também rejeita
/// chaves públicas de ordem pequena, que de outro modo tornariam a assinatura
/// forjável para vários messages distintos com a mesma assinatura. Não há
/// razão para aceitar a variante permissiva num verificador client-side.
pub fn verify_with_key(
    message: &[u8],
    signature: &[u8],
    key_bytes: &[u8; PUBLIC_KEY_HEX_LEN],
) -> Result<(), VerifyError> {
    let verifying_key =
        VerifyingKey::from_bytes(key_bytes).map_err(|_| VerifyError::InvalidPublicKey)?;

    let actual = signature.len();
    if actual != ED25519_SIGNATURE_LEN {
        return Err(VerifyError::WrongLength {
            expected: ED25519_SIGNATURE_LEN,
            actual,
        });
    }
    let signature =
        Signature::from_slice(signature).map_err(|_| VerifyError::InvalidSignature)?;

    verifying_key
        .verify_strict(message, &signature)
        .map_err(|_| VerifyError::InvalidSignature)
}

/// Confere uma assinatura Ed25519 **e** que a chave está no trust root.
///
/// A ordem importa: a chave é conferida contra o trust root *antes* de
/// qualquer trabalho criptográfico, de modo que uma chave não confiável nunca
/// chega a ser usada para verificar — e o erro devolvido é
/// [`VerifyError::UntrustedKey`], que diz exatamente por que falhou, em vez
/// de um genérico "assinatura inválida".
pub fn verify_signature_inner(
    message: &[u8],
    signature: &[u8],
    public_key_hex: &str,
    trust_root: &crate::encoding::TrustRoot,
) -> Result<(), VerifyError> {
    let key_bytes = decode_fixed_hex::<PUBLIC_KEY_HEX_LEN>(public_key_hex)?;
    if !trust_root.contains(&key_bytes) {
        return Err(VerifyError::UntrustedKey);
    }
    verify_with_key(message, signature, &key_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encoding::{encode_hex, TrustRoot};
    use ed25519_dalek::{Signer, SigningKey};

    /// Chaves determinísticas a partir de sementes fixas.
    ///
    /// Sementes fixas em vez de aleatoriedade: os testes ficam reprodutíveis
    /// e o crate não precisa de uma dependência de RNG (que no alvo
    /// `wasm32-unknown-unknown` exigiria configuração de `getrandom`).
    fn signing_key(seed: u8) -> SigningKey {
        SigningKey::from_bytes(&[seed; 32])
    }

    fn public_hex(seed: u8) -> String {
        encode_hex(signing_key(seed).verifying_key().as_bytes())
    }

    fn sign(seed: u8, message: &[u8]) -> Vec<u8> {
        signing_key(seed).sign(message).to_bytes().to_vec()
    }

    const MESSAGE: &[u8] = b"arkhe-os: attestation subject";

    #[test]
    fn accepts_a_valid_key_that_is_in_the_trust_root() {
        let root = TrustRoot::parse(&format!("[\"{}\"]", public_hex(1))).expect("parse");
        assert_eq!(
            verify_signature_inner(MESSAGE, &sign(1, MESSAGE), &public_hex(1), &root),
            Ok(())
        );
    }

    #[test]
    fn rejects_a_valid_signature_from_a_key_outside_the_trust_root() {
        // Assinatura criptograficamente perfeita, chave (a do seed 2) ausente
        // do trust root: tem de ser rejeitada mesmo assim.
        let root = TrustRoot::parse(&format!("[\"{}\"]", public_hex(1))).expect("parse");
        let signature = sign(2, MESSAGE);
        assert_eq!(
            verify_signature_inner(MESSAGE, &signature, &public_hex(2), &root),
            Err(VerifyError::UntrustedKey)
        );
        // Sanidade: a mesma assinatura é válida se a chave 2 for confiável.
        let root2 = TrustRoot::parse(&format!("[\"{}\"]", public_hex(2))).expect("parse");
        assert_eq!(
            verify_signature_inner(MESSAGE, &signature, &public_hex(2), &root2),
            Ok(())
        );
    }

    #[test]
    fn rejects_a_valid_key_signing_a_different_message() {
        let root = TrustRoot::parse(&format!("[\"{}\"]", public_hex(1))).expect("parse");
        let signature = sign(1, b"outra mensagem");
        assert_eq!(
            verify_signature_inner(MESSAGE, &signature, &public_hex(1), &root),
            Err(VerifyError::InvalidSignature)
        );
    }

    #[test]
    fn rejects_a_tampered_signature() {
        let root = TrustRoot::parse(&format!("[\"{}\"]", public_hex(1))).expect("parse");
        let mut signature = sign(1, MESSAGE);
        signature[0] ^= 0x01;
        assert_eq!(
            verify_signature_inner(MESSAGE, &signature, &public_hex(1), &root),
            Err(VerifyError::InvalidSignature)
        );
    }

    #[test]
    fn rejects_a_signature_with_the_wrong_length() {
        let root = TrustRoot::parse(&format!("[\"{}\"]", public_hex(1))).expect("parse");
        let short = &sign(1, MESSAGE)[..63];
        assert_eq!(
            verify_signature_inner(MESSAGE, short, &public_hex(1), &root),
            Err(VerifyError::WrongLength {
                expected: 64,
                actual: 63
            })
        );
    }

    #[test]
    fn rejects_a_malformed_or_truncated_public_key() {
        let root = TrustRoot::parse(&format!("[\"{}\"]", public_hex(1))).expect("parse");
        assert!(matches!(
            verify_signature_inner(MESSAGE, &sign(1, MESSAGE), "nao-hex", &root),
            Err(VerifyError::InvalidHex(_))
        ));
        assert!(matches!(
            verify_signature_inner(MESSAGE, &sign(1, MESSAGE), &public_hex(1)[..62], &root),
            Err(VerifyError::WrongLength { .. })
        ));
    }

    #[test]
    fn an_empty_trust_root_trusts_nothing() {
        let root = TrustRoot::parse("[]").expect("parse");
        assert_eq!(
            verify_signature_inner(MESSAGE, &sign(1, MESSAGE), &public_hex(1), &root),
            Err(VerifyError::UntrustedKey)
        );
    }

    #[test]
    fn accepts_an_empty_message() {
        let root = TrustRoot::parse(&format!("[\"{}\"]", public_hex(1))).expect("parse");
        assert_eq!(
            verify_signature_inner(b"", &sign(1, b""), &public_hex(1), &root),
            Ok(())
        );
        // Uma assinatura do vazio não vale para um message não-vazio.
        assert_eq!(
            verify_signature_inner(MESSAGE, &sign(1, b""), &public_hex(1), &root),
            Err(VerifyError::InvalidSignature)
        );
    }
}
