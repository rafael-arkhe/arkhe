//! FI-122 — the typed provenance graph: one node per evidence record, with
//! the record's hash and its link to the predecessor.

use crate::graph::TypedGraph;
use arkhe_core::ArkheHash;
use arkhe_evidence::{EvidenceChain, GENESIS_HASH};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A node of the provenance graph: one record of an
/// [`EvidenceChain`](arkhe_evidence::EvidenceChain).
///
/// The record's payload is not copied in — it already lives in the chain,
/// and duplicating it here would let the graph disagree with the chain it
/// was built from. [`payload_len`](ProvenanceNode::payload_len) is enough to
/// tell a truncated or padded payload apart when re-checking a graph
/// against its chain later.
///
/// # Two hashes, carried but not re-derived
///
/// A node carries both hashes of the record it was projected from, and they
/// mean here what they mean in `arkhe_evidence`: [`hash`](ProvenanceNode::hash)
/// is the catalog's formula (FI-011/FI-017), the linkage that makes the log a
/// chain; [`record_hash`](ProvenanceNode::record_hash) is the additive,
/// domain-separated binding over `index`, `timestamp`, `prev_hash`, and the
/// payload.
///
/// The node does **not** re-verify either one. It holds no payload (only
/// [`payload_len`](ProvenanceNode::payload_len)), so it cannot recompute
/// `record_hash`, and it keeps no predecessor beyond a hash. What carrying
/// `record_hash` buys is *completeness and cross-checking*: a caller that
/// still holds the `EvidenceChain` can match each node's `record_hash`
/// against its `EvidenceRecord::record_hash` and see a mismatched or
/// tampered projection — something the chain-less
/// [`validate`] cannot see on its own. It is not self-verification, and is
/// not sold as such.
///
/// `record_hash` deliberately does **not** carry `#[serde(default)]`. The
/// `Serialize`/`Deserialize` derive is write-only today — nothing reads a
/// `ProvenanceNode` back in — but should a payload ever be deserialised, a
/// missing field must be an explicit error rather than a silently fabricated
/// all-zero hash: explicit failure over a made-up value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvenanceNode {
    /// The record's position in the chain.
    pub index: u64,

    /// The record's own hash — the catalog formula (FI-011/FI-017),
    /// `BLAKE3(prev_hash ∥ len(payload) ∥ payload)`. This is what the
    /// `prev_hash` linkage runs over, so it is what makes the records a
    /// chain.
    pub hash: ArkheHash,

    /// The record's additive hash,
    /// `BLAKE3(domain ∥ index ∥ timestamp ∥ prev_hash ∥ len(payload) ∥
    /// payload)`, under the `arkhe-evidence/record-hash/v1` domain tag — so
    /// it is never equal to [`hash`](ProvenanceNode::hash). It binds the
    /// fields `hash` does not cover (`index`, `timestamp`). Carried from the
    /// record as-is; the node cannot recompute it, since the payload is not
    /// kept here (see the struct docs).
    pub record_hash: ArkheHash,

    /// The hash this record links back to — its predecessor's
    /// [`hash`](ProvenanceNode::hash), or [`GENESIS_HASH`] for the first
    /// record.
    pub prev_hash: ArkheHash,

    /// When the record was appended, in the chain's own units (seconds
    /// since the Unix epoch, as passed to `EvidenceChain::append`).
    pub timestamp: u64,

    /// How many bytes the record's payload holds.
    pub payload_len: usize,
}

/// The edge from a record to the record that follows it, by index.
///
/// The links that carry the tamper-evidence are the `prev_hash` fields on
/// the nodes; these edges exist so the graph has structure to traverse and
/// validate (one record follows another, no forks).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvenanceEdge {
    /// Index of the earlier record.
    pub from_index: u64,

    /// Index of the record that follows it.
    pub to_index: u64,
}

