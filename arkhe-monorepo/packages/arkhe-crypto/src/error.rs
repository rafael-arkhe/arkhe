//! Tipos de erro unificados do crate `arkhe-crypto`.

use thiserror::Error;

/// Erros unificados dos módulos `dkg`, `hash` e `bip322`.
#[derive(Debug, Error)]
pub enum CryptoError {
    /// A característica `bip322` não está ativada (as dependências
    /// `bdk_message_signer`/`bdk_wallet` não estão ativas no lock).
    #[error("bip322 feature not enabled")]
    FeatureDisabled,

    /// O endereço fornecido não pôde ser interpretado ou não pertence
    /// à rede declarada.
    #[error("invalid address: {0}")]
    InvalidAddress(String),

    /// A prova/assinatura BIP-322 não está em base64 válido.
    #[error("invalid signature encoding: {0}")]
    InvalidSignature(String),

    /// A construção da wallet efêmera falhou.
    #[error("wallet construction failed: {0}")]
    WalletConstruction(String),

    /// Falha na verificação (inclui PSBT não-finalizado — Fix #8).
    #[error("verification failed: {0}")]
    Verification(String),
}

/// Resultado padrão dos módulos do crate.
pub type Result<T> = std::result::Result<T, CryptoError>;