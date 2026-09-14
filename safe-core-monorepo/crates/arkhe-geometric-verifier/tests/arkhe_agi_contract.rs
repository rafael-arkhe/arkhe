//! Compiler-checked proof that these two crates satisfy the API that
//! `arkhe-agi`'s `AgiCoordinator` actually uses.
//!
//! `cargo check -p arkhe-agi` cannot prove this: `arkhe-agi` declares five
//! internal modules that do not exist (`compliance`, `executor`, `geometry`,
//! `metrics`, `planner`), and rustc aborts with `E0583 file not found for
//! module` *before* it resolves any name — so the absence of unresolved-import
//! errors from that command would mean nothing. What is needed is a
//! compilation that reaches the imports.
//!
//! So this file reproduces `crates/arkhe-agi/src/coordinator.rs` as closely as
//! it can without that crate's missing modules:
//!
//! - the import block is copied verbatim from `coordinator.rs:18-23`;
//! - `MirroredCoordinator` mirrors `AgiCoordinator`'s use of each imported
//!   item — the same constructor calls, the same `verify` call without
//!   `.await` at `coordinator.rs:208-209`, the same
//!   `format!("{} ({})", v.display_name, v.id)` at `:206`, the same return
//!   types at `:348` and `:354`;
//! - the assertions are the ones `crates/arkhe-agi/tests/coordinator.rs`
//!   makes, where they only concern these two crates.
//!
//! If a signature here drifts from the consumers, this file stops compiling.

// The two imports `coordinator.rs:18-23` makes, verbatim — the only change is
// that this file names both crates' items separately because it also imports
// `VerifiableRecord`/`VerificationRule`, which `arkhe-agi`'s missing
// `geometry`/`compliance` modules are what would ordinarily bring in.
use arkhe_core::{
    AgentMemory, ArkheError, ArkheResult, InMemoryAgentMemory, MemoryEntry, MemoryLayer,
};
use arkhe_evidence::EvidenceChain;
use arkhe_geometric_verifier::{
    memory_graph::{self, MemoryEdge, MemoryGraphError, MemoryNode},
    provenance_graph::{self, ProvenanceEdge, ProvenanceError, ProvenanceNode},
    GeometricVerifier, TypedGraph,
};
use arkhe_geometric_verifier::{VerifiableRecord, VerificationRule};
use arkhe_orcid::{OrcidClient, OrcidError, OrcidId, OrcidVerification};

use std::sync::Arc;

/// Stands in for `arkhe_agi::geometry::TurnRecord`, whose field names are
/// taken from `coordinator.rs:207`.
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

/// Stands in for `arkhe_agi::geometry::NonEmptyResponse`.
struct NonEmptyResponse;

impl VerificationRule for NonEmptyResponse {
    fn check(&self, record: &dyn VerifiableRecord) -> Result<(), String> {
        if record.response().is_empty() {
            Err("the response is empty".to_string())
        } else {
            Ok(())
        }
    }
}

/// Stands in for `arkhe_agi::compliance::NoUnredactedCpf`.
struct NoUnredactedCpf;

impl VerificationRule for NoUnredactedCpf {
    fn check(&self, record: &dyn VerifiableRecord) -> Result<(), String> {
        let text = format!("{} {}", record.user_input(), record.response());
        if text.split_whitespace().any(looks_like_a_document_number) {
            Err("the turn carries an unredacted document number".to_string())
        } else {
            Ok(())
        }
    }
}

/// Stand-in for the real redaction check: eleven digits in the
/// `000.000.000-00` shape.
fn looks_like_a_document_number(token: &str) -> bool {
    let mut digits = 0;
    let mut dots = 0;
    for c in token.chars() {
        match c {
            '0'..='9' => digits += 1,
            '.' => dots += 1,
            '-' => {}
            _ => return false,
        }
    }
    digits == 11 && dots == 2
}

