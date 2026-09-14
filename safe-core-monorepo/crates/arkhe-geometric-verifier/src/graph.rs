//! A minimal typed graph container: nodes and edges are whatever the caller
//! says they are, and the graph itself enforces nothing.
//!
//! Deliberately policy-free. The rules that make a graph *valid* — a
//! provenance chain must link back through `prev_hash`, a memory link must
//! name entries that exist — belong to the module that knows what the nodes
//! and edges mean ([`crate::memory_graph`], [`crate::provenance_graph`]), not
//! here. That keeps [`TypedGraph`] usable for node and edge types this crate
//! has never heard of.

use serde::{Deserialize, Serialize};

/// A graph whose node and edge payloads are strongly typed.
///
/// Nodes and edges are held in insertion order, and nodes are addressed by
/// their position — `graph.node(0)` is the first node pushed. Consumers read
/// it positionally (`node_count()`, `edges().len()`, `node(0)`), so
/// [`push_node`](TypedGraph::push_node) order is part of the contract.
///
/// ```
/// use arkhe_geometric_verifier::TypedGraph;
///
/// let mut graph: TypedGraph<&str, (&str, &str)> = TypedGraph::new();
/// graph.push_node("working");
/// graph.push_node("episodic");
/// graph.push_edge(("working", "episodic"));
///
/// assert_eq!(graph.node_count(), 2);
/// assert_eq!(graph.edge_count(), 1);
/// assert_eq!(graph.edges(), &[("working", "episodic")]);
/// assert_eq!(graph.node(2), None);
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TypedGraph<N, E> {
    nodes: Vec<N>,
    edges: Vec<E>,
}

impl<N, E> TypedGraph<N, E> {
    /// An empty graph.
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
        }
    }

    /// A graph with `nodes` and `edges` already in place, in the given
    /// order — the whole-graph equivalent of
    /// [`push_node`](TypedGraph::push_node) /
    /// [`push_edge`](TypedGraph::push_edge), for callers that assemble a
    /// graph before handing it to a validator.
    pub fn from_parts(nodes: Vec<N>, edges: Vec<E>) -> Self {
        Self { nodes, edges }
    }

    /// How many nodes the graph holds.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// How many edges the graph holds.
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// Every node, in insertion order.
    pub fn nodes(&self) -> &[N] {
        &self.nodes
    }

    /// Every edge, in insertion order.
    pub fn edges(&self) -> &[E] {
        &self.edges
    }

    /// The node at `index`, or `None` when the graph is shorter than that.
    pub fn node(&self, index: usize) -> Option<&N> {
        self.nodes.get(index)
    }

    /// The edge at `index`, or `None` when the graph has fewer edges.
    pub fn edge(&self, index: usize) -> Option<&E> {
        self.edges.get(index)
    }

    /// Appends a node, returning its index.
    pub fn push_node(&mut self, node: N) -> usize {
        self.nodes.push(node);
        self.nodes.len() - 1
    }

    /// Appends an edge, returning its index.
    pub fn push_edge(&mut self, edge: E) -> usize {
        self.edges.push(edge);
        self.edges.len() - 1
    }

    /// Whether the graph holds nothing at all.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty() && self.edges.is_empty()
    }
}

impl<N, E> Default for TypedGraph<N, E> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_graph_is_empty() {
        let graph: TypedGraph<u32, u32> = TypedGraph::new();
        assert_eq!(graph.node_count(), 0);
        assert_eq!(graph.edge_count(), 0);
        assert!(graph.is_empty());
        assert_eq!(graph.node(0), None);
        assert_eq!(graph.edge(0), None);
    }

    #[test]
    fn push_node_hands_back_the_position_it_landed_at() {
        let mut graph: TypedGraph<&str, ()> = TypedGraph::new();
        assert_eq!(graph.push_node("a"), 0);
        assert_eq!(graph.push_node("b"), 1);
        assert_eq!(graph.node(1), Some(&"b"));
        assert!(!graph.is_empty());
    }

    #[test]
    fn edges_keep_insertion_order() {
        let mut graph: TypedGraph<&str, u8> = TypedGraph::new();
        graph.push_edge(7);
        graph.push_edge(9);
        assert_eq!(graph.edges(), &[7, 9]);
        assert_eq!(graph.edge_count(), 2);
    }

    #[test]
    fn from_parts_matches_pushing_one_by_one() {
        let mut pushed: TypedGraph<&str, u8> = TypedGraph::new();
        pushed.push_node("a");
        pushed.push_node("b");
        pushed.push_edge(1);

        let built = TypedGraph::from_parts(vec!["a", "b"], vec![1]);

        assert_eq!(pushed, built);
    }

    #[test]
    fn clone_is_independent_of_the_original() {
        let mut original: TypedGraph<&str, u8> = TypedGraph::new();
        original.push_node("a");
        let mut copy = original.clone();
        copy.push_node("b");

        assert_eq!(original.node_count(), 1);
        assert_eq!(copy.node_count(), 2);
    }
}
