//! O grafo causal append-only e a sua cadeia de hashes.

use arkhe_core::hash::blake3_hash;
use arkhe_core::ArkheHash;
use serde::{Deserialize, Serialize};

use crate::error::WormGraphError;
use crate::types::{Edge, Entry, Node, WormEntry};

/// O `prev_hash` da primeira entrada de uma cadeia — sentinela de zeros.
///
/// Não é uma saída real do BLAKE3 (BLAKE3 nunca produz o digest todo-zero
/// para uma entrada real, com probabilidade esmagadora), mas isto é uma
/// convenção fixa de formatação, **não** uma propriedade de segurança em que
/// o código se apoie.
pub const GENESIS_HASH: ArkheHash = [0u8; 32];

/// Separador de domínio do wormgraph.
///
/// Impede que um hash calculado aqui seja confundido com um hash da mesma
/// forma calculado por outro protocolo (ou por outra versão deste): mudar o
/// domínio muda todos os hashes, então o formato da cadeia é versionado.
const ENTRY_DOMAIN: &[u8] = b"arkhe-wormgraph/entry/v1";

/// Tag de tipo de entrada, para que nós e arestas nunca colidam.
const TAG_NODE: u8 = 0x00;
/// Tag de tipo de entrada, para que nós e arestas nunca colidam.
const TAG_EDGE: u8 = 0x01;

/// Anexa uma string com prefixo de comprimento.
///
/// Sem o prefixo, `("ab", "c")` e `("a", "bc")` produziriam os mesmos bytes e
/// portanto o mesmo hash — dois nós distintos que a cadeia não conseguiria
/// separar.
fn put_str(buf: &mut Vec<u8>, value: &str) {
    buf.extend_from_slice(&(value.len() as u64).to_be_bytes());
    buf.extend_from_slice(value.as_bytes());
}

/// Filtro conjuntivo para [`WormGraph::query_nodes`].
///
/// Um campo em `None` não restringe; campos preenchidos são combinados com
/// **E** lógico.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NodeFilter {
    /// Restringe a nós deste `node_type`.
    pub node_type: Option<String>,
    /// Restringe a nós assinados exatamente por este signatário.
    pub signer: Option<String>,
    /// Restringe a nós que declaram este ID de invariante.
    pub invariant: Option<String>,
}

impl NodeFilter {
    /// Um filtro sem restrições (casa com todo nó).
    pub fn new() -> Self {
        Self::default()
    }

    /// Restringe pelo tipo do nó.
    pub fn of_type(mut self, node_type: impl Into<String>) -> Self {
        self.node_type = Some(node_type.into());
        self
    }

    /// Restringe pelo signatário.
    pub fn by_signer(mut self, signer: impl Into<String>) -> Self {
        self.signer = Some(signer.into());
        self
    }

    /// Restringe por invariante referenciado.
    pub fn with_invariant(mut self, invariant: impl Into<String>) -> Self {
        self.invariant = Some(invariant.into());
        self
    }

    /// `true` se `node` satisfaz todas as restrições preenchidas.
    pub fn matches(&self, node: &Node) -> bool {
        if let Some(node_type) = &self.node_type {
            if &node.node_type != node_type {
                return false;
            }
        }
        if let Some(signer) = &self.signer {
            if node.signer.as_deref() != Some(signer.as_str()) {
                return false;
            }
        }
        if let Some(invariant) = &self.invariant {
            if !node.references_invariant(invariant) {
                return false;
            }
        }
        true
    }
}

