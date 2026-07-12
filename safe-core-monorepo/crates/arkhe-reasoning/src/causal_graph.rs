//! FI-037 — causal graphs: a set of claims of the shape "node N was caused
//! by node C" must be both acyclic (FI-031's Kahn's-algorithm check, reused
//! here) *and* temporally consistent — a cause can never be timestamped
//! later than the effect it's claimed to have produced. Acyclicity alone
//! (what `validator.rs` checks for execution plans) does not catch this: a
//! plan's `depends_on` edges have no real-world timestamps attached, so
//! there is nothing to violate. A causal graph's nodes always carry a
//! timestamp (they describe things that already happened — e.g. an
//! `arkhe-rsi-core::IterationRecord` or an `arkhe-evidence::EvidenceRecord`
//! a caller maps into a [`CausalNode`]), which is exactly what makes
//! "effect precedes its own cause" a real, checkable error and not just a
//! style question.
//!
//! This module is deliberately generic over `NodeId`/timestamps rather than
//! coupled to any specific record type (`IterationRecord`,
//! `EvidenceRecord`, ...) — matching this crate's existing
//! dependency-free scope (see the crate README): a caller maps its own
//! real, already-hash-chained records into [`CausalNode`]s and validates
//! the resulting graph.

use std::collections::{HashMap, HashSet, VecDeque};

/// Identifies a node in a causal graph. A caller typically uses the same
/// identifier its underlying record already has (e.g. an
/// `IterationRecord::hash` truncated to `u64`, or any stable numeric id).
pub type NodeId = u64;

/// A node in a causal graph: something that happened at `timestamp_secs`,
/// optionally claiming to have been caused by other nodes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CausalNode {
    pub id: NodeId,
    /// Unix timestamp, seconds, of when this node's event occurred.
    pub timestamp_secs: u64,
    /// The nodes this one claims to have been caused by. Order does not
    /// matter and duplicates are harmless (checked once each).
    pub caused_by: Vec<NodeId>,
}

/// A causal graph: a set of [`CausalNode`]s and their `caused_by` claims.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CausalGraph {
    pub nodes: Vec<CausalNode>,
}

/// Why a [`CausalGraph`] failed validation.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CausalError {
    /// The causal graph has a cycle — some node is, transitively, its own
    /// cause. Carries the node IDs that could not be topologically sorted.
    #[error("causal graph contains a cycle involving node(s): {0:?}")]
    Cyclic(Vec<NodeId>),
    /// A node claims to be caused by a node not present in the graph.
    #[error("node {node} claims to be caused by unknown node {missing_cause}")]
    UnknownCause { node: NodeId, missing_cause: NodeId },
    /// A node's claimed cause is timestamped *after* the node itself — the
    /// claimed effect cannot have happened before its own cause.
    #[error(
        "node {node} (t={node_timestamp}) claims to be caused by node {cause} (t={cause_timestamp}), \
         but the cause is timestamped after the effect"
    )]
    EffectPrecedesCause { node: NodeId, cause: NodeId, node_timestamp: u64, cause_timestamp: u64 },
}

/// Validates a [`CausalGraph`]: acyclic (via Kahn's algorithm, same
/// approach as [`crate::PlanValidator`]) and temporally consistent (no
/// cause timestamped after its effect).
#[derive(Debug, Default)]
pub struct CausalGraphValidator;

impl CausalGraphValidator {
    pub fn new() -> Self {
        Self
    }

