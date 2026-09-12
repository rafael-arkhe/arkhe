//! ECDSA com ancoragem de rotação de fase e demonstração MITM.
//!
//! A assinatura é produzida com ECDSA recuperável (secp256k1); a única diferença
//! da assinatura convencional é que o **digest pré-hash é rotacionado** por um
//! `PhaseRotation`. A verificação exige conhecer a mesma rotação λ usada na
//! assinatura. Um **MITM** que intercepte a assinatura e apresente uma rotação
//! diferente (λ′ ≠ λ) fará a verificação **falhar** — evidenciando a rotação de
//! fase, análoga ao invariante de Jarlskog `J ≠ 0`.

use k256::ecdsa::{RecoveryId, Signature as EcdsaSignature, SigningKey, VerifyingKey};
use sha3::{Digest, Keccak256};

use crate::phase::{PhaseRotation, blend_rotation};

/// Erros do protocolo ECDSA·MITM.
#[derive(Debug, thiserror::Error)]
pub enum MitmError {
    #[error("assinatura malformada: esperado 65 bytes, recebido {0}")]
    MalformedRaw(usize),
    #[error("não foi possível recuperar a chave pública a partir da assinatura")]
    RecoveryFailed,
    #[error("não foi possível assinar o prehash")]
    SigningFailed,
    #[error("chave pública não confere com a fase λ apresentada")]
    PhaseMismatch,
}

/// Assinatura ECDSA rotacionada (r ‖ s ‖ recovery-id compacto).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhaseAlignedSignature {
    /// 64 bytes `r‖s` + 1 byte `v`.
    pub raw: [u8; 65],
    /// A rotação de fase usada na produção do digest.
    pub rotation_lambda: u64,
}

fn digest_of(message: &[u8], rotation: &PhaseRotation) -> [u8; 32] {
    let h = Keccak256::digest(message);
    let mut prehash = [0u8; 32];
    prehash.copy_from_slice(&h);
    blend_rotation(prehash, rotation)
}

/// Assina uma mensagem com rotação de fase. O digest é
/// `λ ⊗ Keccak256(message)`; `sign_prehash_recoverable` recupera a chave na
/// verificação sem precisar do segredo.
pub fn sign_phase(
    sk: &SigningKey,
    message: &[u8],
    rotation: &PhaseRotation,
) -> Result<PhaseAlignedSignature, MitmError> {
    let prehash = digest_of(message, rotation);
    let (sig, rid) = sk
        .sign_prehash_recoverable(&prehash)
        .map_err(|_| MitmError::SigningFailed)?;
    let mut raw = [0u8; 65];
    raw[..64].copy_from_slice(&sig.to_bytes());
    raw[64] = u8::from(rid);
    Ok(PhaseAlignedSignature {
        raw,
        rotation_lambda: rotation.lambda,
    })
}

/// Verifica uma assinatura de fase: precisa da mesma λ para que a chave
/// recuperada volte a coincidir com a pública esperada.
pub fn verify_phase(pk: &VerifyingKey, message: &[u8], sig: &PhaseAlignedSignature) -> bool {
    let prehash = digest_of(
        message,
        &PhaseRotation {
            angle_rad: 0.0, // o ângulo é redundante na verificação; λ decide.
            lambda: sig.rotation_lambda,
        },
    );
    let parsed: EcdsaSignature = match EcdsaSignature::from_slice(&sig.raw[..64]) {
        Ok(s) => s,
        Err(_) => return false,
    };
    let rid = match RecoveryId::try_from(sig.raw[64]) {
        Ok(r) => r,
        Err(_) => return false,
    };
    let recovered = match VerifyingKey::recover_from_prehash(&prehash, &parsed, rid) {
        Ok(vk) => vk,
        Err(_) => return false,
    };
    recovered.to_encoded_point(false) == pk.to_encoded_point(false)
}

/// Demonstração MITM: intercepta uma assinatura válida e **troca** a rotação de
/// fase relatada (λ′ ≠ λ). A verificação subsequente falha.
pub fn intercept_phase(sig: &PhaseAlignedSignature, injected_lambda: u64) -> PhaseAlignedSignature {
    PhaseAlignedSignature {
        raw: sig.raw,
        rotation_lambda: injected_lambda,
    }
}

#[cfg(test)]
mod tests {
    use rand::rngs::OsRng;

    use super::*;

    #[test]
    fn sign_and_verify_with_matching_phase() {
        let sk = SigningKey::random(&mut OsRng);
        let pk = VerifyingKey::from(&sk);
        let rot = PhaseRotation::from_delta_degrees(227.0);
        let msg = b"arkhe:phase-aligned-signature";
        let sig = sign_phase(&sk, msg, &rot).unwrap();
        assert!(verify_phase(&pk, msg, &sig));
    }

    #[test]
    fn identity_phase_is_plain_ecdsa() {
        let sk = SigningKey::random(&mut OsRng);
        let pk = VerifyingKey::from(&sk);
        let sig = sign_phase(&sk, b"no-phase", &PhaseRotation::identity()).unwrap();
        assert_eq!(sig.rotation_lambda, 1);
        assert!(verify_phase(&pk, b"no-phase", &sig));
    }

    #[test]
    fn tampered_message_fails() {
        let sk = SigningKey::random(&mut OsRng);
        let pk = VerifyingKey::from(&sk);
        let rot = PhaseRotation::from_delta_degrees(227.0);
        let sig = sign_phase(&sk, b"good", &rot).unwrap();
        assert!(!verify_phase(&pk, b"tampered", &sig));
    }

    #[test]
    fn mitm_phase_rotation_is_detected() {
        let sk = SigningKey::random(&mut OsRng);
        let pk = VerifyingKey::from(&sk);
        let msg = b"intercepted";
        let auth = PhaseRotation::from_delta_degrees(227.0);
        let sig = sign_phase(&sk, msg, &auth).unwrap();
        // Atacante que finge ser a identidade (λ = 1) → verificação falha.
        let proxy = intercept_phase(&sig, 1);
        assert!(!verify_phase(&pk, msg, &proxy));
    }
}