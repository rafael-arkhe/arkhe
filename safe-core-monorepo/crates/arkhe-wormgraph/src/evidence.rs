//! Ponte entre o wormgraph e o log de evidências encadeado
//! (`arkhe-evidence`).
//!
//! Um [`WormGraph`] e uma [`EvidenceChain`] são duas cadeias de hash
//! independentes, com fórmulas diferentes:
//!
//! | | hash de uma entrada |
//! |:---|:---|
//! | `WormGraph` | `BLAKE3(domínio ∥ sequence ∥ prev_hash ∥ conteúdo)` |
//! | `EvidenceChain` | `BLAKE3(prev_hash ∥ len(payload) ∥ payload)` |
//!
//! Por isso as duas **não** são comparáveis campo a campo pelo hash: o que se
//! preserva aqui é o par `(hash, prev_hash)` que o `arkhe-evidence` calculou,
//! gravado no `payload` do nó. O grafo passa a *carregar* a cadeia de
//! evidências, e o cruzamento é feito sobre esses valores preservados — não
//! sobre o `chain_hash` do wormgraph.
//!
//! # O que a ponte entrega
//!
//! - [`build_from_evidence_chain`] — um [`Node`] por [`EvidenceRecord`], com os
//!   hashes encadeados preservados, e uma aresta por par de registros
//!   consecutivos.
//! - [`verify_against_chain`] — cruza as **duas** verificações
//!   ([`WormGraph::verify_chain`] e [`EvidenceChain::verify_chain`]) e depois
//!   confere que o grafo de fato representa aquela cadeia.
//!
//! # O que o cruzamento pega que o `EvidenceChain` sozinho não pega
//!
//! O hash de um [`EvidenceRecord`] cobre `(prev_hash, payload)` — **não** cobre
//! `index` nem `timestamp`, e [`EvidenceChain::verify_chain`] confere a posição
//! pelo lugar que o registro ocupa, não pelo campo `index` que ele mesmo
//! declara. Duas consequências, ambas deliberadas no `arkhe-evidence` e ambas
//! cobertas por [`verify_against_chain`]:
//!
//! - Editar o `timestamp` de um registro deixa a cadeia de evidências
//!   verificando (`Ok`) — e o cruzamento falha com
//!   [`EvidenceBridgeError::TimestampMismatch`], porque o carimbo preservado no
//!   nó continua sendo o antigo.
//! - Editar o campo `index` de um registro idem — falha com
//!   [`EvidenceBridgeError::RecordIndexMismatch`].
//!
//! Isto é um ganho do cruzamento, não uma correção do `arkhe-evidence`: os dois
//! campos são metadados de posicionamento, e a cadeia de evidências é explícita
//! sobre não confiar neles.
//!
//! # Modelo de execução — o `WormGraph` continua síncrono
//!
//! [`EvidenceChain`] é assíncrona (usa `tokio::sync::RwLock`), e o
//! [`WormGraph`] não é — nem passa a ser. A assincronia fica **contida nesta
//! ponte**: as funções daqui são `async`, porque precisam aguardar a cadeia,
//! mas tudo o que elas fazem com o grafo ([`WormGraph::append_node`],
//! [`WormGraph::verify_chain`]) é síncrono, e nenhuma outra parte do wormgraph
//! mudou. Este crate **não** depende do tokio para compilar: a biblioteca só
//! devolve e aguarda futures do `arkhe-evidence` (o tokio entra apenas como
//! dependência de desenvolvimento, para os testes terem um executor).
//!
//! # Reaproveitamento
//!
//! Nada da lógica de hash é duplicado: o wormgraph continua usando
//! [`arkhe_core::hash::blake3_hash`] para a própria cadeia, e aqui só se usa
//! [`arkhe_core::hash::hash_to_hex`] para gravar os hashes que o
//! `arkhe-evidence` já calculou. A fórmula do `EvidenceRecord` permanece
//! privada àquela crate.
//!
//! # Precedente espelhado
//!
//! O desenho segue `arkhe-geometric-verifier::provenance_graph`
//! (`build_from_chain` / `validate`), que já constrói um grafo tipado a partir
//! de uma `EvidenceChain`: valida a cadeia antes de construir, devolve um nó por
//! registro, **não** copia o payload para dentro do nó (ele já vive na cadeia, e
//! duplicá-lo deixaria o grafo discordar dela), guarda `payload_len` para
//! detectar truncamento e confere a própria construção antes de devolver.

use arkhe_core::hash::hash_to_hex;
use arkhe_evidence::{EvidenceChain, EvidenceRecord};
use serde::Deserialize;
use thiserror::Error;

use crate::error::WormGraphError;
use crate::types::{Edge, Node};
use crate::WormGraph;

/// O `node_type` dos nós que representam um registro de evidência.
pub const EVIDENCE_NODE_TYPE: &str = "evidence.record";

/// O `edge_type` das arestas que ligam um registro ao seguinte.
pub const EVIDENCE_EDGE_TYPE: &str = "follows";