/// Um grafo causal append-only, com hash encadeado.
///
/// Não há remoção nem edição: [`WormGraph::append_node`] e
/// [`WormGraph::append_edge`] são as únicas formas de acrescentar entradas,
/// cada entrada compromete-se ao hash da anterior via `prev_hash`, e
/// [`WormGraph::verify_chain`] recomputa a cadeia inteira.
///
/// O campo `entries` é privado de propósito: não existe `&mut` para o vetor
/// de entradas. Como em `arkhe-governance`, [`WormGraph::from_entries`] e
/// `Deserialize` constroem sem validar — a integridade de uma cadeia é
/// estabelecida por verificação, não pelo construtor, e é justamente isso que
/// torna a adulteração *detectável* em vez de apenas impossível de escrever
/// por esta API.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct WormGraph {
    entries: Vec<WormEntry>,
}

impl WormGraph {
    /// Cria um grafo vazio.
    pub fn new() -> Self {
        Self::default()
    }

    /// Reconstroi um grafo a partir de entradas já existentes.
    ///
    /// Não valida nada — chame [`WormGraph::verify_chain`] em seguida. Ver a
    /// nota de integridade em [`WormGraph`].
    pub fn from_entries(entries: Vec<WormEntry>) -> Self {
        Self { entries }
    }

    /// Anexa um nó e devolve o hash da nova entrada.
    ///
    /// Rejeita um `id` já presente com
    /// [`WormGraphError::DuplicateNode`].
    pub fn append_node(&mut self, node: Node) -> Result<ArkheHash, WormGraphError> {
        if self.has_node(&node.id) {
            return Err(WormGraphError::DuplicateNode(node.id));
        }
        self.append(Entry::Node(node))
    }

    /// Anexa uma aresta e devolve o hash da nova entrada.
    ///
    /// Ambas as pontas precisam existir como nós já anexados, senão
    /// [`WormGraphError::UnknownNode`]: numa cadeia causal uma aresta que
    /// aponta para um nó inexistente é uma referência pendente, e a cadeia é
    /// append-only — não haveria como consertá-la depois.
    pub fn append_edge(&mut self, edge: Edge) -> Result<ArkheHash, WormGraphError> {
        for endpoint in [&edge.from, &edge.to] {
            if !self.has_node(endpoint) {
                return Err(WormGraphError::UnknownNode(endpoint.clone()));
            }
        }
        self.append(Entry::Edge(edge))
    }

    /// Anexa uma entrada e devolve o seu hash.
    ///
    /// Se o hash não puder ser computado, nada é anexado: a operação é
    /// tudo-ou-nada, então uma falha de serialização não deixa uma entrada
    /// sem hash válido na cadeia.
    fn append(&mut self, entry: Entry) -> Result<ArkheHash, WormGraphError> {
        let sequence = self.entries.len() as u64;
        let prev_hash = self.head_hash();
        let hash = Self::chain_hash(sequence, &prev_hash, &entry)?;
        self.entries.push(WormEntry {
            sequence,
            prev_hash,
            hash,
            entry,
        });
        Ok(hash)
    }

    /// Hash de uma entrada: `BLAKE3(domínio ∥ sequence ∥ prev_hash ∥ conteúdo)`.
    ///
    /// Estável para a mesma `(sequence, prev_hash, entry)` e sensível a
    /// qualquer mudança em qualquer um deles. O conteúdo é codificado de
    /// forma canônica — strings com prefixo de comprimento, inteiros em
    /// big-endian, tag de tipo explícita — e o `payload`, por ser JSON livre,
    /// entra como JSON canônico (`serde_json` ordena as chaves de objeto, de
    /// modo que a mesma estrutura lógica produz os mesmos bytes).
    pub fn chain_hash(
        sequence: u64,
        prev_hash: &ArkheHash,
        entry: &Entry,
    ) -> Result<ArkheHash, WormGraphError> {
        let mut buf = Vec::new();
        buf.extend_from_slice(ENTRY_DOMAIN);
        buf.extend_from_slice(&sequence.to_be_bytes());
        buf.extend_from_slice(prev_hash);

        match entry {
            Entry::Node(node) => {
                buf.push(TAG_NODE);
                put_str(&mut buf, &node.id);
                put_str(&mut buf, &node.node_type);
                buf.extend_from_slice(&node.timestamp.to_be_bytes());
                match &node.signer {
                    Some(signer) => {
                        buf.push(0x01);
                        put_str(&mut buf, signer);
                    }
                    // O signatário ausente é marcado por um byte distinto do
                    // prefixo de um signatário presente, para que `None` e
                    // `Some("")` não colidam.
                    None => buf.push(0x00),
                }
                buf.extend_from_slice(&(node.invariants.len() as u64).to_be_bytes());
                for invariant in &node.invariants {
                    put_str(&mut buf, invariant);
                }
                let payload = serde_json::to_string(&node.payload)
                    .map_err(|err| WormGraphError::Serialization(err.to_string()))?;
                put_str(&mut buf, &payload);
            }
            Entry::Edge(edge) => {
                buf.push(TAG_EDGE);
                put_str(&mut buf, &edge.from);
                put_str(&mut buf, &edge.to);
                put_str(&mut buf, &edge.edge_type);
                buf.extend_from_slice(&edge.timestamp.to_be_bytes());
            }
        }

        Ok(blake3_hash(&buf))
    }

