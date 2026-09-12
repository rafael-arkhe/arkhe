//! Erros do crate `arkhe-bitcoin`.

use thiserror::Error;

/// Erros recuperáveis na geração, parsing e assinatura de chaves/endereços.
#[derive(Error, Debug)]
pub enum BitcoinError {
    /// Chave privada inválida (tamanho, range ou bytes).
    #[error("invalid private key: {0}")]
    InvalidPrivateKey(String),

    /// Chave pública inválida.
    #[error("invalid public key: {0}")]
    InvalidPublicKey(String),

    /// Endereço inválido (RFC ou parse).
    #[error("invalid address: {0}")]
    InvalidAddress(String),

    /// Endereço pertence a outra rede.
    #[error("network mismatch: expected {expected}, got {actual}")]
    NetworkMismatch {
        /// Rede esperada.
        expected: String,
        /// Rede efetiva do endereço.
        actual: String,
    },

    /// Falha de serialização.
    #[error("serialization error: {0}")]
    Serialization(String),

    /// Falha criptográfica (curva, hash, assinatura).
    #[error("crypto error: {0}")]
    Crypto(String),

    /// Falha na integração com o substrate PoTT (924).
    #[error("PoTT integration error: {0}")]
    PottError(String),
}

/// Resultado padrão do crate.
pub type Result<T> = std::result::Result<T, BitcoinError>;