/// Mirrors the parts of `AgiCoordinator` that touch these two crates —
/// `coordinator.rs:38-73` for the fields, `:82-106` for `new`, `:120-129` for
/// the ORCID attestation pair, `:195-211` for the geometric/evidence step,
/// `:240-242` for the memory link, and `:344-357` for the two graph builders.
struct MirroredCoordinator<M: AgentMemory> {
    evidence: EvidenceChain,
    geometric: GeometricVerifier,
    memory_links: tokio::sync::RwLock<Vec<(String, String, f32)>>,
    memory: Arc<M>,
    attestor: tokio::sync::RwLock<Option<OrcidVerification>>,
}

impl<M: AgentMemory> MirroredCoordinator<M> {
    fn new(memory: Arc<M>) -> Self {
        let mut geometric = GeometricVerifier::new();
        geometric.register(NonEmptyResponse);
        geometric.register(NoUnredactedCpf);
        Self {
            evidence: EvidenceChain::new(),
            geometric,
            memory_links: tokio::sync::RwLock::new(Vec::new()),
            memory,
            attestor: tokio::sync::RwLock::new(None),
        }
    }

    async fn attest_with_orcid(
        &self,
        client: &OrcidClient,
        orcid: OrcidId,
    ) -> Result<OrcidVerification, OrcidError> {
        let verification = client.verify(&orcid).await?;
        *self.attestor.write().await = Some(verification.clone());
        Ok(verification)
    }

    async fn attestor(&self) -> Option<OrcidVerification> {
        self.attestor.read().await.clone()
    }

    /// The turn path, minus inference: geometric verification, provenance
    /// append, memory store, memory link.
    async fn process_turn(&self, user_input: &str, response: &str) -> ArkheResult<()> {
        let now = chrono::Utc::now();

        let attested_by = self
            .attestor
            .read()
            .await
            .as_ref()
            .map(|v| format!("{} ({})", v.display_name, v.id));

        let turn = TurnRecord {
            user_input: user_input.to_string(),
            response: response.to_string(),
            attested_by,
        };

        // Synchronous — the real call site is `.verify(&turn).map_err(..)`
        // with no `.await` (`coordinator.rs:208-209`).
        let artifact = self
            .geometric
            .verify(&turn)
            .map_err(|e| ArkheError::Internal(e.to_string()))?;

        self.evidence
            .append(artifact.canonical_bytes, now.timestamp() as u64)
            .await;

        let turn_key = format!("{}-turn", now.timestamp_millis());
        let episode_key = format!("{}-ep", now.timestamp_millis());

        self.memory
            .store(MemoryEntry {
                key: turn_key.clone(),
                value: format!("User: {user_input}\nAssistant: {response}"),
                score: 0.9,
                layer: MemoryLayer::Working,
                timestamp: now,
            })
            .await
            .map_err(ArkheError::Internal)?;

        self.memory
            .store(MemoryEntry {
                key: episode_key.clone(),
                value: format!("Query: {user_input}"),
                score: 0.7,
                layer: MemoryLayer::Episodic,
                timestamp: now,
            })
            .await
            .map_err(ArkheError::Internal)?;

        self.memory_links
            .write()
            .await
            .push((turn_key, episode_key, 1.0));

        Ok(())
    }

    async fn provenance_graph(
        &self,
    ) -> Result<TypedGraph<ProvenanceNode, ProvenanceEdge>, ProvenanceError> {
        provenance_graph::build_from_chain(&self.evidence).await
    }

    async fn memory_graph(&self) -> Result<TypedGraph<MemoryNode, MemoryEdge>, MemoryGraphError> {
        let links = self.memory_links.read().await.clone();
        memory_graph::build_from_memory(self.memory.as_ref(), &links).await
    }
}

fn coordinator() -> MirroredCoordinator<InMemoryAgentMemory> {
    MirroredCoordinator::new(Arc::new(InMemoryAgentMemory::new()))
}