    /// Returns a valid causal order (earliest causes first) if `graph` is
    /// acyclic and temporally consistent, `Err` otherwise. Unknown-cause
    /// and temporal-consistency checks run before the cycle check, since
    /// both are cheaper, per-edge checks that don't need the full
    /// topological sort to detect.
    pub fn validate(&self, graph: &CausalGraph) -> Result<Vec<NodeId>, CausalError> {
        let by_id: HashMap<NodeId, &CausalNode> = graph.nodes.iter().map(|n| (n.id, n)).collect();

        let mut in_degree: HashMap<NodeId, usize> = HashMap::new();
        let mut dependents: HashMap<NodeId, Vec<NodeId>> = HashMap::new();

        for node in &graph.nodes {
            in_degree.entry(node.id).or_insert(0);
            for &cause_id in &node.caused_by {
                let cause = by_id
                    .get(&cause_id)
                    .ok_or(CausalError::UnknownCause { node: node.id, missing_cause: cause_id })?;
                if cause.timestamp_secs > node.timestamp_secs {
                    return Err(CausalError::EffectPrecedesCause {
                        node: node.id,
                        cause: cause_id,
                        node_timestamp: node.timestamp_secs,
                        cause_timestamp: cause.timestamp_secs,
                    });
                }
                *in_degree.entry(node.id).or_insert(0) += 1;
                dependents.entry(cause_id).or_default().push(node.id);
            }
        }

        let mut queue: VecDeque<NodeId> =
            in_degree.iter().filter(|(_, &deg)| deg == 0).map(|(&id, _)| id).collect();

        let mut sorted = Vec::with_capacity(graph.nodes.len());
        while let Some(id) = queue.pop_front() {
            sorted.push(id);
            if let Some(deps) = dependents.get(&id) {
                for &dependent in deps {
                    let entry = in_degree.get_mut(&dependent).expect("dependent was seen while building in_degree");
                    *entry -= 1;
                    if *entry == 0 {
                        queue.push_back(dependent);
                    }
                }
            }
        }

        if sorted.len() != graph.nodes.len() {
            let sorted_set: HashSet<NodeId> = sorted.iter().copied().collect();
            let remaining: Vec<NodeId> = graph.nodes.iter().map(|n| n.id).filter(|id| !sorted_set.contains(id)).collect();
            return Err(CausalError::Cyclic(remaining));
        }

        Ok(sorted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: NodeId, timestamp_secs: u64, caused_by: &[NodeId]) -> CausalNode {
        CausalNode { id, timestamp_secs, caused_by: caused_by.to_vec() }
    }

    #[test]
    fn linear_causal_chain_is_accepted_earliest_first() {
        // 1 (t=100) caused 2 (t=200) caused 3 (t=300).
        let graph = CausalGraph {
            nodes: vec![node(1, 100, &[]), node(2, 200, &[1]), node(3, 300, &[2])],
        };
        let order = CausalGraphValidator::new().validate(&graph).unwrap();
        let pos = |id: NodeId| order.iter().position(|&x| x == id).unwrap();
        assert!(pos(1) < pos(2));
        assert!(pos(2) < pos(3));
    }

    #[test]
    fn diamond_causal_graph_is_accepted() {
        let graph = CausalGraph {
            nodes: vec![node(1, 100, &[]), node(2, 200, &[1]), node(3, 200, &[1]), node(4, 300, &[2, 3])],
        };
        let order = CausalGraphValidator::new().validate(&graph).unwrap();
        assert_eq!(order.len(), 4);
    }

    #[test]
    fn empty_graph_is_accepted() {
        assert_eq!(CausalGraphValidator::new().validate(&CausalGraph::default()).unwrap(), Vec::<NodeId>::new());
    }

    #[test]
    fn direct_cycle_is_rejected() {
        // Same timestamp so the cycle check itself is what's exercised,
        // not the temporal check short-circuiting first.
        let graph = CausalGraph { nodes: vec![node(1, 100, &[2]), node(2, 100, &[1])] };
        assert!(matches!(CausalGraphValidator::new().validate(&graph), Err(CausalError::Cyclic(_))));
    }

    #[test]
    fn self_causation_is_a_cycle() {
        let graph = CausalGraph { nodes: vec![node(1, 100, &[1])] };
        assert!(matches!(CausalGraphValidator::new().validate(&graph), Err(CausalError::Cyclic(_))));
    }

    #[test]
    fn causation_by_an_unknown_node_is_rejected() {
        let graph = CausalGraph { nodes: vec![node(1, 100, &[999])] };
        let result = CausalGraphValidator::new().validate(&graph);
        assert_eq!(result, Err(CausalError::UnknownCause { node: 1, missing_cause: 999 }));
    }

    #[test]
    fn a_cause_timestamped_after_its_effect_is_rejected() {
        // Node 2 (t=100) claims to be caused by node 1 (t=200) — the
        // "cause" happens later than the "effect", which is incoherent.
        let graph = CausalGraph { nodes: vec![node(1, 200, &[]), node(2, 100, &[1])] };
        let result = CausalGraphValidator::new().validate(&graph);
        assert_eq!(
            result,
            Err(CausalError::EffectPrecedesCause { node: 2, cause: 1, node_timestamp: 100, cause_timestamp: 200 })
        );
    }

    #[test]
    fn a_cause_at_the_same_timestamp_as_its_effect_is_accepted() {
        // Same-second resolution is coarse enough that "simultaneous" must
        // not be treated as a violation — only a cause strictly *after*
        // its effect is.
        let graph = CausalGraph { nodes: vec![node(1, 100, &[]), node(2, 100, &[1])] };
        assert!(CausalGraphValidator::new().validate(&graph).is_ok());
    }

    #[test]
    fn a_longer_cycle_is_rejected_even_when_every_edge_is_temporally_fine() {
        // 3 -> 1 -> 2 -> 3, all at the same timestamp so every individual
        // edge passes the temporal check (a real cycle can only have every
        // edge temporally non-violating if every node in it shares the
        // same timestamp — otherwise the inequalities chain into a
        // contradiction) — only the whole-cycle structure is wrong.
        // Confirms the cycle check still fires even when no single edge
        // looks locally wrong.
        let graph = CausalGraph { nodes: vec![node(1, 100, &[3]), node(2, 100, &[1]), node(3, 100, &[2])] };
        assert!(matches!(CausalGraphValidator::new().validate(&graph), Err(CausalError::Cyclic(_))));
    }
}
