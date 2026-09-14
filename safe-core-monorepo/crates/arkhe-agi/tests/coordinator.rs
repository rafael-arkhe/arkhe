use arkhe_agi::{AgiCoordinator, FetchUrlTool, SessionHistory};
use arkhe_core::{AlwaysAllowVerifier, InMemoryAgentMemory};
use arkhe_inference::{
    ChatMessage, FinishReason, InferenceEngine, InferenceRequest, InferenceResponse, InferenceResult, ModelId,
    NullEngine, TokenUsage, ToolCall,
};
use arkhe_orcid::{OrcidClient, OrcidId};
use arkhe_session_evaluator::SessionEvaluator;
use arkhe_reflector_agent::ReflectorAgent;
use std::sync::Arc;

/// A test-only `InferenceEngine` standing in for "a real model decided to
/// call a tool" — `NullEngine` never emits `tool_calls`, so there is no
/// real engine in this workspace yet that can drive
/// `AgiCoordinator::process`'s tool-dispatch path end to end. This proves
/// the dispatch machinery itself: given a response that names a real
/// tool, does that tool actually run and its result actually surface.
struct ToolCallingEngine {
    model_id: ModelId,
    tool_name: String,
    arguments: serde_json::Value,
}

#[async_trait::async_trait]
impl InferenceEngine for ToolCallingEngine {
    fn model_id(&self) -> &ModelId {
        &self.model_id
    }

    async fn complete(&self, _request: InferenceRequest) -> InferenceResult<InferenceResponse> {
        Ok(InferenceResponse {
            content: "calling a tool".to_string(),
            tool_calls: vec![ToolCall { id: "call-1".to_string(), name: self.tool_name.clone(), arguments: self.arguments.clone() }],
            usage: TokenUsage::new(5, 5),
            finish_reason: FinishReason::ToolCall,
            timestamp: chrono::Utc::now(),
            metadata: Default::default(),
        })
    }
}

#[tokio::test]
async fn coordinator_process_works() {
    let safety = Arc::new(AlwaysAllowVerifier);
    let memory = Arc::new(InMemoryAgentMemory::new());
    let inference = Arc::new(NullEngine::new(ModelId::new("test", "null")));
    let evaluator = Arc::new(SessionEvaluator::new());

    let coord = AgiCoordinator::new(
        safety, memory, inference, evaluator, "test-session", "You are helpful.",
    );

    let resp = coord.process("O que é soberania digital?").await.unwrap();
    assert!(!resp.is_empty());
    assert!(resp.contains("soberania digital"));

    // Segundo turno deve funcionar (histórico)
    let resp2 = coord.process("Pode detalhar?").await.unwrap();
    assert!(!resp2.is_empty());

    // Stats
    let (sid, turns, usage) = coord.stats().await;
    assert_eq!(sid, "test-session");
    assert_eq!(turns, 2);
    assert!(!usage.is_zero());
}