#[tokio::test]
async fn two_turns_produce_a_two_node_provenance_graph() {
    // `arkhe-agi/tests/coordinator.rs:100-112`.
    let coord = coordinator();
    coord
        .process_turn("first turn", "answer one")
        .await
        .unwrap();
    coord
        .process_turn("second turn", "answer two")
        .await
        .unwrap();

    let graph = coord.provenance_graph().await.unwrap();
    assert_eq!(graph.node_count(), 2);
}

#[tokio::test]
async fn one_turn_produces_two_memory_nodes_and_one_edge() {
    // `arkhe-agi/tests/coordinator.rs:114-127`.
    let coord = coordinator();
    coord.process_turn("hello", "hi").await.unwrap();

    let graph = coord.memory_graph().await.unwrap();
    assert_eq!(graph.node_count(), 2);
    assert_eq!(graph.edges().len(), 1);
}

#[tokio::test]
async fn a_multi_sentence_turn_without_inference_is_still_two_records() {
    // `arkhe-agi/tests/coordinator.rs:226-246` processes two planned steps
    // and expects two provenance nodes; two turns is the same shape.
    let coord = coordinator();
    coord.process_turn("What is X?", "X is one").await.unwrap();
    coord.process_turn("What is Y?", "Y is two").await.unwrap();

    let graph = coord.provenance_graph().await.unwrap();
    assert_eq!(graph.node_count(), 2);
    assert_eq!(graph.edges().len(), 1);
}

#[tokio::test]
async fn attested_and_unattested_turns_produce_different_provenance_hashes() {
    // `arkhe-agi/tests/coordinator.rs:183-223`, using the same mock body.
    let mut server = mockito::Server::new_async().await;
    let _mock = server
        .mock("GET", "/0000-0002-1825-0097/person")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{"name":{"given-names":{"value":"Josiah"},"family-name":{"value":"Carberry"}}}"#,
        )
        .create_async()
        .await;

    let unattested = coordinator();
    unattested
        .process_turn("same input", "same output")
        .await
        .unwrap();
    let unattested_hash = unattested
        .provenance_graph()
        .await
        .unwrap()
        .node(0)
        .unwrap()
        .hash;

    let attested = coordinator();
    let verification = attested
        .attest_with_orcid(
            &OrcidClient::with_base_url(server.url()),
            OrcidId::parse("0000-0002-1825-0097").unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(verification.display_name, "Josiah Carberry");
    assert_eq!(
        attested.attestor().await.unwrap().display_name,
        "Josiah Carberry"
    );

    attested
        .process_turn("same input", "same output")
        .await
        .unwrap();
    let attested_hash = attested
        .provenance_graph()
        .await
        .unwrap()
        .node(0)
        .unwrap()
        .hash;

    assert_ne!(unattested_hash, attested_hash);
}

#[tokio::test]
async fn a_rule_rejection_surfaces_as_an_arkhe_error_with_the_rule_name() {
    let coord = coordinator();

    // `coordinator.rs:208-209` maps the rejection into `ArkheError::Internal`
    // by calling `to_string()` on it, so the message has to carry the reason.
    let err = coord
        .process_turn("question", "")
        .await
        .expect_err("an empty response must be rejected");

    let message = err.to_string();
    assert!(message.contains("the response is empty"), "{message}");
    assert!(message.contains("NonEmptyResponse"), "{message}");

    // Nothing was admitted into the chain.
    assert_eq!(coord.provenance_graph().await.unwrap().node_count(), 0);
}

#[tokio::test]
async fn the_second_registered_rule_rejects_too() {
    let coord = coordinator();

    let err = coord
        .process_turn("my cpf is 123.456.789-00", "noted")
        .await
        .expect_err("an unredacted document number must be rejected");

    assert!(err.to_string().contains("NoUnredactedCpf"), "{err}");
}

#[tokio::test]
async fn an_empty_chain_yields_an_empty_provenance_graph() {
    let coord = coordinator();
    let graph = coord.provenance_graph().await.unwrap();
    assert_eq!(graph.node_count(), 0);
    assert_eq!(graph.edges().len(), 0);
}