    /// Recomputa a cadeia inteira e devolve a **primeira** posição que não
    /// verifica.
    ///
    /// Um `payload` adulterado, um `node_type` trocado, um `timestamp`
    /// reescrito, uma entrada reordenada ou uma entrada removida falham aqui.
    ///
    /// A ordem das checagens é ligação, depois `sequence`, depois hash. A
    /// ligação vem primeiro porque uma remoção ou uma troca de posição deixa
    /// a entrada internamente coerente mas apontando para o predecessor
    /// errado — é isso que está quebrado, e reportá-lo como
    /// [`WormGraphError::LinkBroken`] diz o que de fato aconteceu. Checar
    /// `sequence` antes mascararia os dois casos como
    /// [`WormGraphError::SequenceMismatch`], que é o diagnóstico correto
    /// apenas quando o campo `sequence` foi forjado sem mexer na ligação.
    pub fn verify_chain(&self) -> Result<(), WormGraphError> {
        let mut expected_prev = GENESIS_HASH;
        for (position, entry) in self.entries.iter().enumerate() {
            let position = position as u64;
            if entry.prev_hash != expected_prev {
                return Err(WormGraphError::LinkBroken { index: position });
            }
            if entry.sequence != position {
                return Err(WormGraphError::SequenceMismatch { index: position });
            }
            let recomputed = Self::chain_hash(entry.sequence, &entry.prev_hash, &entry.entry)?;
            if recomputed != entry.hash {
                return Err(WormGraphError::HashMismatch { index: position });
            }
            expected_prev = entry.hash;
        }
        Ok(())
    }

    /// Todas as entradas, da mais antiga para a mais nova.
    pub fn entries(&self) -> &[WormEntry] {
        &self.entries
    }

    /// Número de entradas (nós mais arestas).
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// `true` se o grafo não tem nenhuma entrada.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Hash da entrada mais nova, ou [`GENESIS_HASH`] se o grafo estiver
    /// vazio.
    pub fn head_hash(&self) -> ArkheHash {
        self.entries.last().map_or(GENESIS_HASH, |entry| entry.hash)
    }

    /// `true` se existe um nó com este `id`.
    pub fn has_node(&self, id: &str) -> bool {
        self.nodes().any(|node| node.id == id)
    }

    /// O nó com este `id`, se existir.
    pub fn node(&self, id: &str) -> Option<&Node> {
        self.nodes().find(|node| node.id == id)
    }