/// Why a provenance graph could not be built, or why a hand-assembled one
/// is not valid.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProvenanceError {
    /// The evidence chain itself does not verify — a payload was edited, a
    /// record was removed, or two records were swapped. Reported with the
    /// chain's own explanation rather than re-derived here.
    #[error("the evidence chain is not intact: {reason}")]
    BrokenChain {
        /// The underlying chain error, rendered.
        reason: String,
    },

    /// A node's recorded index is not the position it occupies. The index
    /// is the record's own self-reported field, which
    /// `arkhe-evidence::EvidenceChain` is explicit about not trusting
    /// after tampering — so a graph where the two disagree is rejected.
    #[error("the node at position {position} records index {index}")]
    MisplacedNode {
        /// Where the node actually sits.
        position: usize,
        /// What the node says its index is.
        index: u64,
    },

    /// A node does not link to its predecessor: `prev_hash` disagrees with
    /// the hash of the node before it.
    #[error(
        "node {index} does not chain to its predecessor: prev_hash does not match the previous node's hash"
    )]
    HashMismatch {
        /// The node whose link is broken.
        index: u64,
    },

    /// The first node does not carry the all-zero genesis `prev_hash`.
    #[error("the first node does not link to the genesis sentinel")]
    LinkBroken,

    /// An edge names a position the graph does not have.
    #[error("edge {from_index} -> {to_index} names a node outside the graph's {node_count} nodes")]
    IndexOutOfRange {
        /// The edge's source index.
        from_index: u64,
        /// The edge's target index.
        to_index: u64,
        /// How many nodes the graph actually has.
        node_count: usize,
    },

    /// A node is the target of more than one edge. A linear record chain
    /// cannot have two records claiming the same successor — that is a
    /// fork, and forks make "the record after this one" ambiguous.
    #[error("node {index} is the target of more than one edge")]
    BranchingAt {
        /// The node with more than one incoming edge.
        index: u64,
    },

    /// The edges form a cycle, which no valid chain of records can.
    #[error("the provenance graph contains a cycle")]
    CycleDetected,
}

/// Builds the provenance graph over every record in `chain`.
///
/// Fails with [`ProvenanceError::BrokenChain`] if
/// [`EvidenceChain::verify_chain`](arkhe_evidence::EvidenceChain::verify_chain)
/// rejects the chain, and never returns a graph that fails
/// [`validate`] — so a returned `Ok` means what
/// `AgiCoordinator::provenance_graph` documents: the graph is acyclic and
/// every node's `prev_hash` agrees with its predecessor's `hash`.
///
/// One node per record, in chain order, and one edge per adjacent pair: a
/// chain of `n` records yields `n` nodes and `n - 1` edges (an empty chain
/// yields an empty graph). No node stands in for the genesis sentinel, so
/// node count always equals record count.
///
/// ```no_run
/// # async fn example() {
/// use arkhe_evidence::EvidenceChain;
/// use arkhe_geometric_verifier::provenance_graph::build_from_chain;
///
/// let chain = EvidenceChain::new();
/// chain.append(b"turn one".to_vec(), 1_700_000_000).await;
/// chain.append(b"turn two".to_vec(), 1_700_000_060).await;
///
/// let graph = build_from_chain(&chain).await.unwrap();
/// assert_eq!(graph.node_count(), 2);
/// assert_eq!(graph.edges().len(), 1);
/// # }
/// ```
pub async fn build_from_chain(
    chain: &EvidenceChain,
) -> Result<TypedGraph<ProvenanceNode, ProvenanceEdge>, ProvenanceError> {
    chain
        .verify_chain()
        .await
        .map_err(|e| ProvenanceError::BrokenChain {
            reason: e.to_string(),
        })?;

    let count = chain.len().await;
    let mut graph: TypedGraph<ProvenanceNode, ProvenanceEdge> = TypedGraph::new();

    for position in 0..count {
        let record =
            chain
                .record_at(position as u64)
                .await
                .ok_or_else(|| ProvenanceError::BrokenChain {
                    reason: format!(
                        "record {position} disappeared while the graph was being built"
                    ),
                })?;

        graph.push_node(ProvenanceNode {
            index: record.index,
            hash: record.hash,
            record_hash: record.record_hash,
            prev_hash: record.prev_hash,
            timestamp: record.timestamp,
            payload_len: record.payload.len(),
        });

        if position > 0 {
            graph.push_edge(ProvenanceEdge {
                from_index: (position - 1) as u64,
                to_index: position as u64,
            });
        }
    }

    validate(&graph)?;
    Ok(graph)
}

