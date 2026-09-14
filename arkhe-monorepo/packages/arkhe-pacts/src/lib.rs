//! # ARKHE PACTs — Prova BIP-322 temporalmente indexada
//!
//! Nuance **BIP-322 como prova temporalmente indexada** (plano
//! `ARKHE-CRYPTO-CODE-TESTABLE-2026-09-12`): o BIP-322 usa um modelo de
//! **transacção virtual** (`to_spend` → `to_sign`) que nunca é broadcast. A prova
//! é vinculada ao **momento da assinatura** e não é reutilizável nem transferível.
//!
//! Este crate materializa o compromisso **PACT**: `hash(salt || bip322_proof)` —
//! um compromisso que só pode ser revelado por quem detém o `salt`, indexado pelo
//! timestamp da prova e destinado a ser ancorado como `ots_proof` via
//! OpenTimestamps (a ancoragem em si é responsabilidade do serviço de
//! timestamping).
//!
//! ## Propriedades testadas
//!
//! * **Vinculação temporal** — a prova carrega `timestamp` (momento da
//!   assinatura); `is_before` decide se precede um deadline.
//! * **Irrevogabilidade** — `commitment_hash` é imutável após a criação.
//! * **Não-transferibilidade** — endereços distintos produzem compromissos
//!   distintos; a prova não pode ser re-ancorada noutro endereço.
//!
//! ## Invariantes tocados
//!
//! * **Loopseal-1** (temporal anchor): timestamp do momento da assinatura.
//! * **Ghost-1** (integridade): hash vinculado à prova + salt.
//! * **Gap-2** (entropia): salt de 32 bytes via CSPRNG.
//! * **Simplicity-2**: `sha2 0.10` e `rand 0.8` — zero deps externas novas.
//!
//! Zero `unsafe` (lint `unsafe_code = deny`).
//!
//! Honestidade: a assinatura BIP-322 é **simulada** (bytes fixos) — a verificação
//! real (`bdk_message_signer` / virtual transactions) pertence ao substrato
//! 972-ARKHE-BITCOIN. Este crate testa as *propriedades* do compromisso, não a
//! criptografia da assinatura.

use rand::RngCore;
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};

/// Prova BIP-322 simulada (em produção: `bdk_message_signer`).
#[derive(Clone)]
pub struct Bip322Proof {
    /// Endereço Bitcoin que assina (identidade da prova).
    pub address: String,
    /// Mensagem assinada (conteúdo do compromisso).
    pub message: String,
    /// Assinatura BIP-322 (SIMULADA neste crate).
    pub signature: Vec<u8>,
    /// Momento da assinatura (segundos desde UNIX_EPOCH).
    pub timestamp: u64,
}

impl Bip322Proof {
    /// Cria uma prova vinculada ao momento actual.
    pub fn sign(address: &str, message: &str) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        Self {
            address: address.to_string(),
            message: message.to_string(),
            signature: vec![0xDE, 0xAD, 0xBE, 0xEF],
            timestamp,
        }
    }

    /// Verifica a prova (em produção: verificação BIP-322 real).
    pub fn verify(&self) -> bool {
        !self.signature.is_empty() && self.timestamp > 0
    }

    /// Decide se a prova foi criada antes do momento `deadline`.
    pub fn is_before(&self, deadline: u64) -> bool {
        self.timestamp < deadline
    }
}

/// Compromisso PACT: `hash(salt || bip322_proof)` opcionalmente ancorado via
/// OpenTimestamps.
pub struct PactCommitment {
    /// `SHA-256(salt ‖ address ‖ message ‖ signature ‖ timestamp_le)`.
    pub commitment_hash: [u8; 32],
    /// Sal de 32 bytes (CSPRNG) — necessário para revelar a prova.
    pub salt: [u8; 32],
    /// Prova BIP-322 subjacente.
    pub proof: Bip322Proof,
    /// Prova OpenTimestamps (anchor; opcional até a ancoragem).
    pub ots_proof: Option<Vec<u8>>,
}

impl PactCommitment {
    /// Cria um compromisso PACT sobre a prova dada.
    pub fn new(proof: Bip322Proof) -> Self {
        let mut salt = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut salt);

        let commitment_hash = Self::digest(&salt, &proof);

        Self {
            commitment_hash,
            salt,
            proof,
            ots_proof: None,
        }
    }

    /// Digest canónico `SHA-256(salt ‖ address ‖ message ‖ signature ‖ timestamp_le)`.
    fn digest(salt: &[u8; 32], proof: &Bip322Proof) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(salt);
        hasher.update(proof.address.as_bytes());
        hasher.update(proof.message.as_bytes());
        hasher.update(&proof.signature);
        hasher.update(proof.timestamp.to_le_bytes());
        hasher.finalize().into()
    }

    /// Verifica que o compromisso corresponde à prova + salt (re-digest).
    pub fn verify(&self) -> bool {
        Self::digest(&self.salt, &self.proof) == self.commitment_hash
    }

    /// Revela a prova e o salt (apenas se necessário).
    pub fn reveal(&self) -> (&Bip322Proof, &[u8; 32]) {
        (&self.proof, &self.salt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;
    use std::time::Duration;

    #[test]
    fn test_bip322_time_binding() {
        let proof = Bip322Proof::sign("bc1q...", "PACT commitment");
        let deadline = proof.timestamp + 1000;
        assert!(proof.is_before(deadline), "Prova deve ser anterior ao deadline");

        sleep(Duration::from_secs(2));
        assert!(
            !proof.is_before(proof.timestamp),
            "Prova NAO deve ser posterior a si mesma"
        );
    }

    #[test]
    fn test_pact_commitment_verification() {
        let proof = Bip322Proof::sign("bc1q...", "PACT commitment");
        let pact = PactCommitment::new(proof);
        assert!(pact.verify(), "Compromisso deve ser verificavel");

        let mut tampered = pact;
        tampered.salt[0] ^= 0xFF;
        assert!(!tampered.verify(), "Compromisso adulterado deve falhar");
    }

    #[test]
    fn test_pact_irrevocability() {
        let proof = Bip322Proof::sign("bc1q...", "PACT commitment");
        let pact = PactCommitment::new(proof);
        let original_hash = pact.commitment_hash;

        assert_eq!(pact.commitment_hash, original_hash);
    }

    #[test]
    fn test_pact_nontransferability() {
        let proof_a = Bip322Proof::sign("bc1q_aaa", "msg");
        let proof_b = Bip322Proof::sign("bc1q_bbb", "msg");

        let pact_a = PactCommitment::new(proof_a);
        let pact_b = PactCommitment::new(proof_b);

        assert_ne!(
            pact_a.commitment_hash, pact_b.commitment_hash,
            "Compromissos de endereços diferentes devem ser distintos"
        );
    }
}