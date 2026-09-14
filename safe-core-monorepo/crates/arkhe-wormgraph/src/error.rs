//! Erros tipados do wormgraph.
//!
//! Nenhum caminho desta crate usa `panic!`, `unwrap()` ou `expect()`: toda
//! falha previsível é um valor deste enum.

use thiserror::Error;

/// Falha ao anexar uma entrada ou ao verificar a cadeia.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum WormGraphError {
    /// Já existe um nó com este `id`.
    ///
    /// `id` é a chave de consulta de [`crate::WormGraph::node`]; permitir
    /// duplicatas tornaria a consulta ambígua, então a segunda inserção é
    /// rejeitada em vez de aceita silenciosamente.
    #[error("já existe um nó com id `{0}`")]
    DuplicateNode(String),

    /// Uma aresta referencia um nó que não foi anexado ao grafo.
    #[error("aresta referencia nó desconhecido `{0}` — anexe o nó antes da aresta")]
    UnknownNode(String),

    /// O `payload` do nó não pôde ser serializado em JSON canônico, portanto
    /// não há como comprometer-se ao seu conteúdo no hash da cadeia.
    #[error("falha ao serializar o payload em JSON canônico: {0}")]
    Serialization(String),

    /// O campo `sequence` da entrada não corresponde à sua posição na cadeia.
    #[error("cadeia quebrada na posição {index}: `sequence` não corresponde à posição")]
    SequenceMismatch {
        /// Posição (0-based) em que a verificação falhou.
        index: u64,
    },

    /// O `prev_hash` da entrada não é o hash da entrada anterior.
    ///
    /// Acontece quando uma entrada é removida ou trocada de posição: o
    /// conteúdo da entrada pode continuar internamente coerente, mas ela
    /// aponta para o predecessor errado.
    #[error("cadeia quebrada na posição {index}: `prev_hash` não é o hash da entrada anterior")]
    LinkBroken {
        /// Posição (0-based) em que a verificação falhou.
        index: u64,
    },

    /// O hash recomputado a partir do conteúdo da entrada não confere com o
    /// hash que ela armazena.
    ///
    /// É a detecção de adulteração de conteúdo: alterar qualquer campo
    /// (`node_type`, `signer`, `invariants`, `payload`, `timestamp`, ...)
    /// muda o hash recomputado.
    #[error("cadeia quebrada na posição {index}: hash recomputado não confere com o armazenado")]
    HashMismatch {
        /// Posição (0-based) em que a verificação falhou.
        index: u64,
    },
}
