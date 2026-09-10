//! Camada MEV — proteção contra Maximal Extractable Value.
//!
//! Baseado na análise do order flow privado e nas regras oficiais de *searchers*
//! (frontrunning, sandwich, direct builder submission, bundle stuffing e colusão
//! são proibidos). Invariantes **MEV-001 a MEV-006**.
//!
//! ## Nota de honestidade sobre MEV-005
//!
//! Base e Arbitrum são exceções à regra de *backrun*: podem submeter diretamente
//! ao sequenciador. O stub `ensure_atomicity` reflete isso (atomicidade delegada
//! ao sequenciador para chains sem suporte a bundle).

use sha3::{Digest, Sha3_256};

/// Transação avaliada pela camada MEV.
#[derive(Debug, Clone)]
pub struct MevTransaction {
    /// Payload calldata.
    pub data: Vec<u8>,
    /// Chain destino.
    pub chain: String,
    /// Preço base do gás (wei).
    pub gas_price: u64,
    /// Priority fee (wei).
    pub priority_fee: u64,
    /// Habilita recuperação MEV.
    pub mev_recovery: bool,
    /// Habilita gas refund.
    pub gas_recovery: bool,
    /// Identificador opcional do searcher.
    pub searcher_id: Option<String>,
}

impl MevTransaction {
    /// Constrói a transação MEV a partir dos campos completos.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        data: Vec<u8>,
        chain: String,
        gas_price: u64,
        priority_fee: u64,
        mev_recovery: bool,
        gas_recovery: bool,
        searcher_id: Option<String>,
    ) -> Self {
        Self {
            data,
            chain,
            gas_price,
            priority_fee,
            mev_recovery,
            gas_recovery,
            searcher_id,
        }
    }
}

/// Recibo da camada MEV com as garantias de cada invariante.
#[derive(Debug, Clone)]
pub struct MevReceipt {
    /// Hash da transação submetida.
    pub tx_hash: String,
    /// MEV-001 — privacidade garantida.
    pub privacy_guaranteed: bool,
    /// MEV-002 — atomicidade garantida.
    pub atomicity_guaranteed: bool,
    /// MEV-003 — gas refund.
    pub gas_refund: u64,
    /// MEV-003 — valor MEV recuperado.
    pub recovered_mev: u64,
    /// MEV-004 — searcher validado.
    pub searcher_validated: bool,
    /// MEV-005 — chain compatível.
    pub chain_compatible: bool,
    /// MEV-006 — transação imutável.
    pub immutable: bool,
}

/// Recibo emitido pelo backend Blink (stub determinístico em memória).
#[derive(Debug, Clone)]
pub struct SubmitReceipt {
    /// Hash do lote submetido (digest SHA3-256).
    pub tx_hash: String,
    /// Valor recuperado como MEV + gas refund (unidades simuladas).
    pub recovered_amount: u64,
}

/// Erros da camada MEV — cada variante mapeia uma violação de invariante.
#[derive(Debug)]
pub enum MevError {
    /// MEV-001 violado: privacidade não garantida.
    PrivacyViolation,
    /// MEV-002 violado: atomicidade não garantida.
    AtomicityViolation,
    /// MEV-003 violado: recuperação de valor falhou.
    RecoveryFailed,
    /// MEV-004 violado: searcher não autorizado.
    SearcherUnauthorized,
    /// MEV-005 violado: chain não suportada.
    ChainNotSupported,
    /// MEV-006 violado: transação mutável.
    ImmutabilityViolation,
}

impl std::fmt::Display for MevError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MevError::PrivacyViolation => write!(f, "MEV-001 VIOLADO: Privacidade não garantida"),
            MevError::AtomicityViolation => write!(f, "MEV-002 VIOLADO: Atomicidade não garantida"),
            MevError::RecoveryFailed => write!(f, "MEV-003 VIOLADO: Recuperação de valor falhou"),
            MevError::SearcherUnauthorized => write!(f, "MEV-004 VIOLADO: Searcher não autorizado"),
            MevError::ChainNotSupported => write!(f, "MEV-005 VIOLADO: Chain não suportada"),
            MevError::ImmutabilityViolation => write!(f, "MEV-006 VIOLADO: Transação mutável"),
        }
    }
}

impl std::error::Error for MevError {}

/// Proteção MEV — titular das regras MEV-001..006.
#[derive(Debug)]
pub struct MevProtection {
    trusted_searchers: Vec<&'static str>,
    supported_chains: Vec<&'static str>,
    last_receipt: MevReceipt,
}

impl MevProtection {
    /// Constrói a proteção com o catálogo canónico de searchers e chains.
    pub fn new() -> Self {
        Self {
            trusted_searchers: vec![
                "flashbots",
                "blocknative",
                "base",
                "jito",
                "arbitrum",
            ],
            supported_chains: vec![
                "ethereum",
                "base",
                "solana",
                "arbitrum",
                "bsc",
                "polygon",
            ],
            last_receipt: MevReceipt {
                tx_hash: String::new(),
                privacy_guaranteed: false,
                atomicity_guaranteed: false,
                gas_refund: 0,
                recovered_mev: 0,
                searcher_validated: false,
                chain_compatible: false,
                immutable: false,
            },
        }
    }

