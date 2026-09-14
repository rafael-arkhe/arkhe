//! Os tipos de dado do grafo causal: [`Node`], [`Edge`] e o envelope de
//! cadeia [`WormEntry`].
//!
//! [`Node`] e [`Edge`] são dados puros — não carregam hash nem posição. O
//! encadeamento vive em [`WormEntry`], para que um nó possa ser serializado,
//! comparado e transportado sem arrastar consigo a posição que ocupava numa
//! cadeia específica.

use arkhe_core::ArkheHash;
use serde::{Deserialize, Serialize};

/// Um nó do grafo causal.
///
/// `id` é a chave estável do nó (única dentro de um grafo); `node_type`
/// classifica o nó para efeito de consulta; `invariants` são os IDs de
/// invariante que este nó reivindica satisfazer, e é sobre eles que a consulta
/// por invariante opera.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Node {
    /// Identificador único do nó dentro do grafo.
    pub id: String,
    /// Classificação do nó (ex.: `"invariant.check"`, `"artifact"`).
    pub node_type: String,
    /// Instante de criação, em milissegundos desde a época Unix.
    pub timestamp: u64,
    /// Quem assina este nó. `None` significa nó não assinado — o campo é
    /// opcional porque nem todo nó do grafo é atestado, e inventar uma string
    /// vazia como sentinela tornaria essa distinção impossível de recuperar
    /// depois do round-trip JSON.
    pub signer: Option<String>,
    /// IDs dos invariantes a que este nó se refere.
    pub invariants: Vec<String>,
    /// Conteúdo livre do nó, como JSON.
    pub payload: serde_json::Value,
}

impl Node {
    /// Cria um nó sem signatário, sem invariantes e com `payload` nulo.
    ///
    /// Os campos opcionais são preenchidos pelos construtores encadeados
    /// [`Node::with_signer`], [`Node::with_invariant`] e [`Node::with_payload`].
    pub fn new(id: impl Into<String>, node_type: impl Into<String>, timestamp: u64) -> Self {
        Self {
            id: id.into(),
            node_type: node_type.into(),
            timestamp,
            signer: None,
            invariants: Vec::new(),
            payload: serde_json::Value::Null,
        }
    }

    /// Define o signatário deste nó.
    pub fn with_signer(mut self, signer: impl Into<String>) -> Self {
        self.signer = Some(signer.into());
        self
    }

    /// Acrescenta um ID de invariante a este nó. Chamadas sucessivas acumulam.
    pub fn with_invariant(mut self, invariant: impl Into<String>) -> Self {
        self.invariants.push(invariant.into());
        self
    }

    /// Define o conteúdo deste nó.
    pub fn with_payload(mut self, payload: serde_json::Value) -> Self {
        self.payload = payload;
        self
    }

    /// `true` se este nó declara o invariante `invariant`.
    pub fn references_invariant(&self, invariant: &str) -> bool {
        self.invariants.iter().any(|known| known == invariant)
    }
}

/// Uma aresta dirigida do grafo causal, de [`Edge::from`] para [`Edge::to`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Edge {
    /// `id` do nó de origem.
    pub from: String,
    /// `id` do nó de destino.
    pub to: String,
    /// Classificação da aresta (ex.: `"depends_on"`, `"supersedes"`).
    pub edge_type: String,
    /// Instante de criação, em milissegundos desde a época Unix.
    pub timestamp: u64,
}

impl Edge {
    /// Cria uma aresta entre dois `id` de nó.
    pub fn new(
        from: impl Into<String>,
        to: impl Into<String>,
        edge_type: impl Into<String>,
        timestamp: u64,
    ) -> Self {
        Self {
            from: from.into(),
            to: to.into(),
            edge_type: edge_type.into(),
            timestamp,
        }
    }
}

/// O que uma entrada da cadeia carrega: um nó ou uma aresta.
///
/// Nós e arestas compartilham uma única cadeia append-only, em vez de duas
/// cadeias separadas, porque a ordem causal entre um nó e a aresta que o
/// liga a outro é exatamente a informação que a cadeia precisa preservar.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Entry {
    /// Uma entrada de nó.
    Node(Node),
    /// Uma entrada de aresta.
    Edge(Edge),
}

impl Entry {
    /// Rótulo estável do tipo de entrada: `"node"` ou `"edge"`.
    pub fn kind(&self) -> &'static str {
        match self {
            Entry::Node(_) => "node",
            Entry::Edge(_) => "edge",
        }
    }

    /// Devolve o nó desta entrada, se ela for de nó.
    pub fn as_node(&self) -> Option<&Node> {
        match self {
            Entry::Node(node) => Some(node),
            Entry::Edge(_) => None,
        }
    }

    /// Devolve a aresta desta entrada, se ela for de aresta.
    pub fn as_edge(&self) -> Option<&Edge> {
        match self {
            Entry::Edge(edge) => Some(edge),
            Entry::Node(_) => None,
        }
    }
}