/// Checks a provenance graph on its own terms — no chain needed.
///
/// Rejects, in this order: a node whose self-reported index is not its
/// position ([`MisplacedNode`](ProvenanceError::MisplacedNode)), a first
/// node that does not link to the genesis sentinel
/// ([`LinkBroken`](ProvenanceError::LinkBroken)), an edge naming a node that
/// is not there ([`IndexOutOfRange`](ProvenanceError::IndexOutOfRange)), a
/// node with more than one incoming edge
/// ([`BranchingAt`](ProvenanceError::BranchingAt)), a node whose
/// `prev_hash` disagrees with its predecessor's hash
/// ([`HashMismatch`](ProvenanceError::HashMismatch)), and finally a cyclic
/// edge set ([`CycleDetected`](ProvenanceError::CycleDetected)).
///
/// Public because a graph can arrive from elsewhere — deserialised, or
/// assembled by hand — and the same checks that guard
/// [`build_from_chain`] should be runnable on it.
pub fn validate(graph: &TypedGraph<ProvenanceNode, ProvenanceEdge>) -> Result<(), ProvenanceError> {
    let node_count = graph.node_count();

    for (position, node) in graph.nodes().iter().enumerate() {
        if node.index != position as u64 {
            return Err(ProvenanceError::MisplacedNode {
                position,
                index: node.index,
            });
        }
    }

    if let Some(first) = graph.nodes().first() {
        if first.prev_hash != GENESIS_HASH {
            return Err(ProvenanceError::LinkBroken);
        }
    }

    let mut incoming = vec![0usize; node_count];
    for edge in graph.edges() {
        if edge.from_index as usize >= node_count || edge.to_index as usize >= node_count {
            return Err(ProvenanceError::IndexOutOfRange {
                from_index: edge.from_index,
                to_index: edge.to_index,
                node_count,
            });
        }
        incoming[edge.to_index as usize] += 1;
    }

    for (position, count) in incoming.iter().enumerate() {
        if *count > 1 {
            return Err(ProvenanceError::BranchingAt {
                index: position as u64,
            });
        }
    }

    for position in 1..node_count {
        let node = &graph.nodes()[position];
        let predecessor = &graph.nodes()[position - 1];
        if node.prev_hash != predecessor.hash {
            return Err(ProvenanceError::HashMismatch { index: node.index });
        }
    }

    if !is_acyclic(node_count, graph.edges()) {
        return Err(ProvenanceError::CycleDetected);
    }

    Ok(())
}

