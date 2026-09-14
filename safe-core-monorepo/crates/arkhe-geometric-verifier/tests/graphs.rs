//! The public API exercised the way `arkhe-agi` exercises it: a record type
//! and a rule defined *outside* this crate, a memory graph built from an
//! `AgentMemory`, and a provenance graph built from a real `EvidenceChain`.
//!
//! The counts asserted here are the ones `AgiCoordinator`'s tests assert
//! (`crates/arkhe-agi/tests/coordinator.rs`): one turn yields two memory
//! nodes joined by one edge, and two turns yield two provenance nodes.

use arkhe_core::{AgentMemory, InMemoryAgentMemory, MemoryEntry, MemoryLayer};
use arkhe_evidence::EvidenceChain;
use arkhe_geometric_verifier::{
    memory_graph, provenance_graph, GeometricVerifier, TypedGraph, VerifiableRecord,
    VerificationRule,
};
use chrono::Utc;

/// Stands in for `arkhe-agi`'s `TurnRecord`.
struct TurnRecord {
    user_input: String,
    response: String,
    attested_by: Option<String>,
}

impl VerifiableRecord for TurnRecord {
    fn user_input(&self) -> &str {
        &self.user_input
    }

    fn response(&self) -> &str {
        &self.response
    }

    fn attested_by(&self) -> Option<&str> {
        self.attested_by.as_deref()
    }
}

/// Stands in for `arkhe-agi`'s `NonEmptyResponse`.
struct NonEmptyResponse;

impl VerificationRule for NonEmptyResponse {
    fn check(&self, record: &dyn VerifiableRecord) -> Result<(), String> {
        if record.response().trim().is_empty() {
            Err("the response is empty".to_string())
        } else {
            Ok(())
        }
    }
}

fn turn(response: &str) -> TurnRecord {
    TurnRecord {
        user_input: "question".to_string(),
        response: response.to_string(),
        attested_by: None,
    }
}

fn entry(key: &str, layer: MemoryLayer, score: f32) -> MemoryEntry {
    MemoryEntry {
        key: key.to_string(),
        value: format!("value for {key}"),
        score,
        layer,
        timestamp: Utc::now(),
    }
}

#[test]
fn a_rule_and_a_record_can_both_be_defined_outside_the_crate() {
    let mut verifier = GeometricVerifier::new();
    verifier.register(NonEmptyResponse);

    let artifact = verifier.verify(&turn("an answer")).unwrap();
    assert_eq!(artifact.checks.len(), 1);
    assert!(!artifact.canonical_bytes.is_empty());

    let err = verifier.verify(&turn("")).unwrap_err();
    assert!(err.to_string().contains("the response is empty"), "{err}");
}

#[test]
fn an_attested_turn_hashes_differently_from_an_unattested_one() {
    let verifier = GeometricVerifier::new();

    let unattested = verifier.verify(&turn("same output")).unwrap();
    let mut attested_turn = turn("same output");
    attested_turn.attested_by = Some("Josiah Carberry (0000-0002-1825-0097)".to_string());
    let attested = verifier.verify(&attested_turn).unwrap();

    assert_ne!(unattested.canonical_bytes, attested.canonical_bytes);
    assert_ne!(unattested.coordinate, attested.coordinate);
}

#[tokio::test]
async fn the_canonical_bytes_of_a_turn_can_be_chained_and_verified() {
    let verifier = GeometricVerifier::new();
    let chain = EvidenceChain::new();

    for (index, response) in ["first", "second"].iter().enumerate() {
        let artifact = verifier.verify(&turn(response)).unwrap();
        chain.append(artifact.canonical_bytes, index as u64).await;
    }

    assert!(chain.verify_chain().await.is_ok());

    let graph = provenance_graph::build_from_chain(&chain).await.unwrap();
    assert_eq!(graph.node_count(), 2);
    assert_eq!(graph.edges().len(), 1);

    // The graph's first node is the hash of the first turn's canonical
    // bytes, and the second chains back to it.
    let second = graph.node(1).unwrap();
    assert_eq!(second.prev_hash, graph.node(0).unwrap().hash);
}

#[tokio::test]
async fn one_turn_of_memory_links_produces_two_nodes_and_one_edge() {
    let memory = InMemoryAgentMemory::new();
    memory
        .store(entry("session:turn-1", MemoryLayer::Working, 0.9))
        .await
        .unwrap();
    memory
        .store(entry("session:ep:1", MemoryLayer::Episodic, 0.7))
        .await
        .unwrap();

    // Exactly what AgiCoordinator::process pushes, once per turn.
    let links = vec![(
        "session:turn-1".to_string(),
        "session:ep:1".to_string(),
        1.0f32,
    )];

    let graph: TypedGraph<memory_graph::MemoryNode, memory_graph::MemoryEdge> =
        memory_graph::build_from_memory(&memory, &links)
            .await
            .unwrap();

    assert_eq!(graph.node_count(), 2);
    assert_eq!(graph.edges().len(), 1);
    assert_eq!(graph.edges()[0].weight, 1.0);

    // The graph is a description of what is in memory, not a copy of the
    // keys it was told about.
    assert_eq!(graph.node(0).unwrap().layer, MemoryLayer::Working);
    assert_eq!(graph.node(1).unwrap().layer, MemoryLayer::Episodic);
}

#[tokio::test]
async fn two_turns_of_memory_links_produce_four_nodes_and_two_edges() {
    let memory = InMemoryAgentMemory::new();
    for key in ["w1", "e1", "w2", "e2"] {
        let layer = if key.starts_with('w') {
            MemoryLayer::Working
        } else {
            MemoryLayer::Episodic
        };
        memory.store(entry(key, layer, 0.5)).await.unwrap();
    }

    let links = vec![
        ("w1".to_string(), "e1".to_string(), 1.0f32),
        ("w2".to_string(), "e2".to_string(), 1.0f32),
    ];

    let graph = memory_graph::build_from_memory(&memory, &links)
        .await
        .unwrap();

    assert_eq!(graph.node_count(), 4);
    assert_eq!(graph.edges().len(), 2);
}

#[tokio::test]
async fn a_chain_of_two_turns_is_the_two_node_graph_the_coordinator_expects() {
    let chain = EvidenceChain::new();
    chain.append(b"turn one".to_vec(), 1_700_000_000).await;
    chain.append(b"turn two".to_vec(), 1_700_000_060).await;

    let graph = provenance_graph::build_from_chain(&chain).await.unwrap();

    assert_eq!(graph.node_count(), 2);
    assert_eq!(graph.node(0).unwrap().index, 0);
    assert_eq!(graph.node(1).unwrap().index, 1);
    assert_eq!(graph.node(1).unwrap().timestamp, 1_700_000_060);
    assert_eq!(graph.edges()[0].from_index, 0);
    assert_eq!(graph.edges()[0].to_index, 1);
}
