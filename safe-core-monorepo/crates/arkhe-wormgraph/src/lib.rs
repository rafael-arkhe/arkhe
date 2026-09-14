//! `arkhe-wormgraph` — §1.5 do plano Arkhe OS: o **grafo causal**.
//!
//! Um [`WormGraph`] é um log append-only de nós e arestas tipados. Cada
//! entrada carrega o hash encadeado da anterior (`prev_hash`), no mesmo
//! espírito do `AuditLog` de `arkhe-governance`: não existe remoção nem
//! edição pela API, e [`WormGraph::verify_chain`] recomputa a cadeia inteira
//! de modo que qualquer adulteração — conteúdo reescrito, entrada removida,
//! entradas trocadas de posição — seja **detectada** em vez de meramente
//! impossível de escrever.
//!
//! # O que a crate entrega
//!
//! - **Nós e arestas**: [`Node`] e [`Edge`], com tipo e timestamp, ambos com
//!   `Serialize`/`Deserialize` e campos públicos, então o grafo inteiro faz
//!   round-trip por JSON.
//! - **Consultas**: por tipo de nó, tipo de aresta, signatário e invariante
//!   referenciado — isoladamente ou combinados em [`NodeFilter`].
//! - **Imutabilidade**: append-only com hash encadeado; [`WormGraph::chain_hash`]
//!   é estável e verificável, e [`WormGraph::verify_chain`] o confere.
//! - **Erros tipados**: [`WormGraphError`], com `thiserror`.
//! - **Ponte para `arkhe-evidence`**: [`evidence::build_from_evidence_chain`]
//!   constrói o grafo a partir de uma `EvidenceChain` (um nó por registro, com
//!   os hashes encadeados preservados) e [`evidence::verify_against_chain`]
//!   cruza as duas verificações. Ver a nota de modelo de execução abaixo.
//!
//! # Reaproveitamento de `arkhe_core`
//!
//! O hash é `arkhe_core::hash::blake3_hash` sobre [`arkhe_core::ArkheHash`]
//! (`[u8; 32]`) — a mesma primitiva que `arkhe-core` e `arkhe-evidence` já
//! usam, verificada nesta máquina antes do uso. Não há dependência direta de
//! `blake3` aqui.
//!
//! [//]: # (O `CanonicalEncoder` de `arkhe-governance` não é reaproveitável: é)
//! [//]: # (`pub(crate)` àquele crate. A codificação canônica é reimplementada)
//! [//]: # (localmente, com prefixos de comprimento e separador de domínio.)
//!
//! # Modelo de execução: síncrono, com uma ponte assíncrona
//!
//! O [`WormGraph`] e toda a API de grafo continuam **síncronos**. A única parte
//! assíncrona é o módulo [`evidence`], porque `EvidenceChain` é assíncrona
//! (`tokio::sync::RwLock`) — as funções daquela ponte são `async` para poder
//! aguardar a cadeia, mas o que fazem com o grafo é síncrono, e este crate não
//! passou a depender do tokio para compilar (o tokio entra só como dependência
//! de desenvolvimento, para os testes terem executor). Nenhum outro módulo
//! mudou de modelo.

#![deny(unsafe_code)]
#![warn(missing_docs)]

pub mod error;
pub mod evidence;
pub mod graph;
pub mod types;

pub use error::WormGraphError;
pub use evidence::{
    build_from_evidence_chain, evidence_nodes, node_id_for, verify_against_chain,
    EvidenceBridgeError, EVIDENCE_EDGE_TYPE, EVIDENCE_NODE_TYPE,
};
pub use graph::{NodeFilter, WormGraph, GENESIS_HASH};
pub use types::{Edge, Entry, Node, WormEntry};