/// Kahn's algorithm over the edge set. Endpoints are assumed in range —
/// [`validate`] checks that first.
fn is_acyclic(node_count: usize, edges: &[ProvenanceEdge]) -> bool {
    let mut incoming = vec![0usize; node_count];
    let mut successors: Vec<Vec<usize>> = vec![Vec::new(); node_count];

    for edge in edges {
        successors[edge.from_index as usize].push(edge.to_index as usize);
        incoming[edge.to_index as usize] += 1;
    }

    let mut ready: Vec<usize> = (0..node_count).filter(|i| incoming[*i] == 0).collect();
    let mut visited = 0;

    while let Some(node) = ready.pop() {
        visited += 1;
        for next in &successors[node] {
            incoming[*next] -= 1;
            if incoming[*next] == 0 {
                ready.push(*next);
            }
        }
    }

    visited == node_count
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A node whose hash, prev_hash, and record_hash are, for test purposes,
    /// whatever they are asked to be — used to assemble graphs the builder
    /// would never produce. `record_byte` is separate from `hash_byte` so the
    /// fixtures never conflate the two hashes.
    fn node(index: u64, hash_byte: u8, prev_byte: u8, record_byte: u8) -> ProvenanceNode {
        ProvenanceNode {
            index,
            hash: [hash_byte; 32],
            record_hash: [record_byte; 32],
            prev_hash: [prev_byte; 32],
            timestamp: index,
            payload_len: 8,
        }
    }

    /// A two-node graph with matching hashes and one edge between them.
    fn chained_pair() -> TypedGraph<ProvenanceNode, ProvenanceEdge> {
        TypedGraph::from_parts(
            vec![node(0, 1, 0, 9), node(1, 2, 1, 10)],
            vec![ProvenanceEdge {
                from_index: 0,
                to_index: 1,
            }],
        )
    }

    #[tokio::test]
    async fn a_real_chain_builds_and_validates() {
        let chain = EvidenceChain::new();
        chain.append(b"first".to_vec(), 100).await;
        chain.append(b"second".to_vec(), 200).await;
        chain.append(b"third".to_vec(), 300).await;

        let graph = build_from_chain(&chain).await.unwrap();

        assert_eq!(graph.node_count(), 3);
        assert_eq!(graph.edges().len(), 2);

        let first = chain.record_at(0).await.unwrap();
        assert_eq!(graph.node(0).unwrap().hash, first.hash);
        assert_eq!(graph.node(0).unwrap().prev_hash, GENESIS_HASH);
        assert_eq!(graph.node(0).unwrap().payload_len, b"first".len());
        assert_eq!(graph.node(0).unwrap().timestamp, 100);

        // Every node chains to the one before it.
        for position in 1..graph.node_count() {
            assert_eq!(
                graph.node(position).unwrap().prev_hash,
                graph.node(position - 1).unwrap().hash
            );
        }
    }

    #[tokio::test]
    async fn an_empty_chain_builds_an_empty_graph() {
        let graph = build_from_chain(&EvidenceChain::new()).await.unwrap();

        assert_eq!(graph.node_count(), 0);
        assert_eq!(graph.edges().len(), 0);
        assert!(graph.is_empty());
    }

    #[tokio::test]
    async fn a_single_record_has_no_edges() {
        let chain = EvidenceChain::new();
        chain.append(b"only".to_vec(), 1).await;

        let graph = build_from_chain(&chain).await.unwrap();

        assert_eq!(graph.node_count(), 1);
        assert_eq!(graph.edges().len(), 0);
    }

    #[test]
    fn a_hand_built_graph_with_mismatched_prev_hash_is_rejected() {
        // Node 1 claims predecessor hash 9, but node 0's hash is 1.
        let graph = TypedGraph::from_parts(
            vec![node(0, 1, 0, 9), node(1, 2, 9, 11)],
            vec![ProvenanceEdge {
                from_index: 0,
                to_index: 1,
            }],
        );

        assert_eq!(
            validate(&graph),
            Err(ProvenanceError::HashMismatch { index: 1 })
        );
    }

    #[test]
    fn a_first_node_not_linking_to_genesis_is_rejected() {
        let graph: TypedGraph<ProvenanceNode, ProvenanceEdge> =
            TypedGraph::from_parts(vec![node(0, 1, 7, 9)], Vec::new());

        assert_eq!(validate(&graph), Err(ProvenanceError::LinkBroken));
    }

    #[test]
    fn a_node_with_two_incoming_edges_is_rejected() {
        let graph = TypedGraph::from_parts(
            vec![node(0, 1, 0, 9), node(1, 2, 1, 10), node(2, 3, 2, 12)],
            vec![
                ProvenanceEdge {
                    from_index: 0,
                    to_index: 2,
                },
                ProvenanceEdge {
                    from_index: 1,
                    to_index: 2,
                },
            ],
        );

        assert_eq!(
            validate(&graph),
            Err(ProvenanceError::BranchingAt { index: 2 })
        );
    }

    #[test]
    fn an_edge_outside_the_graph_is_rejected() {
        let graph = TypedGraph::from_parts(
            vec![node(0, 1, 0, 9)],
            vec![ProvenanceEdge {
                from_index: 0,
                to_index: 5,
            }],
        );

        assert_eq!(
            validate(&graph),
            Err(ProvenanceError::IndexOutOfRange {
                from_index: 0,
                to_index: 5,
                node_count: 1,
            })
        );
    }

    #[test]
    fn a_misplaced_node_is_rejected() {
        let graph: TypedGraph<ProvenanceNode, ProvenanceEdge> =
            TypedGraph::from_parts(vec![node(1, 1, 0, 13)], Vec::new());

        assert_eq!(
            validate(&graph),
            Err(ProvenanceError::MisplacedNode {
                position: 0,
                index: 1,
            })
        );
    }

    #[test]
    fn a_cyclic_edge_set_is_rejected() {
        // Two nodes whose hashes chain, but whose edges point at each
        // other — a cycle that no chain of records can produce.
        let graph = TypedGraph::from_parts(
            vec![node(0, 1, 0, 9), node(1, 2, 1, 10)],
            vec![
                ProvenanceEdge {
                    from_index: 0,
                    to_index: 1,
                },
                ProvenanceEdge {
                    from_index: 1,
                    to_index: 0,
                },
            ],
        );

        assert_eq!(validate(&graph), Err(ProvenanceError::CycleDetected));
    }

    #[test]
    fn a_valid_hand_built_graph_passes() {
        assert_eq!(validate(&chained_pair()), Ok(()));
    }

    #[test]
    fn a_self_loop_is_a_cycle() {
        assert!(!is_acyclic(
            1,
            &[ProvenanceEdge {
                from_index: 0,
                to_index: 0,
            }]
        ));
    }

    #[test]
    fn a_disconnected_graph_is_acyclic() {
        assert!(is_acyclic(
            3,
            &[ProvenanceEdge {
                from_index: 0,
                to_index: 1,
            }]
        ));
    }

    #[tokio::test]
    async fn build_from_chain_carries_each_records_record_hash() {
        let chain = EvidenceChain::new();
        chain.append(b"first".to_vec(), 100).await;
        chain.append(b"second".to_vec(), 200).await;

        let graph = build_from_chain(&chain).await.unwrap();

        for position in 0..graph.node_count() {
            let record = chain.record_at(position as u64).await.unwrap();
            assert_eq!(
                graph.node(position).unwrap().record_hash,
                record.record_hash,
                "node {position} did not carry the record's record_hash"
            );
        }
    }

    #[tokio::test]
    async fn a_nodes_record_hash_differs_from_its_catalog_hash() {
        // The two hashes are domain-separated, so a node must not be
        // carrying the same value twice under two names.
        let chain = EvidenceChain::new();
        chain.append(b"first".to_vec(), 100).await;
        chain.append(b"second".to_vec(), 200).await;

        let graph = build_from_chain(&chain).await.unwrap();

        for position in 0..graph.node_count() {
            let node = graph.node(position).unwrap();
            assert_ne!(node.record_hash, node.hash);
        }
    }

    #[tokio::test]
    async fn a_wrong_record_hash_is_distinguished_only_by_crossing_against_the_chain() {
        let chain = EvidenceChain::new();
        chain.append(b"first".to_vec(), 100).await;
        chain.append(b"second".to_vec(), 200).await;

        let honest = build_from_chain(&chain).await.unwrap();

        // Hand-assemble a graph identical except that node 1's record_hash is
        // wrong. Every field `validate` looks at is untouched, so the graph
        // still passes on its own terms...
        let mut nodes = honest.nodes().to_vec();
        nodes[1].record_hash = [0xEE; 32];
        let tampered = TypedGraph::from_parts(nodes, honest.edges().to_vec());

        assert_eq!(validate(&tampered), Ok(()));

        // ...and only a caller that still holds the chain can tell, by
        // crossing each node's record_hash against the record it came from.
        // This crossing is the caller's to do with the public accessors; the
        // graph cannot do it alone (it keeps no payload to recompute from).
        let mut mismatches = Vec::new();
        for (position, node) in tampered.nodes().iter().enumerate() {
            let record = chain.record_at(position as u64).await.unwrap();
            if node.record_hash != record.record_hash {
                mismatches.push(node.index);
            }
        }

        assert_eq!(mismatches, vec![1]);
    }
}