#[tokio::test]
async fn coordinator_safety_blocks() {
    use arkhe_core::AlwaysRejectVerifier;
    let safety = Arc::new(AlwaysRejectVerifier { reason: "blocked".into() });
    let memory = Arc::new(InMemoryAgentMemory::new());
    let inference = Arc::new(NullEngine::new(ModelId::new("test", "null")));
    let evaluator = Arc::new(SessionEvaluator::new());

    let coord = AgiCoordinator::new(safety, memory, inference, evaluator, "blocked-session", "sys");

    let result = coord.process("test").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn coordinator_evaluate_session() {
    let safety = Arc::new(AlwaysAllowVerifier);
    let memory = Arc::new(InMemoryAgentMemory::new());
    let inference = Arc::new(NullEngine::new(ModelId::new("test", "null")));
    let evaluator = Arc::new(SessionEvaluator::new());

    let coord = AgiCoordinator::new(safety, memory, inference, evaluator, "eval-session", "sys");

    coord.process("X porque Y. Portanto, Z.").await.unwrap();
    coord.process("Verifiquei que W é verdadeiro.").await.unwrap();

    let result = coord.evaluate_session().await.unwrap();
    assert!((0.0..=1.0).contains(&result.overall_score));
    assert_eq!(result.dimensions.len(), 4);
}

#[tokio::test]
async fn coordinator_builds_provenance_graph_after_turns() {
    let safety = Arc::new(AlwaysAllowVerifier);
    let memory = Arc::new(InMemoryAgentMemory::new());
    let inference = Arc::new(NullEngine::new(ModelId::new("test", "null")));
    let evaluator = Arc::new(SessionEvaluator::new());

    let coord = AgiCoordinator::new(safety, memory, inference, evaluator, "prov-session", "sys");
    coord.process("first turn").await.unwrap();
    coord.process("second turn").await.unwrap();

    let graph = coord.provenance_graph().await.unwrap();
    assert_eq!(graph.node_count(), 2);
}

#[tokio::test]
async fn coordinator_builds_memory_graph_after_turns() {
    let safety = Arc::new(AlwaysAllowVerifier);
    let memory = Arc::new(InMemoryAgentMemory::new());
    let inference = Arc::new(NullEngine::new(ModelId::new("test", "null")));
    let evaluator = Arc::new(SessionEvaluator::new());

    let coord = AgiCoordinator::new(safety, memory, inference, evaluator, "mem-session", "sys");
    coord.process("hello").await.unwrap();

    let graph = coord.memory_graph().await.unwrap();
    assert_eq!(graph.node_count(), 2);
    assert_eq!(graph.edges().len(), 1);
}

#[tokio::test]
async fn coordinator_tracks_latency_stats_across_turns() {
    let safety = Arc::new(AlwaysAllowVerifier);
    let memory = Arc::new(InMemoryAgentMemory::new());
    let inference = Arc::new(NullEngine::new(ModelId::new("test", "null")));
    let evaluator = Arc::new(SessionEvaluator::new());

    let coord = AgiCoordinator::new(safety, memory, inference, evaluator, "latency-session", "sys");

    let empty_stats = coord.latency_stats().await;
    assert_eq!(empty_stats.count, 0);
    assert!(empty_stats.last.is_none());

    coord.process("first turn").await.unwrap();
    coord.process("second turn").await.unwrap();

    let stats = coord.latency_stats().await;
    assert_eq!(stats.count, 2);
    assert!(stats.min_ms as f64 <= stats.avg_ms);
    assert!(stats.avg_ms <= stats.max_ms as f64);
    assert!(stats.last.is_some());
}

#[tokio::test]
async fn coordinator_attests_turns_after_attest_with_orcid() {
    let mut server = mockito::Server::new_async().await;
    let _mock = server
        .mock("GET", "/0000-0002-1825-0097/person")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"name":{"given-names":{"value":"Josiah"},"family-name":{"value":"Carberry"}}}"#)
        .create_async()
        .await;

    let safety = Arc::new(AlwaysAllowVerifier);
    let memory = Arc::new(InMemoryAgentMemory::new());
    let inference = Arc::new(NullEngine::new(ModelId::new("test", "null")));
    let evaluator = Arc::new(SessionEvaluator::new());
    let coord = AgiCoordinator::new(safety, memory, inference, evaluator, "attested-session", "sys");

    assert!(coord.attestor().await.is_none());

    let orcid_client = OrcidClient::with_base_url(server.url());
    let orcid_id = OrcidId::parse("0000-0002-1825-0097").unwrap();
    let verification = coord.attest_with_orcid(&orcid_client, orcid_id).await.unwrap();
    assert_eq!(verification.display_name, "Josiah Carberry");
    assert_eq!(coord.attestor().await.unwrap().display_name, "Josiah Carberry");

    coord.process("hello").await.unwrap();

    let graph = coord.provenance_graph().await.unwrap();
    assert_eq!(graph.node_count(), 1);
}

#[tokio::test]
async fn attested_and_unattested_turns_produce_different_provenance_hashes() {
    let mut server = mockito::Server::new_async().await;
    let _mock = server
        .mock("GET", "/0000-0002-1825-0097/person")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"name":{"given-names":{"value":"Josiah"},"family-name":{"value":"Carberry"}}}"#)
        .create_async()
        .await;

    let unattested = AgiCoordinator::new(
        Arc::new(AlwaysAllowVerifier),
        Arc::new(InMemoryAgentMemory::new()),
        Arc::new(NullEngine::new(ModelId::new("test", "null"))),
        Arc::new(SessionEvaluator::new()),
        "unattested-session",
        "sys",
    );
    unattested.process("same input").await.unwrap();
    let unattested_graph = unattested.provenance_graph().await.unwrap();
    let unattested_hash = unattested_graph.node(0).unwrap().hash;

    let attested = AgiCoordinator::new(
        Arc::new(AlwaysAllowVerifier),
        Arc::new(InMemoryAgentMemory::new()),
        Arc::new(NullEngine::new(ModelId::new("test", "null"))),
        Arc::new(SessionEvaluator::new()),
        "attested-session-2",
        "sys",
    );
    attested
        .attest_with_orcid(&OrcidClient::with_base_url(server.url()), OrcidId::parse("0000-0002-1825-0097").unwrap())
        .await
        .unwrap();
    attested.process("same input").await.unwrap();
    let attested_graph = attested.provenance_graph().await.unwrap();
    let attested_hash = attested_graph.node(0).unwrap().hash;

    assert_ne!(unattested_hash, attested_hash);
}

