//! FI-124 — the typed memory graph: stored memory entries as nodes, the
//! working/episodic associations between them as weighted edges.

use crate::graph::TypedGraph;
use arkhe_core::hash::{blake3_hash, hash_to_hex};
use arkhe_core::{AgentMemory, MemoryLayer};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A node of the memory graph: one entry that is actually stored in the
/// agent's memory.
///
/// Built by looking the key up, so [`layer`](MemoryNode::layer) and
/// [`score`](MemoryNode::score) are the values memory returned rather than
/// what a caller assumed — a node cannot claim a layer the store disagrees
/// with.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MemoryNode {
    /// Stable identifier derived from the key by
    /// [`key_node_id`] — equal keys yield equal ids, so the same entry
    /// referenced by two links is one node.
    pub node_id: String,

    /// The memory key, as stored.
    pub key: String,

    /// Which layer of memory the entry lives in.
    pub layer: MemoryLayer,

    /// The entry's relevance score, as stored.
    pub score: f32,
}

impl MemoryNode {
    /// Whether this node is the entry stored under `key`.
    pub fn is_for_key(&self, key: &str) -> bool {
        self.key == key
    }
}

/// A directed, weighted association between two memory nodes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MemoryEdge {
    /// [`MemoryNode::node_id`] the association starts at — the working
    /// entry, for the links `AgiCoordinator` records.
    pub from: String,

    /// [`MemoryNode::node_id`] the association points at — the episodic
    /// entry, for the links `AgiCoordinator` records.
    pub to: String,

    /// Association strength, as supplied by the caller. Carried through
    /// unchanged; this crate does not interpret it.
    pub weight: f32,
}

/// Why a memory graph could not be built.
#[derive(Debug, Error, PartialEq)]
pub enum MemoryGraphError {
    /// A link names a key that is not in memory. Rejected rather than
    /// skipped: a graph silently missing an edge would report a smaller
    /// structure than the caller recorded, which is worse than an error
    /// the caller can see.
    #[error("no memory entry is stored under key '{key}'")]
    UnknownKey {
        /// The key that was looked up and not found.
        key: String,
    },

    /// A link carries a weight that is not a finite number. `NaN` and
    /// infinities cannot be compared or ranked, and would propagate into
    /// anything that later sorts by weight.
    #[error("link weight {weight} is not a finite number")]
    InvalidWeight {
        /// The offending weight.
        weight: f32,
    },
}

/// The node id for a memory `key`: BLAKE3 of the key's UTF-8 bytes, hex.
///
/// Deterministic and collision-resistant, so a caller — or a second build
/// of the same graph — can recompute a node's identity from the key alone
/// without reading the graph.
///
/// ```
/// use arkhe_geometric_verifier::memory_graph::key_node_id;
///
/// assert_eq!(key_node_id("session:turn-1"), key_node_id("session:turn-1"));
/// assert_ne!(key_node_id("session:turn-1"), key_node_id("session:turn-2"));
/// assert_eq!(key_node_id("session:turn-1").len(), 64);
/// ```
pub fn key_node_id(key: &str) -> String {
    hash_to_hex(&blake3_hash(key.as_bytes()))
}

/// Builds the memory graph from the stored entries behind `links`.
///
/// `links` is `(from_key, to_key, weight)` triples — exactly the shape
/// `AgiCoordinator` records per turn as
/// `(working_key, episodic_key, weight)`. Each endpoint is looked up in
/// `memory`; a link naming a key that is not stored fails the whole build
/// with [`MemoryGraphError::UnknownKey`], and a non-finite weight fails it
/// with [`MemoryGraphError::InvalidWeight`].
///
/// Nodes are deduplicated by key, so the keys of `n` links produce between
/// 2 and `2n` nodes depending on how much they overlap, and always `n`
/// edges — one per link, in the order given.
///
/// ```
/// # async fn example() {
/// use arkhe_core::{AgentMemory, InMemoryAgentMemory, MemoryEntry, MemoryLayer};
/// use arkhe_geometric_verifier::memory_graph::build_from_memory;
/// use chrono::Utc;
/// # let memory = InMemoryAgentMemory::new();
/// # memory.store(MemoryEntry { key: "w".into(), value: "User: hi".into(), score: 0.9, layer: MemoryLayer::Working, timestamp: Utc::now() }).await.unwrap();
/// # memory.store(MemoryEntry { key: "e".into(), value: "Query: hi".into(), score: 0.7, layer: MemoryLayer::Episodic, timestamp: Utc::now() }).await.unwrap();
/// # let links = vec![("w".to_string(), "e".to_string(), 1.0f32)];
/// let graph = build_from_memory(&memory, &links).await.unwrap();
///
/// assert_eq!(graph.node_count(), 2);
/// assert_eq!(graph.edges().len(), 1);
/// assert_eq!(graph.edges()[0].weight, 1.0);
/// # }
/// ```
pub async fn build_from_memory(
    memory: &dyn AgentMemory,
    links: &[(String, String, f32)],
) -> Result<TypedGraph<MemoryNode, MemoryEdge>, MemoryGraphError> {
    let mut graph: TypedGraph<MemoryNode, MemoryEdge> = TypedGraph::new();

    for (from_key, to_key, weight) in links {
        if !weight.is_finite() {
            return Err(MemoryGraphError::InvalidWeight { weight: *weight });
        }

        let from = ensure_node(&mut graph, memory, from_key).await?;
        let to = ensure_node(&mut graph, memory, to_key).await?;

        graph.push_edge(MemoryEdge {
            from,
            to,
            weight: *weight,
        });
    }

    Ok(graph)
}