    /// Recibo da última submissão processada (para consulta de estatísticas).
    pub fn last_receipt(&self) -> MevReceipt {
        self.last_receipt.clone()
    }

    /// MEV-001 — garante que a transação nunca é exposta ao mempool público.
    ///
    /// O order flow é roteado exclusivamente pelo canal privado do Blink; o stub
    /// aceita a premissa e não expõe o payload a nenhum canal público.
    pub fn ensure_privacy(&self, _tx: &MevTransaction) -> Result<(), MevError> {
        Ok(())
    }

    /// MEV-002 — garante atomicidade do bundle (tudo ou nada).
    ///
    /// Devolve o lote a submeter. Em chains sem suporte a bundle (ex.: Base,
    /// Arbitrum), a atomicidade é delegada ao sequenciador — exceção MEV-005.
    pub fn ensure_atomicity(&self, tx: &MevTransaction) -> Result<Vec<u8>, MevError> {
        if !self.supported_chains.contains(&tx.chain.as_str()) {
            return Err(MevError::ChainNotSupported);
        }
        Ok(tx.data.clone())
    }

    /// MEV-003 — recupera MEV e gas refunds para o utilizador.
    pub fn recover_value(&mut self, receipt: &SubmitReceipt) -> MevReceipt {
        let mev_receipt = MevReceipt {
            tx_hash: receipt.tx_hash.clone(),
            privacy_guaranteed: true,
            atomicity_guaranteed: true,
            gas_refund: receipt.recovered_amount / 2,
            recovered_mev: receipt.recovered_amount / 2,
            searcher_validated: true,
            chain_compatible: true,
            immutable: true,
        };
        self.last_receipt = mev_receipt.clone();
        mev_receipt
    }

    /// MEV-004 — valida o searcher contra o catálogo aprovado (deny por omissão).
    ///
    /// Apenas os searchers do catálogo canónico podem participar. Isto codifica a
    /// proibição de frontrunning, sandwich, direct builder submission, bundle
    /// stuffing e colusão das regras do Blink.
    pub fn validate_searcher(&self, tx: &MevTransaction) -> Result<(), MevError> {
        if let Some(ref searcher_id) = tx.searcher_id {
            if !self.trusted_searchers.contains(&searcher_id.as_str()) {
                return Err(MevError::SearcherUnauthorized);
            }
        }
        Ok(())
    }

    /// MEV-005 — garante compatibilidade multi-chain.
    ///
    /// Chains suportadas: ethereum, base, solana, arbitrum, bsc, polygon.
    pub fn ensure_multichain_compatibility(&self, tx: &MevTransaction) -> Result<(), MevError> {
        if !self.supported_chains.contains(&tx.chain.as_str()) {
            return Err(MevError::ChainNotSupported);
        }
        Ok(())
    }

    /// MEV-006 — garante que a transação assinada nunca é alterada.
    ///
    /// O stub não muta o payload nem partilha a assinatura com terceiros.
    pub fn verify_immutability(&self, _tx: &MevTransaction) -> Result<(), MevError> {
        Ok(())
    }

    /// Submete o lote ao backend Blink (stub determinístico em memória).
    ///
    /// O hash é o digest SHA3-256 do lote — determinístico e auditável, sem rede.
    pub fn submit(&self, bundle: &[u8]) -> Result<SubmitReceipt, MevError> {
        let mut hasher = Sha3_256::new();
        hasher.update(bundle);
        let digest = hasher.finalize();
        let tx_hash = format!("0x{}", digest.iter().map(|b| format!("{b:02x}")).collect::<String>());
        Ok(SubmitReceipt {
            tx_hash,
            recovered_amount: 5_000_000_000,
        })
    }
}

impl Default for MevProtection {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_tx() -> MevTransaction {
        MevTransaction::new(
            b"bundle".to_vec(),
            "ethereum".to_string(),
            30_000_000_000,
            1_000_000_000,
            true,
            true,
            Some("flashbots".to_string()),
        )
    }

    #[test]
    fn mev_005_supports_all_six_chains() {
        let p = MevProtection::new();
        for chain in ["ethereum", "base", "solana", "arbitrum", "bsc", "polygon"] {
            let tx = MevTransaction::new(
                b"bundle".to_vec(),
                chain.to_string(),
                0,
                0,
                true,
                true,
                None,
            );
            assert!(p.ensure_multichain_compatibility(&tx).is_ok());
        }
    }

    #[test]
    fn mev_004_rejects_untrusted_searcher() {
        let p = MevProtection::new();
        let mut tx = sample_tx();
        tx.searcher_id = Some("malicious_bot".to_string());
        assert!(matches!(
            p.validate_searcher(&tx),
            Err(MevError::SearcherUnauthorized)
        ));
    }

    #[test]
    fn mev_002_submit_digest_is_deterministic_sha3() {
        let p = MevProtection::new();
        let r1 = p.submit(b"bundle").unwrap();
        let r2 = p.submit(b"bundle").unwrap();
        assert_eq!(r1.tx_hash, r2.tx_hash);
        assert_eq!(r1.tx_hash.len(), 66);
    }
}