#[tokio::test]
async fn process_planned_splits_multi_sentence_input_into_multiple_attested_turns() {
    let coord = AgiCoordinator::new(
        Arc::new(AlwaysAllowVerifier),
        Arc::new(InMemoryAgentMemory::new()),
        Arc::new(NullEngine::new(ModelId::new("test", "null"))),
        Arc::new(SessionEvaluator::new()),
        "planned-session",
        "sys",
    );

    let responses = coord.process_planned("What is X? What is Y?").await.unwrap();
    assert_eq!(responses.len(), 2);
    assert!(responses[0].contains("What is X?"));
    assert!(responses[1].contains("What is Y?"));

    let (_, turn_count, _) = coord.stats().await;
    assert_eq!(turn_count, 2);

    let graph = coord.provenance_graph().await.unwrap();
    assert_eq!(graph.node_count(), 2);
}

#[tokio::test]
async fn process_planned_on_a_single_sentence_behaves_like_one_process_call() {
    let coord = AgiCoordinator::new(
        Arc::new(AlwaysAllowVerifier),
        Arc::new(InMemoryAgentMemory::new()),
        Arc::new(NullEngine::new(ModelId::new("test", "null"))),
        Arc::new(SessionEvaluator::new()),
        "planned-single-session",
        "sys",
    );

    let responses = coord.process_planned("hello arkhe").await.unwrap();
    assert_eq!(responses.len(), 1);

    let (_, turn_count, _) = coord.stats().await;
    assert_eq!(turn_count, 1);
}

#[tokio::test]
async fn process_planned_stops_at_the_first_safety_rejection() {
    let coord = AgiCoordinator::new(
        Arc::new(arkhe_core::AlwaysRejectVerifier { reason: "blocked".into() }),
        Arc::new(InMemoryAgentMemory::new()),
        Arc::new(NullEngine::new(ModelId::new("test", "null"))),
        Arc::new(SessionEvaluator::new()),
        "planned-rejected-session",
        "sys",
    );

    let result = coord.process_planned("Step one? Step two?").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn process_dispatches_a_tool_call_and_surfaces_its_result() {
    let mut server = mockito::Server::new_async().await;
    let _mock = server.mock("GET", "/weather").with_status(200).with_body("sunny, 22C").create_async().await;

    let coord = AgiCoordinator::new(
        Arc::new(AlwaysAllowVerifier),
        Arc::new(InMemoryAgentMemory::new()),
        Arc::new(ToolCallingEngine {
            model_id: ModelId::new("test", "tool-caller"),
            tool_name: "fetch_url".to_string(),
            arguments: serde_json::json!({"url": "/weather"}),
        }),
        Arc::new(SessionEvaluator::new()),
        "tool-session",
        "sys",
    );
    coord.register_tool(FetchUrlTool::with_base_url(server.url())).await;

    let response = coord.process("what's the weather?").await.unwrap();
    assert!(response.contains("calling a tool"));
    assert!(response.contains("[tool:fetch_url]"));
    assert!(response.contains("sunny, 22C"));
}

#[tokio::test]
async fn process_surfaces_a_failed_tool_call_without_erroring_the_turn() {
    let coord = AgiCoordinator::new(
        Arc::new(AlwaysAllowVerifier),
        Arc::new(InMemoryAgentMemory::new()),
        Arc::new(ToolCallingEngine {
            model_id: ModelId::new("test", "tool-caller"),
            tool_name: "does_not_exist".to_string(),
            arguments: serde_json::json!({}),
        }),
        Arc::new(SessionEvaluator::new()),
        "tool-fail-session",
        "sys",
    );

    let response = coord.process("hello").await.unwrap();
    assert!(response.contains("[tool:does_not_exist failed]"));
}

#[test]
fn session_history_works() {
    let mut h = SessionHistory::new("test");
    assert_eq!(h.turn_count(), 0);

    h.push_turn("hello", "world", arkhe_inference::TokenUsage::new(5, 3));
    assert_eq!(h.turn_count(), 1);
    assert_eq!(h.total_usage().total_tokens, 8);
    assert_eq!(h.messages().len(), 2);

    let v = h.to_vec();
    assert_eq!(v.len(), 2);

    h.clear();
    assert_eq!(h.turn_count(), 0);
    assert!(h.total_usage().is_zero());
}