/// Returns the node id for `key`, pushing the node if it is not there yet.
async fn ensure_node(
    graph: &mut TypedGraph<MemoryNode, MemoryEdge>,
    memory: &dyn AgentMemory,
    key: &str,
) -> Result<String, MemoryGraphError> {
    let node_id = key_node_id(key);

    if graph.nodes().iter().any(|node| node.node_id == node_id) {
        return Ok(node_id);
    }

    let entry = memory
        .get(key)
        .await
        .ok_or_else(|| MemoryGraphError::UnknownKey {
            key: key.to_string(),
        })?;

    graph.push_node(MemoryNode {
        node_id: node_id.clone(),
        key: entry.key,
        layer: entry.layer,
        score: entry.score,
    });

    Ok(node_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use arkhe_core::{InMemoryAgentMemory, MemoryEntry};
    use chrono::Utc;

    fn entry(key: &str, layer: MemoryLayer, score: f32) -> MemoryEntry {
        MemoryEntry {
            key: key.to_string(),
            value: format!("value for {key}"),
            score,
            layer,
            timestamp: Utc::now(),
        }
    }

    async fn memory_with(entries: &[(&str, MemoryLayer, f32)]) -> InMemoryAgentMemory {
        let memory = InMemoryAgentMemory::new();
        for (key, layer, score) in entries {
            memory.store(entry(key, *layer, *score)).await.unwrap();
        }
        memory
    }

    #[tokio::test]
    async fn links_between_stored_keys_are_accepted() {
        let memory = memory_with(&[
            ("w1", MemoryLayer::Working, 0.9),
            ("e1", MemoryLayer::Episodic, 0.7),
        ])
        .await;

        let graph = build_from_memory(&memory, &[("w1".to_string(), "e1".to_string(), 1.0)])
            .await
            .unwrap();

        assert_eq!(graph.node_count(), 2);
        assert_eq!(graph.edges().len(), 1);

        let edge = &graph.edges()[0];
        assert_eq!(edge.weight, 1.0);
        assert_eq!(edge.from, key_node_id("w1"));
        assert_eq!(edge.to, key_node_id("e1"));

        // Node fields come from memory, not from the link.
        let working = graph.node(0).unwrap();
        assert_eq!(working.key, "w1");
        assert_eq!(working.layer, MemoryLayer::Working);
        assert_eq!(working.score, 0.9);
        assert!(graph.node(1).unwrap().is_for_key("e1"));
    }

    #[tokio::test]
    async fn a_link_naming_an_unstored_key_is_rejected() {
        let memory = memory_with(&[("w1", MemoryLayer::Working, 0.9)]).await;

        let err = build_from_memory(&memory, &[("w1".to_string(), "missing".to_string(), 1.0)])
            .await
            .unwrap_err();

        assert_eq!(
            err,
            MemoryGraphError::UnknownKey {
                key: "missing".to_string()
            }
        );
    }

    #[tokio::test]
    async fn the_same_key_used_in_two_links_produces_one_node() {
        let memory = memory_with(&[
            ("w1", MemoryLayer::Working, 0.9),
            ("e1", MemoryLayer::Episodic, 0.7),
            ("e2", MemoryLayer::Episodic, 0.6),
        ])
        .await;

        let graph = build_from_memory(
            &memory,
            &[
                ("w1".to_string(), "e1".to_string(), 1.0),
                ("w1".to_string(), "e2".to_string(), 0.5),
            ],
        )
        .await
        .unwrap();

        assert_eq!(graph.node_count(), 3);
        assert_eq!(graph.edges().len(), 2);
        assert_eq!(graph.edges()[1].from, key_node_id("w1"));
    }

    #[tokio::test]
    async fn a_key_that_is_both_endpoint_of_a_link_is_one_node() {
        let memory = memory_with(&[("w1", MemoryLayer::Working, 0.9)]).await;

        let graph = build_from_memory(&memory, &[("w1".to_string(), "w1".to_string(), 1.0)])
            .await
            .unwrap();

        assert_eq!(graph.node_count(), 1);
        assert_eq!(graph.edges().len(), 1);
    }

    #[test]
    fn key_node_id_is_deterministic() {
        assert_eq!(key_node_id("session:turn-1"), key_node_id("session:turn-1"));
        assert_ne!(key_node_id("session:turn-1"), key_node_id("session:turn-2"));
        assert_eq!(key_node_id(""), hash_to_hex(&blake3_hash(b"")));
    }

    #[tokio::test]
    async fn no_links_builds_an_empty_graph() {
        let memory = InMemoryAgentMemory::new();
        let graph = build_from_memory(&memory, &[]).await.unwrap();

        assert_eq!(graph.node_count(), 0);
        assert_eq!(graph.edges().len(), 0);
        assert!(graph.is_empty());
    }

    #[tokio::test]
    async fn a_non_finite_weight_is_rejected() {
        let memory = memory_with(&[("w1", MemoryLayer::Working, 0.9)]).await;

        let err = build_from_memory(&memory, &[("w1".to_string(), "w1".to_string(), f32::NAN)])
            .await
            .unwrap_err();

        assert!(
            matches!(err, MemoryGraphError::InvalidWeight { .. }),
            "{err}"
        );
    }

    #[tokio::test]
    async fn a_non_finite_weight_is_rejected_before_any_lookup() {
        // The weight check comes first, so a NaN weight reports itself
        // rather than the unknown key that also happens to be there.
        let memory = InMemoryAgentMemory::new();

        let err = build_from_memory(
            &memory,
            &[(
                "missing".to_string(),
                "also-missing".to_string(),
                f32::INFINITY,
            )],
        )
        .await
        .unwrap_err();

        assert!(
            matches!(err, MemoryGraphError::InvalidWeight { .. }),
            "{err}"
        );
    }
}
