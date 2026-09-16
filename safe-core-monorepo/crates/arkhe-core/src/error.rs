//! Erro único do sistema Arkhe.
//!
//! [`ArkheError`] é o erro que atravessa as fronteiras entre crates: quem
//! consome o `arkhe-core` (`arkhe-agi`, `arkhe-cloud-provider`,
//! `arkhe-artifact-signing`, …) fala neste tipo em vez de inventar um erro por
//! crate, e [`ArkheResult`] é o alias que o transporta.
//!
//! As variantes são deliberadamente poucas e grosseiras. Dizem *de quem é a
//! culpa* — entrada do chamador, estado do sistema, autorização — e não de que
//! subsistema veio; é o que permite a um chamador decidir se repete, se corrige
//! o pedido, ou se desiste, sem conhecer o produtor do erro.

use thiserror::Error;

/// Hash tipografada — wrapper em torno de [u8; 32].
pub type ArkheHash = [u8; 32];

/// Resultado tipado do Arkhe.
pub type ArkheResult<T> = Result<T, ArkheError>;

/// Erros centrais do sistema Arkhe.
#[derive(Debug, Error)]
pub enum ArkheError {
    /// Falha interna, não imputável ao chamador: transporte, uma resposta
    /// externa que não parseia, ou uma resposta bem-formada mas vazia na parte
    /// que se ia ler. A `String` é a descrição já formatada do erro original —
    /// repetir o pedido é a resposta habitual, corrigi-lo não é.
    #[error("Internal error: {0}")]
    Internal(String),

    /// O recurso pedido não existe: identificador desconhecido, ou entidade
    /// removida entre a listagem e o acesso. A `String` identifica o recurso.
    #[error("Not found: {0}")]
    NotFound(String),

    /// O chamador passou uma entrada que não respeita o contrato da operação.
    /// Distinta de [`ArkheError::NotFound`]: o recurso pode perfeitamente
    /// existir — é o pedido em si que é inválido.
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    /// A autorização falhou: a identidade do chamador não tem direito a
    /// executar a ação. A `String` diz por que motivo foi negada, para que a
    /// recusa seja auditável e não apenas um "não".
    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    /// A operação excedeu o prazo que lhe foi dado. Carrega o limite excedido
    /// (não o tempo realmente decorrido), formatado com `{0:?}` — é o valor de
    /// que quem trata o erro precisa para decidir se vale a pena repetir.
    #[error("Timeout: {0:?}")]
    Timeout(std::time::Duration),

    /// Uma operação de (de)serialização falhou: bytes que não correspondem ao
    /// formato esperado, ou uma estrutura que não é representável no formato
    /// de destino.
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// Erro de origem externa que não vale a pena modelar. O `#[from]` faz o
    /// operador `?` converter automaticamente qualquer
    /// `Box<dyn Error + Send + Sync>` neste variante, e
    /// `#[error(transparent)]` delega no erro interior o `Display` e a cadeia
    /// de [`source`](std::error::Error::source) — a mensagem e as causas do
    /// erro original ficam preservadas. É a última rede: quando o chamador
    /// tiver de reagir de forma diferente, acrescente-se uma variante tipada.
    #[error(transparent)]
    Other(#[from] Box<dyn std::error::Error + Send + Sync>),
}
