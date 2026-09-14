//! Erros tipados da verificação.
//!
//! Nenhum caminho de biblioteca desta crate usa `panic!`, `unwrap()` ou
//! `expect()`: toda falha previsível é um valor deste enum, e os pontos de
//! entrada `#[wasm_bindgen]` o convertem em um `bool` (ou em um relatório
//! JSON, no caso de [`crate::verify_attestation`]).

use thiserror::Error;

/// Falha ao interpretar uma entrada ou ao verificar uma afirmação.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum VerifyError {
    /// A string não é hex válido.
    #[error("hex inválido: {0}")]
    InvalidHex(String),

    /// A string não é base64 válido.
    #[error("base64 inválido: {0}")]
    InvalidBase64(String),

    /// A string decodificou para um número de bytes diferente do exigido.
    #[error("esperava {expected} bytes, recebeu {actual}")]
    WrongLength {
        /// Número de bytes exigido pelo campo.
        expected: usize,
        /// Número de bytes efetivamente recebido.
        actual: usize,
    },

    /// O JSON não pôde ser interpretado.
    #[error("JSON inválido: {0}")]
    InvalidJson(String),

    /// O trust root não é um array JSON de chaves públicas Ed25519 em hex.
    #[error("trust root inválido: {0}")]
    InvalidTrustRoot(String),

    /// A chave pública apresentada não está no trust root.
    ///
    /// Uma assinatura criptograficamente válida por uma chave **não
    /// confiável** é rejeitada: sem esta checagem, qualquer um poderia
    /// assinar qualquer coisa e a verificação não diria nada.
    #[error("chave pública apresentada não está no trust root")]
    UntrustedKey,

    /// Os bytes não formam uma chave pública Ed25519 válida.
    #[error("chave pública Ed25519 inválida")]
    InvalidPublicKey,

    /// A assinatura Ed25519 não confere com a mensagem e a chave.
    #[error("assinatura Ed25519 inválida")]
    InvalidSignature,

    /// O digest SHA-256 calculado não confere com o digest declarado.
    #[error("digest SHA-256 não confere com o declarado")]
    DigestMismatch,

    /// A prova de inclusão Merkle não confere com a raiz informada.
    #[error("prova de inclusão Merkle inválida: {0}")]
    InvalidInclusionProof(String),

    /// O limiar pedido é menor que o mínimo exigido.
    #[error("quórum pedido ({threshold}) abaixo do mínimo exigido ({minimum})")]
    QuorumBelowMinimum {
        /// Limiar pedido pelo chamador.
        threshold: u32,
        /// Mínimo exigido pela crate.
        minimum: u32,
    },

    /// Menos witnesses válidos do que o limiar exige.
    #[error("quórum não atingido: {valid} witnesses válidos, {required} exigidos")]
    QuorumNotMet {
        /// Número de witnesses distintos com assinatura válida e confiável.
        valid: u32,
        /// Limiar exigido.
        required: u32,
    },
}