/// O `id` do nó que representa o registro `index`.
///
/// O índice é único e estável dentro de uma cadeia, então ele é a chave natural
/// — e [`WormGraph::append_node`] rejeita duplicatas, o que faz uma colisão de
/// `id` virar erro em vez de um grafo ambíguo.
pub fn node_id_for(index: u64) -> String {
    format!("evidence/{index}")
}

/// A inversa de [`node_id_for`], para relatar **qual** índice uma aresta de
/// fato liga quando ela diverge do esperado. `None` se a ponta não for um `id`
/// de registro.
fn index_of_node_id(id: &str) -> Option<u64> {
    id.strip_prefix("evidence/")?.parse().ok()
}

/// O `payload` que um nó de evidência carrega.
///
/// Só os campos escalares entram: `index`, `timestamp` (no próprio campo
/// `timestamp` do [`Node`]), e os hashes que o `arkhe-evidence` calculou, em
/// hex. O payload do registro **não** é copiado — ver a nota em
/// [`build_from_evidence_chain`].
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct RecordPayload {
    index: u64,
    hash: String,
    prev_hash: String,
    payload_len: u64,
}

/// Falha ao construir o grafo a partir de uma cadeia, ou ao cruzar as duas
/// verificações.
///
/// Todas as variantes nomeiam **o que** divergiu e **onde**: o cruzamento é a
/// única coisa que esta ponte faz, e um erro que só dissesse "não confere"
/// obrigaria quem chama a refazer a comparação para descobrir o campo.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum EvidenceBridgeError {
    /// A própria `EvidenceChain` não verifica — payload editado, registro
    /// removido ou dois registros trocados de posição. Reportado com a
    /// explicação do `arkhe-evidence`, não re-derivada aqui.
    ///
    /// Defensivo em relação à API pública atual do `arkhe-evidence`: o campo
    /// `records` é privado e não há mutador público, então hoje uma cadeia
    /// construída por `append` sempre verifica. A variante existe para o dia em
    /// que houver um caminho de entrada para uma cadeia já adulterada
    /// (persistência, desserialização) — e para não trocar um erro tipado por um
    /// `expect`.
    #[error("a cadeia de evidências não está íntegra: {reason}")]
    ChainNotIntact {
        /// O erro do `arkhe-evidence`, renderizado.
        reason: String,
    },

    /// O próprio `WormGraph` não verifica: uma entrada foi reescrita, removida
    /// ou reordenada depois de anexada.
    #[error("o wormgraph não está íntegro: {reason}")]
    GraphNotIntact {
        /// O erro do wormgraph, renderizado.
        reason: String,
    },

    /// A anexação de um nó ou de uma aresta foi rejeitada pelo wormgraph
    /// durante a construção.
    ///
    /// Inalcançável a partir de uma cadeia íntegra (os `id` derivam de índices
    /// únicos e cada aresta é anexada depois das duas pontas), mas propagado
    /// como valor em vez de tratado com `expect`: "não deveria acontecer" não é
    /// o mesmo que "não pode acontecer".
    #[error("falha ao anexar ao grafo: {0}")]
    GraphBuild(#[from] WormGraphError),

    /// O grafo não tem um nó de evidência para cada registro da cadeia.
    ///
    /// É também o que acontece quando um nó de evidência é substituído por um
    /// de outro tipo: o substituto não conta como nó de evidência, então a
    /// contagem cai. Entradas de outros tipos que **convivem** com o subgrafo,
    /// em vez de substituí-lo, são ignoradas — o wormgraph admite um log misto,
    /// e um nó de outro tipo não é assunto desta ponte.
    #[error("o grafo tem {graph_nodes} nós de evidência, a cadeia tem {chain_records} registros")]
    RecordCountMismatch {
        /// Nós de evidência no grafo.
        graph_nodes: usize,
        /// Registros na cadeia.
        chain_records: usize,
    },

    /// O grafo tem um nó para uma posição que a cadeia não tem.
    #[error("o grafo tem um nó para o índice {index}, que a cadeia não tem")]
    MissingRecord {
        /// Índice pedido.
        index: u64,
    },

    /// O `payload` do nó não tem os campos esperados, ou eles têm o tipo
    /// errado — logo não há como compará-lo com o registro.
    #[error("payload inválido no nó do índice {index}: {reason}")]
    MalformedPayload {
        /// Índice do nó.
        index: u64,
        /// O que o `serde` reclamou.
        reason: String,
    },

    /// O índice auto-declarado no `payload` não é a posição que o nó ocupa.
    #[error("o nó na posição {position} declara index {index}")]
    MisplacedIndex {
        /// Posição em que o nó efetivamente está.
        position: usize,
        /// O índice que o nó declara.
        index: u64,
    },

    /// O índice auto-declarado pelo registro da cadeia não é a posição dele.
    ///
    /// `EvidenceChain::verify_chain` **não** confere este campo (o hash de um
    /// registro não cobre `index`), então esta é a única checagem que o pega.
    #[error("o registro na posição {position} declara index {index}")]
    RecordIndexMismatch {
        /// Posição em que o registro efetivamente está.
        position: usize,
        /// O índice que o registro declara.
        index: u64,
    },

    /// O `timestamp` preservado no nó não é o `timestamp` do registro.
    ///
    /// O hash de um registro também **não** cobre `timestamp`: uma cadeia com
    /// um carimbo reescrito continua verificando. Aqui não.
    #[error("o carimbo do índice {index} diverge: o nó preserva outro timestamp")]
    TimestampMismatch {
        /// Índice do registro.
        index: u64,
    },

    /// O hash preservado no nó não é o hash do registro.
    #[error("o hash do índice {index} diverge entre o nó e o registro")]
    HashMismatch {
        /// Índice do registro.
        index: u64,
    },

    /// O `prev_hash` preservado no nó não é o `prev_hash` do registro.
    #[error("o prev_hash do índice {index} diverge entre o nó e o registro")]
    PrevHashMismatch {
        /// Índice do registro.
        index: u64,
    },

    /// O comprimento de payload preservado no nó não é o do registro.
    #[error("o comprimento de payload do índice {index} diverge entre o nó e o registro")]
    PayloadLenMismatch {
        /// Índice do registro.
        index: u64,
    },

    /// O grafo não tem exatamente uma aresta de evidência por par de registros
    /// consecutivos.
    ///
    /// Uma aresta com o `edge_type` errado conta como ausente — ela é filtrada
    /// antes da contagem, junto com as arestas de outros tipos que um grafo
    /// misto legitimamente carrega.
    #[error("o grafo tem {graph_edges} arestas de evidência, esperava {expected_edges}")]
    EdgeCountMismatch {
        /// Arestas de evidência no grafo.
        graph_edges: usize,
        /// Arestas esperadas: `registros - 1`.
        expected_edges: usize,
    },

    /// O índice de destino não é o do registro seguinte.
    #[error(
        "a aresta na posição {position} deveria ligar {from} a {to}, mas liga {found_from} a {found_to}"
    )]
    EdgeMismatch {
        /// Posição da aresta entre as arestas de evidência.
        position: usize,
        /// Índice de origem esperado.
        from: u64,
        /// Índice de destino esperado.
        to: u64,
        /// Índice de origem efetivamente encontrado.
        found_from: u64,
        /// Índice de destino efetivamente encontrado.
        found_to: u64,
    },

    /// Uma aresta de evidência tem uma ponta que não é um nó de registro.
    ///
    /// [`WormGraph::append_edge`] rejeita uma aresta com ponta desconhecida,
    /// mas [`WormGraph::from_entries`] não valida nada — então um grafo montado
    /// à mão pode chegar aqui com uma aresta apontando para fora do subgrafo de
    /// evidências.
    #[error("a aresta na posição {position} tem a ponta `{endpoint}`, que não é um registro")]
    EdgeEndpointNotARecord {
        /// Posição da aresta entre as arestas de evidência.
        position: usize,
        /// O `id` de nó que não pôde ser lido como índice de registro.
        endpoint: String,
    },
}