    /// Itera sobre todos os nós, na ordem em que foram anexados.
    pub fn nodes(&self) -> impl Iterator<Item = &Node> + '_ {
        self.entries.iter().filter_map(WormEntry::as_node)
    }

    /// Itera sobre todas as arestas, na ordem em que foram anexadas.
    pub fn edges(&self) -> impl Iterator<Item = &Edge> + '_ {
        self.entries.iter().filter_map(WormEntry::as_edge)
    }

    /// Consulta por `node_type`. Devolve um iterador (sem alocação).
    pub fn nodes_by_type<'g, 't: 'g>(
        &'g self,
        node_type: &'t str,
    ) -> impl Iterator<Item = &'g Node> + 'g {
        self.nodes()
            .filter(move |node| node.node_type == node_type)
    }

    /// Consulta por signatário — apenas nós assinados por exatamente
    /// `signer` (um nó não assinado nunca casa).
    pub fn nodes_by_signer<'g, 't: 'g>(
        &'g self,
        signer: &'t str,
    ) -> impl Iterator<Item = &'g Node> + 'g {
        self.nodes()
            .filter(move |node| node.signer.as_deref() == Some(signer))
    }

    /// Consulta por invariante referenciado.
    pub fn nodes_by_invariant<'g, 't: 'g>(
        &'g self,
        invariant: &'t str,
    ) -> impl Iterator<Item = &'g Node> + 'g {
        self.nodes()
            .filter(move |node| node.references_invariant(invariant))
    }

    /// Consulta por `edge_type`. Devolve um iterador (sem alocação).
    pub fn edges_by_type<'g, 't: 'g>(
        &'g self,
        edge_type: &'t str,
    ) -> impl Iterator<Item = &'g Edge> + 'g {
        self.edges()
            .filter(move |edge| edge.edge_type == edge_type)
    }

    /// Arestas que saem de `from`.
    pub fn edges_from<'g, 't: 'g>(&'g self, from: &'t str) -> impl Iterator<Item = &'g Edge> + 'g {
        self.edges().filter(move |edge| edge.from == from)
    }

    /// Arestas que chegam a `to`.
    pub fn edges_to<'g, 't: 'g>(&'g self, to: &'t str) -> impl Iterator<Item = &'g Edge> + 'g {
        self.edges().filter(move |edge| edge.to == to)
    }

    /// Consulta conjuntiva por tipo, signatário e invariante.
    ///
    /// Devolve um iterador (sem alocação); ver [`NodeFilter`] para as
    /// combinações e para os métodos de conveniência de filtro único.
    pub fn query_nodes<'g>(&'g self, filter: &'g NodeFilter) -> impl Iterator<Item = &'g Node> + 'g {
        self.nodes().filter(move |node| filter.matches(node))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Grafo com dois nós e uma aresta entre eles — base da maioria dos
    /// testes de consulta e de cadeia.
    fn sample_graph() -> WormGraph {
        let mut graph = WormGraph::new();
        let alpha = Node::new("alpha", "invariant.check", 1_000)
            .with_signer("witness-a")
            .with_invariant("INV-009");
        let beta = Node::new("beta", "artifact", 2_000).with_signer("witness-b");
        graph.append_node(alpha).expect("alpha deve ser anexável");
        graph.append_node(beta).expect("beta deve ser anexável");
        graph
            .append_edge(Edge::new("alpha", "beta", "depends_on", 3_000))
            .expect("aresta deve ser anexável");
        graph
    }

    // --- cadeia / imutabilidade -------------------------------------------

    #[test]
    fn empty_graph_verifies() {
        let graph = WormGraph::new();
        assert!(graph.is_empty());
        assert_eq!(graph.head_hash(), GENESIS_HASH);
        assert_eq!(graph.verify_chain(), Ok(()));
    }

    #[test]
    fn first_entry_links_to_genesis() {
        let mut graph = WormGraph::new();
        graph
            .append_node(Node::new("alpha", "artifact", 1))
            .expect("anexável");
        let first = &graph.entries()[0];
        assert_eq!(first.sequence, 0);
        assert_eq!(first.prev_hash, GENESIS_HASH);
        assert_eq!(graph.head_hash(), first.hash);
    }

    #[test]
    fn each_entry_links_to_its_predecessor() {
        let graph = sample_graph();
        assert_eq!(graph.len(), 3);
        assert_eq!(graph.verify_chain(), Ok(()));
        assert_eq!(graph.entries()[1].prev_hash, graph.entries()[0].hash);
        assert_eq!(graph.entries()[2].prev_hash, graph.entries()[1].hash);
    }

    #[test]
    fn append_returns_the_hash_of_the_stored_entry() {
        let mut graph = WormGraph::new();
        let returned = graph
            .append_node(Node::new("alpha", "artifact", 1))
            .expect("anexável");
        assert_eq!(returned, graph.entries()[0].hash);
    }

    #[test]
    fn tampering_node_payload_is_detected() {
        let mut graph = sample_graph();
        graph.entries[0].entry = Entry::Node(
            Node::new("alpha", "invariant.check", 1_000)
                .with_signer("witness-a")
                .with_invariant("INV-009")
                .with_payload(serde_json::json!({ "verdict": "tampered" })),
        );
        assert_eq!(
            graph.verify_chain(),
            Err(WormGraphError::HashMismatch { index: 0 })
        );
    }

    #[test]
    fn tampering_the_signer_is_detected() {
        let mut graph = sample_graph();
        if let Entry::Node(node) = &mut graph.entries[1].entry {
            node.signer = Some("witness-z".to_string());
        }
        assert_eq!(
            graph.verify_chain(),
            Err(WormGraphError::HashMismatch { index: 1 })
        );
    }

    #[test]
    fn tampering_an_invariant_list_is_detected() {
        let mut graph = sample_graph();
        if let Entry::Node(node) = &mut graph.entries[0].entry {
            node.invariants.push("INV-999".to_string());
        }
        assert_eq!(
            graph.verify_chain(),
            Err(WormGraphError::HashMismatch { index: 0 })
        );
    }

    #[test]
    fn tampering_a_timestamp_is_detected() {
        let mut graph = sample_graph();
        if let Entry::Edge(edge) = &mut graph.entries[2].entry {
            edge.timestamp = 999_999;
        }
        assert_eq!(
            graph.verify_chain(),
            Err(WormGraphError::HashMismatch { index: 2 })
        );
    }

    #[test]
    fn removing_an_entry_breaks_the_link() {
        let mut graph = sample_graph();
        graph.entries.remove(1);
        // A entrada que era a terceira agora ocupa a posição 1 e aponta para
        // o hash da entrada removida, não para a nova predecessora.
        assert_eq!(
            graph.verify_chain(),
            Err(WormGraphError::LinkBroken { index: 1 })
        );
    }

    #[test]
    fn reordering_entries_breaks_the_link() {
        let mut graph = sample_graph();
        graph.entries.swap(0, 1);
        assert_eq!(
            graph.verify_chain(),
            Err(WormGraphError::LinkBroken { index: 0 })
        );
    }

    #[test]
    fn forging_the_sequence_field_is_detected() {
        let mut graph = sample_graph();
        graph.entries[2].sequence = 7;
        assert_eq!(
            graph.verify_chain(),
            Err(WormGraphError::SequenceMismatch { index: 2 })
        );
    }

    // --- consultas ---------------------------------------------------------

    #[test]
    fn query_by_node_type() {
        let graph = sample_graph();
        let checks: Vec<&str> = graph
            .nodes_by_type("invariant.check")
            .map(|node| node.id.as_str())
            .collect();
        assert_eq!(checks, vec!["alpha"]);

        let artifacts: Vec<&str> = graph
            .nodes_by_type("artifact")
            .map(|node| node.id.as_str())
            .collect();
        assert_eq!(artifacts, vec!["beta"]);
        assert_eq!(graph.nodes_by_type("não-existe").count(), 0);
    }

    #[test]
    fn query_by_edge_type() {
        let graph = sample_graph();
        let deps: Vec<&str> = graph
            .edges_by_type("depends_on")
            .map(|edge| edge.from.as_str())
            .collect();
        assert_eq!(deps, vec!["alpha"]);
        assert_eq!(graph.edges_by_type("supersedes").count(), 0);
    }

    #[test]
    fn query_by_signer() {
        let graph = sample_graph();
        let signed: Vec<&str> = graph
            .nodes_by_signer("witness-a")
            .map(|node| node.id.as_str())
            .collect();
        assert_eq!(signed, vec!["alpha"]);
        assert_eq!(graph.nodes_by_signer("ninguém").count(), 0);
    }

    #[test]
    fn query_by_signer_does_not_match_unsigned_nodes() {
        let mut graph = WormGraph::new();
        graph
            .append_node(Node::new("unsigned", "artifact", 1))
            .expect("anexável");
        assert_eq!(graph.nodes_by_signer("").count(), 0);
        assert!(graph.node("unsigned").expect("existe").signer.is_none());
    }

    #[test]
    fn query_by_invariant() {
        let graph = sample_graph();
        let inv: Vec<&str> = graph
            .nodes_by_invariant("INV-009")
            .map(|node| node.id.as_str())
            .collect();
        assert_eq!(inv, vec!["alpha"]);
        assert_eq!(graph.nodes_by_invariant("INV-000").count(), 0);
    }

    #[test]
    fn combined_filter_is_a_conjunction() {
        let graph = sample_graph();

        let both = NodeFilter::new()
            .of_type("invariant.check")
            .by_signer("witness-a")
            .with_invariant("INV-009");
        assert_eq!(graph.query_nodes(&both).count(), 1);

        // Cada restrição individualmente correta, mas incompatíveis entre si.
        let contradictory = NodeFilter::new()
            .of_type("invariant.check")
            .by_signer("witness-b");
        assert_eq!(graph.query_nodes(&contradictory).count(), 0);

        // Filtro vazio casa com todos os nós (mas não com arestas).
        assert_eq!(graph.query_nodes(&NodeFilter::new()).count(), 2);
    }

    #[test]
    fn edge_endpoints_queries() {
        let graph = sample_graph();
        assert_eq!(graph.edges_from("alpha").count(), 1);
        assert_eq!(graph.edges_to("beta").count(), 1);
        assert_eq!(graph.edges_from("beta").count(), 0);
    }

    #[test]
    fn node_lookup_by_id() {
        let graph = sample_graph();
        assert_eq!(graph.node("beta").expect("existe").node_type, "artifact");
        assert!(graph.node("gama").is_none());
        assert!(graph.has_node("alpha"));
        assert!(!graph.has_node("gama"));
    }

    // --- invariantes de integridade da inserção ---------------------------

    #[test]
    fn duplicate_node_id_is_rejected() {
        let mut graph = WormGraph::new();
        graph
            .append_node(Node::new("alpha", "artifact", 1))
            .expect("primeira inserção");
        assert_eq!(
            graph.append_node(Node::new("alpha", "artifact", 2)),
            Err(WormGraphError::DuplicateNode("alpha".to_string()))
        );
        assert_eq!(graph.len(), 1);
    }

    #[test]
    fn edge_with_unknown_endpoint_is_rejected() {
        let mut graph = WormGraph::new();
        graph
            .append_node(Node::new("alpha", "artifact", 1))
            .expect("anexável");

        assert_eq!(
            graph.append_edge(Edge::new("alpha", "fantasma", "depends_on", 2)),
            Err(WormGraphError::UnknownNode("fantasma".to_string()))
        );
        assert_eq!(
            graph.append_edge(Edge::new("fantasma", "alpha", "depends_on", 2)),
            Err(WormGraphError::UnknownNode("fantasma".to_string()))
        );
        assert_eq!(graph.len(), 1);
    }

    #[test]
    fn a_rejected_insert_does_not_touch_the_chain() {
        let mut graph = sample_graph();
        let head_before = graph.head_hash();
        let len_before = graph.len();

        assert!(graph.append_node(Node::new("alpha", "artifact", 9)).is_err());
        assert!(graph
            .append_edge(Edge::new("alpha", "fantasma", "depends_on", 9))
            .is_err());

        assert_eq!(graph.len(), len_before);
        assert_eq!(graph.head_hash(), head_before);
        assert_eq!(graph.verify_chain(), Ok(()));
    }

    // --- hash da cadeia ----------------------------------------------------

    #[test]
    fn chain_hash_is_deterministic() {
        let node = Node::new("alpha", "artifact", 1);
        let entry = Entry::Node(node);
        let a = WormGraph::chain_hash(0, &GENESIS_HASH, &entry).expect("hash");
        let b = WormGraph::chain_hash(0, &GENESIS_HASH, &entry).expect("hash");
        assert_eq!(a, b);
    }

    #[test]
    fn chain_hash_is_sensitive_to_every_input() {
        let entry = Entry::Node(Node::new("alpha", "artifact", 1));
        let base = WormGraph::chain_hash(0, &GENESIS_HASH, &entry).expect("hash");

        let other_sequence = WormGraph::chain_hash(1, &GENESIS_HASH, &entry).expect("hash");
        assert_ne!(base, other_sequence);

        let other_prev = WormGraph::chain_hash(0, &[1u8; 32], &entry).expect("hash");
        assert_ne!(base, other_prev);

        let other_entry =
            Entry::Node(Node::new("beta", "artifact", 1));
        assert_ne!(base, WormGraph::chain_hash(0, &GENESIS_HASH, &other_entry).expect("hash"));
    }

    #[test]
    fn chain_hash_separates_node_from_edge_with_the_same_payload() {
        // Nó e aresta com os mesmos bytes de string não podem colidir.
        let node = Entry::Node(Node::new("a", "b", 0));
        let edge = Entry::Edge(Edge::new("a", "b", "", 0));
        assert_ne!(
            WormGraph::chain_hash(0, &GENESIS_HASH, &node).expect("hash"),
            WormGraph::chain_hash(0, &GENESIS_HASH, &edge).expect("hash")
        );
    }

    #[test]
    fn chain_hash_length_prefixes_prevent_field_boundary_collisions() {
        let a = Entry::Node(Node::new("ab", "c", 0));
        let b = Entry::Node(Node::new("a", "bc", 0));
        assert_ne!(
            WormGraph::chain_hash(0, &GENESIS_HASH, &a).expect("hash"),
            WormGraph::chain_hash(0, &GENESIS_HASH, &b).expect("hash")
        );
    }

    // --- serialização ------------------------------------------------------

    #[test]
    fn graph_round_trips_through_json_and_still_verifies() {
        let graph = sample_graph();
        let json = serde_json::to_string(&graph).expect("serializa");
        let restored: WormGraph = serde_json::from_str(&json).expect("desserializa");
        assert_eq!(restored, graph);
        assert_eq!(restored.verify_chain(), Ok(()));
    }

    #[test]
    fn a_json_edit_that_keeps_hashes_is_caught_on_verify() {
        // Adultera o JSON de um nó sem tocar nos hashes — o round-trip
        // desserializa com sucesso, e ainda assim a verificação falha.
        let graph = sample_graph();
        let mut value = serde_json::to_value(&graph).expect("para Value");
        let tampered = value["entries"][0]["entry"]["Node"]["node_type"]
            .as_str()
            .expect("campo presente")
            .to_string();
        assert_eq!(tampered, "invariant.check");
        value["entries"][0]["entry"]["Node"]["node_type"] =
            serde_json::Value::String("artifact".to_string());

        let restored: WormGraph = serde_json::from_value(value).expect("desserializa");
        assert_eq!(
            restored.verify_chain(),
            Err(WormGraphError::HashMismatch { index: 0 })
        );
    }
}