/// Uma entrada da cadeia: o conteúdo mais a sua posição e a sua ligação ao
/// predecessor.
///
/// Os campos são públicos para que a cadeia seja legível e consumível por
/// máquina, mas ela **não** é mutável por esta API — entradas só nascem de
/// [`crate::WormGraph::append_node`] / [`crate::WormGraph::append_edge`], e
/// qualquer edição de um campo invalida o hash, o que
/// [`crate::WormGraph::verify_chain`] detecta.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WormEntry {
    /// Posição na cadeia, começando em zero.
    pub sequence: u64,
    /// Hash da entrada anterior, ou [`crate::GENESIS_HASH`] para a raiz.
    pub prev_hash: ArkheHash,
    /// Hash sobre todos os outros campos desta entrada.
    pub hash: ArkheHash,
    /// O nó ou a aresta anexada.
    pub entry: Entry,
}

impl WormEntry {
    /// Devolve o nó desta entrada, se ela for de nó.
    pub fn as_node(&self) -> Option<&Node> {
        match &self.entry {
            Entry::Node(node) => Some(node),
            Entry::Edge(_) => None,
        }
    }

    /// Devolve a aresta desta entrada, se ela for de aresta.
    pub fn as_edge(&self) -> Option<&Edge> {
        match &self.entry {
            Entry::Edge(edge) => Some(edge),
            Entry::Node(_) => None,
        }
    }

    /// Rótulo estável do tipo de entrada: `"node"` ou `"edge"`.
    pub fn kind(&self) -> &'static str {
        self.entry.kind()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_round_trips_through_json() {
        let node = Node::new("alpha", "invariant.check", 1_700_000_000_000)
            .with_signer("witness-a")
            .with_invariant("INV-009")
            .with_invariant("INV-011")
            .with_payload(serde_json::json!({ "verdict": "pass", "score": 1 }));

        let json = serde_json::to_string(&node).expect("serializa");
        let restored: Node = serde_json::from_str(&json).expect("desserializa");

        assert_eq!(restored, node);
        assert_eq!(restored.id, "alpha");
        assert_eq!(restored.node_type, "invariant.check");
        assert_eq!(restored.timestamp, 1_700_000_000_000);
        assert_eq!(restored.signer.as_deref(), Some("witness-a"));
        assert_eq!(restored.invariants, vec!["INV-009", "INV-011"]);
        assert_eq!(restored.payload["verdict"], "pass");
    }

    #[test]
    fn node_fields_are_public_in_the_json_shape() {
        let node = Node::new("alpha", "artifact", 5);
        let value = serde_json::to_value(&node).expect("para Value");
        for field in [
            "id",
            "node_type",
            "timestamp",
            "signer",
            "invariants",
            "payload",
        ] {
            assert!(value.get(field).is_some(), "campo `{field}` ausente");
        }
        // Sem signatário, o campo é `null` — não uma string vazia.
        assert!(value["signer"].is_null());
    }

    #[test]
    fn edge_round_trips_through_json() {
        let edge = Edge::new("alpha", "beta", "depends_on", 1_700_000_000_001);

        let json = serde_json::to_string(&edge).expect("serializa");
        let restored: Edge = serde_json::from_str(&json).expect("desserializa");

        assert_eq!(restored, edge);
        assert_eq!(restored.from, "alpha");
        assert_eq!(restored.to, "beta");
        assert_eq!(restored.edge_type, "depends_on");
        assert_eq!(restored.timestamp, 1_700_000_000_001);
    }

    #[test]
    fn entry_discriminates_node_from_edge() {
        let node = Entry::Node(Node::new("a", "artifact", 0));
        let edge = Entry::Edge(Edge::new("a", "b", "depends_on", 0));

        assert_eq!(node.kind(), "node");
        assert_eq!(edge.kind(), "edge");
        assert!(node.as_node().is_some());
        assert!(node.as_edge().is_none());
        assert!(edge.as_edge().is_some());
        assert!(edge.as_node().is_none());
    }

    #[test]
    fn references_invariant_matches_exactly() {
        let node = Node::new("a", "artifact", 0).with_invariant("INV-009");
        assert!(node.references_invariant("INV-009"));
        assert!(!node.references_invariant("INV-00"));
        assert!(!node.references_invariant("INV-0099"));
        assert!(!node.references_invariant(""));
    }
}