/// O nó que representa `record`.
fn node_for(record: &EvidenceRecord) -> Node {
    Node::new(
        node_id_for(record.index),
        EVIDENCE_NODE_TYPE,
        record.timestamp,
    )
    .with_payload(serde_json::json!({
        "index": record.index,
        "hash": hash_to_hex(&record.hash),
        "prev_hash": hash_to_hex(&record.prev_hash),
        "payload_len": record.payload.len(),
    }))
}

/// Itera sobre os nós de evidência do grafo, na ordem em que foram anexados.
///
/// Filtra por [`EVIDENCE_NODE_TYPE`]: um grafo causal pode carregar outros
/// tipos de nó, e o cruzamento só diz respeito a estes.
pub fn evidence_nodes(graph: &WormGraph) -> impl Iterator<Item = &Node> + '_ {
    graph.nodes().filter(|node| node.node_type == EVIDENCE_NODE_TYPE)
}

/// Constrói o grafo sobre todos os registros de `chain`.
///
/// Falha com [`EvidenceBridgeError::ChainNotIntact`] se
/// [`EvidenceChain::verify_chain`] rejeitar a cadeia, e nunca devolve um grafo
/// que não passe em [`WormGraph::verify_chain`] — então um `Ok` significa que o
/// grafo é uma cadeia wormgraph íntegra *e* um retrato de uma cadeia de
/// evidências íntegra.
///
/// Um nó por registro, em ordem de cadeia, e uma aresta por par adjacente: uma
/// cadeia de `n` registros rende `n` nós e `n - 1` arestas; uma cadeia vazia
/// rende um grafo vazio. Nenhum nó representa a sentinela de gênese, então a
/// contagem de nós é sempre igual à de registros.
///
/// O payload do registro **não** é copiado para dentro do nó, e sim o seu
/// comprimento: o payload já vive na cadeia, e duplicá-lo aqui criaria uma
/// segunda cópia que poderia discordar da primeira. O que precisa ser preservado
/// é o encadeamento — `hash` e `prev_hash` —, e é isso que o `payload` do nó
/// carrega.
///
/// O `timestamp` da aresta é o do registro **de destino** (a aresta existe a
/// partir do momento em que o sucessor é anexado), e o nó não recebe
/// signatário: um registro de evidência não é assinado, e inventar um
/// signatário tornaria essa distinção impossível de recuperar.
///
/// ```no_run
/// # async fn exemplo() {
/// use arkhe_evidence::EvidenceChain;
/// use arkhe_wormgraph::evidence::build_from_evidence_chain;
///
/// let chain = EvidenceChain::new();
/// chain.append(b"turno um".to_vec(), 1_700_000_000).await;
/// chain.append(b"turno dois".to_vec(), 1_700_000_060).await;
///
/// let graph = build_from_evidence_chain(&chain).await.unwrap();
/// assert_eq!(graph.len(), 3); // 2 nós + 1 aresta
/// # }
/// ```
pub async fn build_from_evidence_chain(
    chain: &EvidenceChain,
) -> Result<WormGraph, EvidenceBridgeError> {
    chain
        .verify_chain()
        .await
        .map_err(|err| EvidenceBridgeError::ChainNotIntact {
            reason: err.to_string(),
        })?;

    let count = chain.len().await;
    let mut graph = WormGraph::new();

    for position in 0..count {
        let index = position as u64;
        let record = chain.record_at(index).await.ok_or_else(|| {
            EvidenceBridgeError::ChainNotIntact {
                reason: format!("o registro {index} desapareceu durante a construção do grafo"),
            }
        })?;

        graph.append_node(node_for(&record))?;

        if position > 0 {
            let from = position as u64 - 1;
            let to = index;
            graph.append_edge(Edge::new(
                node_id_for(from),
                node_id_for(to),
                EVIDENCE_EDGE_TYPE,
                record.timestamp,
            ))?;
        }
    }

    // Mesma postura do precedente (`provenance_graph::build_from_chain`
    // termina em `validate(&graph)?`): a construção confere a si mesma antes de
    // devolver, em vez de prometer na documentação uma propriedade que não
    // testou.
    graph
        .verify_chain()
        .map_err(|err| EvidenceBridgeError::GraphNotIntact {
            reason: err.to_string(),
        })?;

    Ok(graph)
}

