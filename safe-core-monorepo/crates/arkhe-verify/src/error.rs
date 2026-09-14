//! Erros do adaptador Rekor e dos seus modelos.
//!
//! Como no resto da família `arkhe-verify`, nenhum caminho de biblioteca usa
//! `panic!`, `unwrap()` ou `expect()`: toda falha previsível é um valor deste
//! enum. As verificações em si devolvem *relatórios* (`crate::report`), não
//! `Result` — este enum é para o que **impede** uma verificação de acontecer:
//! rede, resposta, e a interpretação do formato Rekor/CT.

use thiserror::Error;

/// Falha ao falar com o Rekor, ou ao interpretar o que ele devolve.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RekorError {
    /// A requisição HTTP não completou (DNS, TLS, conexão recusada, timeout).
    #[error("falha de transporte: {0}")]
    Transport(String),

    /// O log não tem entrada naquela posição.
    #[error("o Rekor não tem entrada para o logIndex {log_index}")]
    NotFound {
        /// Posição pedida.
        log_index: u64,
    },

    /// O log respondeu `429`.
    #[error("o Rekor respondeu 429 (rate limit)")]
    RateLimited,

    /// O log respondeu `5xx`.
    #[error("o Rekor respondeu {status}")]
    Provider {
        /// O status HTTP devolvido.
        status: u16,
    },

    /// A resposta veio, mas não tem a forma esperada.
    #[error("resposta inesperada: {reason}")]
    UnexpectedResponse {
        /// O que não bateu.
        reason: String,
    },

    /// A entrada de log desserializou, mas não é utilizável.
    #[error("entrada de log malformada: {reason}")]
    MalformedLogEntry {
        /// O que faltou ou não bateu.
        reason: String,
    },

    /// O checkpoint (uma *signed note* do CT) não tem a forma esperada.
    #[error("checkpoint malformado: {reason}")]
    MalformedCheckpoint {
        /// O que não bateu.
        reason: String,
    },

    /// Uma codificação hex ou base64 do formato não pôde ser decodificada.
    #[error("falha de codificação: {0}")]
    Encoding(String),

    /// Nenhuma chave pública conhecida para o `key id` de um witness.
    ///
    /// Uma *signed note* identifica o witness por um `key id` de 4 bytes, não
    /// pela chave pública: resolver um no outro é responsabilidade de quem
    /// chama, via [`crate::rekor::WitnessKeyring`].
    #[error("nenhuma chave conhecida para o key id `{key_id}`")]
    UnknownWitnessKey {
        /// O `key id` de 4 bytes, em base64, como aparece na note.
        key_id: String,
    },
}
