//! # ARKHE STARK — resistência pós-quântica condicional à hash function
//!
//! Nuance **STARKs pós-quânticos condicionais à hash function** (plano
//! `ARKHE-CRYPTO-CODE-TESTABLE-2026-09-12`): STARKs usam hash functions
//! resistentes a colisões (não ECC). A resistência pós-quântica do **SHA-256** é
//! de **128 bits** (Grover reduz a pré-imagem `2^256` → `2^128`) — suficiente
//! para 128 bits, insuficiente para 256. Para segurança mais forte,
//! **SHAKE256** (FIPS 202) oferece **256 bits** pós-quânticos.
//!
//! ## Propriedades testadas
//!
//! * [`StarkHashBackend::pq_security_bits`]: SHA-256 → 128 bits, SHAKE256 → 256
//!   bits; `is_sufficient` decide se o backend atende o nível requerido.
//! * [`StarkSecurityParams::validate`]: fecho da política — rejeita um STARK
//!   configurado com hash abaixo do nível de segurança exigido.
//! * Determinismo dos digests por backend.
//!
//! ## Honestidade
//!
//! Este crate **não implementa STARKs** (sem FRI/IOP/proof system) — materializa
//! exclusivamente a *condicionalidade* da resistência pós-quântica à hash
//! function escolhida e a validação de parâmetros. A claim SHA-256 = 128 bits PQ
//! (Grover) é um secure-claim padrão, não uma prova contida neste crate; a
//! escolha SHAKE256 para 256 bits segue o FIPS 202.
//!
//! Zero `unsafe` (lint `unsafe_code = deny`).

use sha2::{Digest, Sha256};
use sha3::{
    digest::{ExtendableOutput, Update, XofReader},
    Shake256,
};

/// Backend de hash para STARK.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StarkHashBackend {
    /// SHA-256 — 128 bits pós-quânticos (Grover reduz 256 → 128).
    Sha256,
    /// SHAKE256 (FIPS 202) — 256 bits pós-quânticos.
    Shake256,
}

impl StarkHashBackend {
    /// Bits de segurança pós-quântica do backend.
    pub fn pq_security_bits(&self) -> u32 {
        match self {
            Self::Sha256 => 128,
            Self::Shake256 => 256,
        }
    }

    /// Decide se o backend atende o nível de segurança requerido.
    pub fn is_sufficient(&self, required_bits: u32) -> bool {
        self.pq_security_bits() >= required_bits
    }

    /// Gera um compromisso hash determinístico (32 bytes de saída).
    pub fn hash(&self, data: &[u8]) -> Vec<u8> {
        match self {
            Self::Sha256 => {
                let mut hasher = Sha256::new();
                Digest::update(&mut hasher, data);
                hasher.finalize().to_vec()
            }
            Self::Shake256 => {
                let mut hasher = Shake256::default();
                Update::update(&mut hasher, data);
                let mut reader = hasher.finalize_xof();
                let mut output = vec![0u8; 32];
                reader.read(&mut output);
                output
            }
        }
    }
}

/// Parâmetros de segurança STARK.
pub struct StarkSecurityParams {
    /// Hash function escolhida para o proof system.
    pub hash_backend: StarkHashBackend,
    /// Nível de segurança da prova em si (convenção do STARK).
    pub proof_security_bits: u32,
    /// Nível de segurança pós-quântico exigido pela política.
    pub required_pq_bits: u32,
}

impl StarkSecurityParams {
    /// Valida que o backend entrega pelo menos os bits PQ exigidos.
    pub fn validate(&self) -> Result<(), String> {
        if !self.hash_backend.is_sufficient(self.required_pq_bits) {
            return Err(format!(
                "Hash backend {:?} fornece {} bits PQ, mas sao necessarios {}",
                self.hash_backend,
                self.hash_backend.pq_security_bits(),
                self.required_pq_bits
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sha256_pq_security() {
        let backend = StarkHashBackend::Sha256;
        assert_eq!(backend.pq_security_bits(), 128);
        assert!(backend.is_sufficient(128));
        assert!(!backend.is_sufficient(256));
    }

    #[test]
    fn test_shake256_pq_security() {
        let backend = StarkHashBackend::Shake256;
        assert_eq!(backend.pq_security_bits(), 256);
        assert!(backend.is_sufficient(256));
    }

    #[test]
    fn test_stark_params_validation() {
        let params_sha256 = StarkSecurityParams {
            hash_backend: StarkHashBackend::Sha256,
            proof_security_bits: 128,
            required_pq_bits: 128,
        };
        assert!(params_sha256.validate().is_ok());

        let params_insufficient = StarkSecurityParams {
            hash_backend: StarkHashBackend::Sha256,
            proof_security_bits: 128,
            required_pq_bits: 256,
        };
        assert!(params_insufficient.validate().is_err());

        let params_shake = StarkSecurityParams {
            hash_backend: StarkHashBackend::Shake256,
            proof_security_bits: 256,
            required_pq_bits: 256,
        };
        assert!(params_shake.validate().is_ok());
    }

    #[test]
    fn test_hash_determinism() {
        let data = b"test data";
        let sha = StarkHashBackend::Sha256;
        let shake = StarkHashBackend::Shake256;

        assert_eq!(sha.hash(data), sha.hash(data));
        assert_eq!(shake.hash(data), shake.hash(data));
        assert_ne!(sha.hash(data), shake.hash(data));
    }
}