/// Cruza as duas verificações: a do wormgraph, a da cadeia de evidências, e a
/// concordância entre elas.
///
/// A ordem é deliberada. Primeiro cada lado verifica **nos próprios termos**
/// ([`WormGraph::verify_chain`], [`EvidenceChain::verify_chain`]) — se um dos
/// dois está quebrado, o diagnóstico é o dele, não uma divergência de
/// cruzamento. Só então o grafo é conferido contra a cadeia, nó a nó: contagem,
/// índice, carimbo, `hash`, `prev_hash`, comprimento de payload, e depois as
/// arestas.
///
/// Um grafo pode verificar consigo mesmo e ainda assim não representar esta
/// cadeia — foi construído a partir de outra, ou montado à mão com
/// [`WormGraph::from_entries`] e hashes coerentes. É esse caso que o cruzamento
/// existe para pegar, e é o único que os dois `verify_chain` isolados não pegam.
///
/// O escopo é o **subgrafo de evidências**: nós com [`EVIDENCE_NODE_TYPE`] e
/// arestas com [`EVIDENCE_EDGE_TYPE`]. Entradas de outros tipos são ignoradas,
/// já que o wormgraph admite um único log para nós e arestas de todos os tipos;
/// o que se exige é que o subgrafo de evidências seja exatamente o retrato da
/// cadeia.
pub async fn verify_against_chain(
    graph: &WormGraph,
    chain: &EvidenceChain,
) -> Result<(), EvidenceBridgeError> {
    graph
        .verify_chain()
        .map_err(|err| EvidenceBridgeError::GraphNotIntact {
            reason: err.to_string(),
        })?;
    chain
        .verify_chain()
        .await
        .map_err(|err| EvidenceBridgeError::ChainNotIntact {
            reason: err.to_string(),
        })?;

    let chain_len = chain.len().await;
    let node_count = evidence_nodes(graph).count();
    if node_count != chain_len {
        return Err(EvidenceBridgeError::RecordCountMismatch {
            graph_nodes: node_count,
            chain_records: chain_len,
        });
    }

    for (position, node) in evidence_nodes(graph).enumerate() {
        let index = position as u64;
        let record = chain
            .record_at(index)
            .await
            .ok_or(EvidenceBridgeError::MissingRecord { index })?;

        let payload: RecordPayload = serde_json::from_value(node.payload.clone()).map_err(|err| {
            EvidenceBridgeError::MalformedPayload {
                index,
                reason: err.to_string(),
            }
        })?;

        // O `index` do nó é conferido contra a **posição**, não contra o
        // `index` do registro: o `arkhe-evidence` é explícito sobre não confiar
        // no campo auto-declarado de um registro depois de uma adulteração, e
        // uma segunda fonte de verdade aqui só criaria a chance de as duas
        // discordarem.
        if payload.index != index {
            return Err(EvidenceBridgeError::MisplacedIndex { position, index: payload.index });
        }
        // O registro, esse sim, é comparado com o próprio campo — porque este é
        // justamente o campo que `EvidenceChain::verify_chain` não olha.
        if record.index != index {
            return Err(EvidenceBridgeError::RecordIndexMismatch {
                position,
                index: record.index,
            });
        }

        if node.timestamp != record.timestamp {
            return Err(EvidenceBridgeError::TimestampMismatch { index });
        }
        if payload.hash != hash_to_hex(&record.hash) {
            return Err(EvidenceBridgeError::HashMismatch { index });
        }
        if payload.prev_hash != hash_to_hex(&record.prev_hash) {
            return Err(EvidenceBridgeError::PrevHashMismatch { index });
        }
        if payload.payload_len != record.payload.len() as u64 {
            return Err(EvidenceBridgeError::PayloadLenMismatch { index });
        }
    }

    // Arestas de evidência, na ordem de anexação. Arestas de outros tipos são
    // ignoradas: o wormgraph admite um log misto, e o que esta ponte responde é
    // pelo subgrafo de evidências. Uma aresta de evidência com o `edge_type`
    // errado, por outro lado, é filtrada aqui e aparece como divergência de
    // contagem — que é o diagnóstico correto, porque ela de fato não está lá.
    let edges: Vec<&Edge> = graph
        .edges()
        .filter(|edge| edge.edge_type == EVIDENCE_EDGE_TYPE)
        .collect();
    let expected_edges = chain_len.saturating_sub(1);
    if edges.len() != expected_edges {
        return Err(EvidenceBridgeError::EdgeCountMismatch {
            graph_edges: edges.len(),
            expected_edges,
        });
    }
    for (position, edge) in edges.iter().enumerate() {
        let from = position as u64;
        let to = from + 1;
        let Some(found_from) = index_of_node_id(&edge.from) else {
            return Err(EvidenceBridgeError::EdgeEndpointNotARecord {
                position,
                endpoint: edge.from.clone(),
            });
        };
        let Some(found_to) = index_of_node_id(&edge.to) else {
            return Err(EvidenceBridgeError::EdgeEndpointNotARecord {
                position,
                endpoint: edge.to.clone(),
            });
        };
        if (found_from, found_to) != (from, to) {
            return Err(EvidenceBridgeError::EdgeMismatch {
                position,
                from,
                to,
                found_from,
                found_to,
            });
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Entry, WormEntry};
    use arkhe_evidence::GENESIS_HASH;

    /// Uma cadeia com `n` registros, payload `registro-{i}` e carimbo `i + 1`.
    async fn chain_of(n: usize) -> EvidenceChain {
        let chain = EvidenceChain::new();
        for index in 0..n {
            chain
                .append(format!("registro-{index}").into_bytes(), index as u64 + 1)
                .await;
        }
        chain
    }

    /// Uma cadeia equivalente a `chain_of(3)`, mas com os carimbos deslocados.
    /// Os hashes do `arkhe-evidence` **não** cobrem `timestamp`, então esta
    /// cadeia tem exatamente os mesmos `hash`/`prev_hash` da original.
    async fn chain_with_shifted_timestamps() -> EvidenceChain {
        let chain = EvidenceChain::new();
        for index in 0..3 {
            chain
                .append(format!("registro-{index}").into_bytes(), index as u64 + 100)
                .await;
        }
        chain
    }

    /// Monta um grafo à mão a partir de entradas, sem passar pelo construtor.
    fn hand_built(entries: Vec<WormEntry>) -> WormGraph {
        WormGraph::from_entries(entries)
    }

    // --- construção -------------------------------------------------------

    #[tokio::test]
    async fn an_empty_chain_builds_an_empty_graph() {
        let chain = EvidenceChain::new();
        let graph = build_from_evidence_chain(&chain).await.expect("constrói");

        assert!(graph.is_empty());
        assert_eq!(graph.len(), 0);
        assert_eq!(evidence_nodes(&graph).count(), 0);
        assert_eq!(verify_against_chain(&graph, &chain).await, Ok(()));
    }

    #[tokio::test]
    async fn a_single_record_yields_one_node_and_no_edges() {
        let chain = chain_of(1).await;
        let graph = build_from_evidence_chain(&chain).await.expect("constrói");

        assert_eq!(graph.len(), 1);
        assert_eq!(evidence_nodes(&graph).count(), 1);
        assert_eq!(graph.edges().count(), 0);

        let node = graph.node(&node_id_for(0)).expect("o nó do índice 0 existe");
        assert_eq!(node.node_type, EVIDENCE_NODE_TYPE);
        assert_eq!(node.timestamp, 1);
        assert!(node.signer.is_none());
        assert!(node.invariants.is_empty());

        assert_eq!(verify_against_chain(&graph, &chain).await, Ok(()));
    }

    #[tokio::test]
    async fn n_records_yield_n_nodes_and_n_minus_one_edges() {
        let chain = chain_of(5).await;
        let graph = build_from_evidence_chain(&chain).await.expect("constrói");

        assert_eq!(evidence_nodes(&graph).count(), 5);
        assert_eq!(graph.edges().count(), 4);
        assert_eq!(graph.len(), 9);

        // A cadeia wormgraph em si é íntegra, independentemente da ponte.
        assert_eq!(graph.verify_chain(), Ok(()));
        assert_eq!(verify_against_chain(&graph, &chain).await, Ok(()));
    }

    #[tokio::test]
    async fn every_node_carries_the_record_hashes_and_timestamp() {
        let chain = chain_of(3).await;
        let graph = build_from_evidence_chain(&chain).await.expect("constrói");

        for index in 0..3u64 {
            let record = chain.record_at(index).await.expect("o registro existe");
            let node = graph.node(&node_id_for(index)).expect("o nó existe");

            assert_eq!(node.timestamp, record.timestamp);
            assert_eq!(node.payload["index"], index);
            assert_eq!(node.payload["hash"], hash_to_hex(&record.hash));
            assert_eq!(node.payload["prev_hash"], hash_to_hex(&record.prev_hash));
            assert_eq!(node.payload["payload_len"], record.payload.len());
        }

        // O encadeamento preservado é o da cadeia de evidências, não o do
        // wormgraph: o `prev_hash` de um nó é o `hash` do nó anterior.
        let first = graph.node(&node_id_for(0)).expect("nó 0");
        assert_eq!(
            first.payload["prev_hash"],
            hash_to_hex(&GENESIS_HASH),
            "o primeiro nó preserva a sentinela de gênese do arkhe-evidence"
        );
        for index in 1..3u64 {
            let previous = graph.node(&node_id_for(index - 1)).expect("anterior");
            let current = graph.node(&node_id_for(index)).expect("atual");
            assert_eq!(previous.payload["hash"], current.payload["prev_hash"]);
        }
    }

    #[tokio::test]
    async fn the_edges_are_chained_between_consecutive_records() {
        let chain = chain_of(4).await;
        let graph = build_from_evidence_chain(&chain).await.expect("constrói");

        let edges: Vec<&Edge> = graph.edges().collect();
        assert_eq!(edges.len(), 3);
        for (position, edge) in edges.iter().enumerate() {
            let from = position as u64;
            assert_eq!(edge.from, node_id_for(from));
            assert_eq!(edge.to, node_id_for(from + 1));
            assert_eq!(edge.edge_type, EVIDENCE_EDGE_TYPE);
            // O carimbo da aresta é o do registro de destino.
            assert_eq!(edge.timestamp, from + 2);
        }
    }

    #[tokio::test]
    async fn the_graph_survives_a_json_round_trip_and_still_crosses() {
        let chain = chain_of(3).await;
        let graph = build_from_evidence_chain(&chain).await.expect("constrói");

        let json = serde_json::to_string(&graph).expect("serializa");
        let restored: WormGraph = serde_json::from_str(&json).expect("desserializa");

        assert_eq!(restored, graph);
        assert_eq!(verify_against_chain(&restored, &chain).await, Ok(()));
    }

    #[tokio::test]
    async fn a_graph_does_not_cross_against_a_chain_of_a_different_length() {
        let graph = build_from_evidence_chain(&chain_of(3).await)
            .await
            .expect("constrói");

        assert_eq!(
            verify_against_chain(&graph, &chain_of(1).await).await,
            Err(EvidenceBridgeError::RecordCountMismatch {
                graph_nodes: 3,
                chain_records: 1
            })
        );
        assert_eq!(
            verify_against_chain(&graph, &EvidenceChain::new()).await,
            Err(EvidenceBridgeError::RecordCountMismatch {
                graph_nodes: 3,
                chain_records: 0
            })
        );
    }

    // --- detecção de adulteração -----------------------------------------

    #[tokio::test]
    async fn tampering_a_graph_entry_fails_the_crossing() {
        let chain = chain_of(3).await;
        let graph = build_from_evidence_chain(&chain).await.expect("constrói");

        // Reescreve o `hash` preservado no nó do índice 1, sem tocar no
        // restante da entrada.
        let mut entries = graph.entries().to_vec();
        let mut payload = entries[1].entry.as_node().expect("é um nó").payload.clone();
        payload["hash"] = serde_json::Value::String(hash_to_hex(&[0xAB; 32]));
        entries[1].entry = Entry::Node(
            Node::new(node_id_for(1), EVIDENCE_NODE_TYPE, 2).with_payload(payload),
        );
        let tampered = hand_built(entries);

        assert!(matches!(
            verify_against_chain(&tampered, &chain).await,
            Err(EvidenceBridgeError::GraphNotIntact { .. })
        ));
    }

    #[tokio::test]
    async fn tampering_a_graph_timestamp_fails_the_crossing() {
        let chain = chain_of(2).await;
        let graph = build_from_evidence_chain(&chain).await.expect("constrói");

        let mut entries = graph.entries().to_vec();
        if let Entry::Node(node) = &mut entries[1].entry {
            node.timestamp = 999_999;
        }
        let tampered = hand_built(entries);

        assert!(matches!(
            verify_against_chain(&tampered, &chain).await,
            Err(EvidenceBridgeError::GraphNotIntact { .. })
        ));
    }

    #[tokio::test]
    async fn removing_a_graph_entry_fails_the_crossing() {
        let chain = chain_of(3).await;
        let graph = build_from_evidence_chain(&chain).await.expect("constrói");

        let mut entries = graph.entries().to_vec();
        entries.remove(1);
        let tampered = hand_built(entries);

        assert!(matches!(
            verify_against_chain(&tampered, &chain).await,
            Err(EvidenceBridgeError::GraphNotIntact { .. })
        ));
    }

    #[tokio::test]
    async fn a_graph_that_is_intact_but_represents_another_chain_fails_the_crossing() {
        // O caso que só o cruzamento pega: o grafo verifica consigo mesmo, a
        // cadeia verifica consigo mesma, e ainda assim o grafo é de outra
        // cadeia. Aqui os hashes do `arkhe-evidence` coincidem (o hash não
        // cobre `timestamp`), então a divergência que sobra é o carimbo.
        let original = chain_of(3).await;
        let graph = build_from_evidence_chain(&original).await.expect("constrói");
        let other = chain_with_shifted_timestamps().await;

        assert_eq!(graph.verify_chain(), Ok(()), "o grafo está íntegro");
        assert!(other.verify_chain().await.is_ok(), "a outra cadeia está íntegra");

        assert_eq!(
            verify_against_chain(&graph, &other).await,
            Err(EvidenceBridgeError::TimestampMismatch { index: 0 }),
            "carimbo divergente é detectado mesmo com os hashes coincidindo"
        );
    }

    #[tokio::test]
    async fn a_graph_of_another_payload_set_fails_on_the_hash() {
        let graph = build_from_evidence_chain(&chain_of(2).await)
            .await
            .expect("constrói");

        let other = EvidenceChain::new();
        other.append(b"payload-diferente".to_vec(), 1).await;
        other.append(b"registro-1".to_vec(), 2).await;

        assert_eq!(
            verify_against_chain(&graph, &other).await,
            Err(EvidenceBridgeError::HashMismatch { index: 0 })
        );
    }

    #[tokio::test]
    async fn a_hand_built_graph_whose_node_is_not_a_record_is_caught() {
        // Monta entradas com hashes wormgraph **coerentes** (via o
        // `chain_hash` público) mas com um `node_type` que não é o de um
        // registro. O `verify_chain` do grafo passa — é o cruzamento que
        // distingue, e ele o faz pela contagem: o nó estranho não conta como
        // nó de evidência, então o subgrafo fica com zero nós para um registro.
        let chain = chain_of(1).await;
        let node = Node::new(node_id_for(0), "artifact", 1).with_payload(
            serde_json::json!({ "index": 0, "hash": "00", "prev_hash": "00", "payload_len": 0 }),
        );
        let entry = Entry::Node(node);
        let hash =
            WormGraph::chain_hash(0, &GENESIS_HASH, &entry).expect("o hash do wormgraph computa");

        let graph = hand_built(vec![WormEntry {
            sequence: 0,
            prev_hash: GENESIS_HASH,
            hash,
            entry,
        }]);

        assert_eq!(graph.verify_chain(), Ok(()));
        assert_eq!(
            verify_against_chain(&graph, &chain).await,
            Err(EvidenceBridgeError::RecordCountMismatch {
                graph_nodes: 0,
                chain_records: 1
            })
        );
    }

    #[tokio::test]
    async fn a_foreign_entry_beside_the_evidence_subgraph_is_ignored() {
        // Um grafo misto — o subgrafo de evidências íntegro e um nó de outro
        // tipo anexado depois. O wormgraph admite isso, então o cruzamento não
        // deve recusar o grafo por causa do que não é evidência.
        let chain = chain_of(2).await;
        let mut graph = build_from_evidence_chain(&chain).await.expect("constrói");
        graph
            .append_node(Node::new("nota/interna", "artifact", 5))
            .expect("o wormgraph aceita um nó de outro tipo");

        assert_eq!(graph.verify_chain(), Ok(()));
        assert_eq!(evidence_nodes(&graph).count(), 2);
        assert_eq!(verify_against_chain(&graph, &chain).await, Ok(()));
    }

    #[tokio::test]
    async fn a_hand_built_graph_with_a_malformed_payload_is_caught() {
        let chain = chain_of(1).await;
        let node = Node::new(node_id_for(0), EVIDENCE_NODE_TYPE, 1)
            .with_payload(serde_json::json!({ "index": 0 }));
        let entry = Entry::Node(node);
        let hash =
            WormGraph::chain_hash(0, &GENESIS_HASH, &entry).expect("o hash do wormgraph computa");

        let graph = hand_built(vec![WormEntry {
            sequence: 0,
            prev_hash: GENESIS_HASH,
            hash,
            entry,
        }]);

        assert_eq!(graph.verify_chain(), Ok(()));
        assert!(matches!(
            verify_against_chain(&graph, &chain).await,
            Err(EvidenceBridgeError::MalformedPayload { index: 0, .. })
        ));
    }

    #[tokio::test]
    async fn a_node_declaring_the_wrong_index_is_caught() {
        let chain = chain_of(1).await;
        let node = Node::new(node_id_for(0), EVIDENCE_NODE_TYPE, 1).with_payload(
            serde_json::json!({ "index": 7, "hash": "00", "prev_hash": "00", "payload_len": 0 }),
        );
        let entry = Entry::Node(node);
        let hash =
            WormGraph::chain_hash(0, &GENESIS_HASH, &entry).expect("o hash do wormgraph computa");

        let graph = hand_built(vec![WormEntry {
            sequence: 0,
            prev_hash: GENESIS_HASH,
            hash,
            entry,
        }]);

        assert_eq!(
            verify_against_chain(&graph, &chain).await,
            Err(EvidenceBridgeError::MisplacedIndex {
                position: 0,
                index: 7
            })
        );
    }

    #[tokio::test]
    async fn a_graph_missing_an_edge_is_caught() {
        let chain = chain_of(3).await;
        let graph = build_from_evidence_chain(&chain).await.expect("constrói");

        // Remove a última aresta e reencadeia o wormgraph a partir dela, para
        // que o grafo continue internamente íntegro.
        let mut entries = graph.entries().to_vec();
        entries.remove(2);
        let mut rebuilt = Vec::with_capacity(entries.len());
        let mut prev_hash = GENESIS_HASH;
        for entry in entries {
            let sequence = rebuilt.len() as u64;
            let hash = WormGraph::chain_hash(sequence, &prev_hash, &entry.entry)
                .expect("o hash do wormgraph computa");
            rebuilt.push(WormEntry {
                sequence,
                prev_hash,
                hash,
                entry: entry.entry,
            });
            prev_hash = hash;
        }
        let tampered = hand_built(rebuilt);

        assert_eq!(tampered.verify_chain(), Ok(()), "o grafo está internamente íntegro");
        assert_eq!(
            verify_against_chain(&tampered, &chain).await,
            Err(EvidenceBridgeError::EdgeCountMismatch {
                graph_edges: 1,
                expected_edges: 2
            })
        );
    }

    #[tokio::test]
    async fn a_graph_with_a_rewired_edge_is_caught() {
        let chain = chain_of(3).await;
        let graph = build_from_evidence_chain(&chain).await.expect("constrói");

        // Troca a última aresta por uma que salta do 0 para o 2.
        let mut entries = graph.entries().to_vec();
        entries.pop().expect("a última entrada é a aresta");
        let sequence = entries.len() as u64;
        let prev_hash = entries.last().map_or(GENESIS_HASH, |entry| entry.hash);
        let rewired = Entry::Edge(Edge::new(
            node_id_for(0),
            node_id_for(2),
            EVIDENCE_EDGE_TYPE,
            3,
        ));
        let hash = WormGraph::chain_hash(sequence, &prev_hash, &rewired)
            .expect("o hash do wormgraph computa");
        entries.push(WormEntry {
            sequence,
            prev_hash,
            hash,
            entry: rewired,
        });
        let tampered = hand_built(entries);

        assert_eq!(tampered.verify_chain(), Ok(()));
        assert_eq!(
            verify_against_chain(&tampered, &chain).await,
            Err(EvidenceBridgeError::EdgeMismatch {
                position: 1,
                from: 1,
                to: 2,
                found_from: 0,
                found_to: 2
            })
        );
    }

    #[tokio::test]
    async fn the_original_chain_is_untouched_by_the_bridge() {
        let chain = chain_of(3).await;
        let before = chain.latest_hash().await;
        let _graph = build_from_evidence_chain(&chain).await.expect("constrói");

        assert_eq!(chain.len().await, 3);
        assert_eq!(chain.latest_hash().await, before);
        assert!(chain.verify_chain().await.is_ok());
    }

    #[test]
    fn node_ids_are_unique_per_index() {
        assert_eq!(node_id_for(0), "evidence/0");
        assert_eq!(node_id_for(12), "evidence/12");
        assert_ne!(node_id_for(1), node_id_for(11));
        assert_eq!(index_of_node_id("evidence/7"), Some(7));
        assert_eq!(index_of_node_id("artifact/7"), None);
        assert_eq!(index_of_node_id("evidence/sete"), None);
    }

    #[tokio::test]
    async fn an_edge_pointing_outside_the_evidence_subgraph_is_caught() {
        let chain = chain_of(2).await;
        let graph = build_from_evidence_chain(&chain).await.expect("constrói");

        // Reescreve a aresta para apontar a um nó que não é um registro,
        // recalculando o hash para o grafo continuar íntegro consigo mesmo.
        let mut entries = graph.entries().to_vec();
        entries.pop().expect("a última entrada é a aresta");
        let sequence = entries.len() as u64;
        let prev_hash = entries.last().map_or(GENESIS_HASH, |entry| entry.hash);
        let rewired = Entry::Edge(Edge::new(node_id_for(0), "artifact/x", EVIDENCE_EDGE_TYPE, 2));
        let hash = WormGraph::chain_hash(sequence, &prev_hash, &rewired)
            .expect("o hash do wormgraph computa");
        entries.push(WormEntry {
            sequence,
            prev_hash,
            hash,
            entry: rewired,
        });
        let tampered = hand_built(entries);

        assert_eq!(tampered.verify_chain(), Ok(()));
        assert_eq!(
            verify_against_chain(&tampered, &chain).await,
            Err(EvidenceBridgeError::EdgeEndpointNotARecord {
                position: 0,
                endpoint: "artifact/x".to_string()
            })
        );
    }
}
