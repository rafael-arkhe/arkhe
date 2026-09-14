//! AgiCoordinator — Orquestrador central DESACOPLADO.
//!
//! Usa traits de arkhe-core (SafetyVerifier, AgentMemory) em vez de
//! depender de arkhe-pea e arkhe-memory diretamente. Isso permite:
//! - Testar com AlwaysAllowVerifier + InMemoryAgentMemory
//! - Trocar implementações sem recompilar
//! - Eliminar dependências circulares

use arkhe_core::{
    ArkheResult, SafetyVerifier, SafetyVerdict, AgentMemory, MemoryEntry, MemoryLayer,
};
use arkhe_inference::{
    InferenceEngine, InferenceRequest, InferenceResponse, ChatMessage, ChatRole, FinishReason,
};
use arkhe_session_evaluator::{SessionTrajectory, SessionEvaluator};
use arkhe_reflector_agent::ReflectorAgent;
use arkhe_evidence::EvidenceChain;
use arkhe_geometric_verifier::{
    GeometricVerifier, TypedGraph,
    memory_graph::{self, MemoryEdge, MemoryGraphError, MemoryNode},
    provenance_graph::{self, ProvenanceEdge, ProvenanceError, ProvenanceNode},
};
use arkhe_orcid::{OrcidClient, OrcidError, OrcidId, OrcidVerification};
use crate::executor::{Tool, ToolRegistry};
use crate::geometry::{NonEmptyResponse, TurnRecord};
use crate::metrics::{LatencyStats, MetricsRecorder, PhaseTimings};
use crate::planner;
use crate::session::SessionHistory;
use std::sync::Arc;
use tracing::{info, warn, error};

/// Coordenador AGI — ponto central de orquestração.
///
/// Genérico sobre 3 componentes:
/// - `V`: Verificador de segurança (ex: SafetyEnforcer do PEA, ou AlwaysAllowVerifier para dev)
/// - `M`: Memória do agente (ex: EvolutionaryMemory, ou InMemoryAgentMemory para testes)
/// - `I`: Motor de inferência (ex: MistralRsEngine, NullEngine, etc.)
pub struct AgiCoordinator<V, M, I>
where
    V: SafetyVerifier,
    M: AgentMemory,
    I: InferenceEngine,
{
    safety: Arc<V>,
    memory: Arc<M>,
    inference: Arc<I>,
    evaluator: Arc<SessionEvaluator>,
    history: tokio::sync::RwLock<SessionHistory>,
    system_prompt: String,
    /// Tamper-evident, hash-chained record of every turn's canonical bytes
    /// (FI-011/FI-017), used to build [`Self::provenance_graph`].
    evidence: EvidenceChain,
    /// Runs `canonicalize -> BLAKE3 -> invariants -> coordinate` (FI-120)
    /// over each turn before it's admitted into `evidence`.
    geometric: GeometricVerifier,
    /// `(working_key, episodic_key, weight)` links recorded per turn, used
    /// to build [`Self::memory_graph`].
    memory_links: tokio::sync::RwLock<Vec<(String, String, f32)>>,
    /// Per-phase timing of recent turns, used by [`Self::latency_stats`]
    /// to answer "where does time go?" with real numbers instead of
    /// guesswork.
    metrics: MetricsRecorder,
    /// The verified researcher identity currently attesting every turn's
    /// provenance record, if [`Self::attest_with_orcid`] has been called.
    /// Verified once and cached here rather than re-verified per turn —
    /// re-hitting the live ORCID API on every `process()` call would add
    /// real network latency and rate-limit risk for a name that doesn't
    /// change turn-to-turn.
    attestor: tokio::sync::RwLock<Option<OrcidVerification>>,
    /// Tools this coordinator can dispatch a `ToolCall` to. Empty by
    /// default — register via [`Self::register_tool`].
    tools: ToolRegistry,
}

impl<V, M, I> AgiCoordinator<V, M, I>
where
    V: SafetyVerifier,
    M: AgentMemory,
    I: InferenceEngine,
{
    /// Cria um novo coordenador.
    pub fn new(
        safety: Arc<V>,
        memory: Arc<M>,
        inference: Arc<I>,
        evaluator: Arc<SessionEvaluator>,
        session_id: &str,
        system_prompt: &str,
    ) -> Self {
        let mut geometric = GeometricVerifier::new();
        geometric.register(NonEmptyResponse);
        geometric.register(crate::compliance::NoUnredactedCpf);
        Self {
            safety,
            memory,
            inference,
            evaluator,
            history: tokio::sync::RwLock::new(SessionHistory::new(session_id)),
            system_prompt: system_prompt.to_string(),
            evidence: EvidenceChain::new(),
            geometric,
            memory_links: tokio::sync::RwLock::new(Vec::new()),
            metrics: MetricsRecorder::new(),
            attestor: tokio::sync::RwLock::new(None),
            tools: ToolRegistry::new(),
        }
    }

    /// Registers `tool` so future `process()` calls both advertise it to
    /// the inference engine (via `InferenceRequest::tools`) and dispatch
    /// any resulting `ToolCall`s to it.
    pub async fn register_tool(&self, tool: impl Tool + 'static) {
        self.tools.register(tool).await;
    }

    /// Verifies `orcid` against the live ORCID public API via `client`
    /// and, on success, caches the result so every subsequent
    /// `process()` call attests its turn's provenance record with it.
    /// Does not affect turns already processed before this call.
    pub async fn attest_with_orcid(&self, client: &OrcidClient, orcid: OrcidId) -> Result<OrcidVerification, OrcidError> {
        let verification = client.verify(&orcid).await?;
        *self.attestor.write().await = Some(verification.clone());
        Ok(verification)
    }

    /// The verified identity currently attesting turns, if any.
    pub async fn attestor(&self) -> Option<OrcidVerification> {
        self.attestor.read().await.clone()
    }

    /// Processa input do usuário com ciclo completo.
    ///
    /// Fluxo:
    /// 1. Verificar segurança
    /// 2. Construir requisição (histórico + novo input)
    /// 3. Executar inferência
    /// 4. Armazenar na memória
    /// 5. Atualizar histórico
    pub async fn process(&self, user_input: &str) -> ArkheResult<String> {
        let start = std::time::Instant::now();
        let session_id = {
            let h = self.history.read().await;
            h.session_id().to_string()
        };

        info!(session = %session_id, "Processing input");

        // 1. Verificação de segurança
        let phase_start = std::time::Instant::now();
        let verdict = self.safety.verify("llm_inference", user_input).await;
        if let SafetyVerdict::Rejected(reason) = verdict {
            warn!(session = %session_id, reason = %reason, "Safety rejected");
            return Err(arkhe_core::ArkheError::PermissionDenied(reason));
        }
        let safety_ms = phase_start.elapsed().as_millis() as u64;

        // 2. Construir requisição com histórico
        let mut messages = vec![ChatMessage::system(&self.system_prompt)];
        {
            let h = self.history.read().await;
            messages.extend(h.to_vec());
        }
        messages.push(ChatMessage::user(user_input));

        let request = InferenceRequest {
            messages,
            params: arkhe_inference::SamplingParams::default(),
            tools: self.tools.definitions().await,
            session_id: Some(session_id.clone()),
        };

        // 3. Inferência
        let phase_start = std::time::Instant::now();
        let mut response = self.inference.complete(request).await
            .map_err(|e| arkhe_core::ArkheError::Internal(e.to_string()))?;
        let inference_ms = phase_start.elapsed().as_millis() as u64;

        // 3a. Executor: se o motor pediu ferramentas, executa cada uma e
        // anexa o resultado ao conteúdo da resposta. Nenhum InferenceEngine
        // real está conectado ainda que de fato emita tool_calls
        // (NullEngine só ecoa) — este caminho existe para quando um
        // motor real fizer isso; ver crates/arkhe-agi/src/executor.rs.
        for call in &response.tool_calls {
            match self.tools.execute(call).await {
                Ok(result) => {
                    response.content = format!("{}\n\n[tool:{}] {}", response.content, call.name, result);
                }
                Err(err) => {
                    warn!(tool = %call.name, error = %err, "Tool execution failed");
                    response.content = format!("{}\n\n[tool:{} failed] {}", response.content, call.name, err);
                }
            }
        }

        let now = chrono::Utc::now();

        // 3b. Verificação geométrica (FI-120) + registro de proveniência
        // (FI-011/FI-017): rejeita turnos com resposta vazia antes de
        // admiti-los na cadeia de evidências.
        let phase_start = std::time::Instant::now();
        let attested_by = self
            .attestor
            .read()
            .await
            .as_ref()
            .map(|v| format!("{} ({})", v.display_name, v.id));
        let turn = TurnRecord { user_input: user_input.to_string(), response: response.content.clone(), attested_by };
        let artifact = self.geometric.verify(&turn)
            .map_err(|e| arkhe_core::ArkheError::Internal(e.to_string()))?;
        self.evidence.append(artifact.canonical_bytes, now.timestamp() as u64).await;
        let geometric_ms = phase_start.elapsed().as_millis() as u64;

        // 4. Armazenar na memória
        let phase_start = std::time::Instant::now();
        let turn_key = format!("{}:turn-{}", session_id, now.timestamp_millis());
        let episode_key = format!("{}:ep:{}", session_id, uuid::Uuid::new_v4());

        let working_entry = MemoryEntry {
            key: turn_key.clone(),
            value: format!("User: {}\nAssistant: {}", user_input, response.content),
            score: if response.finish_reason == FinishReason::Stop { 0.9 } else { 0.5 },
            layer: MemoryLayer::Working,
            timestamp: now,
        };
        if let Err(e) = self.memory.store(working_entry).await {
            warn!(error = %e, "Failed to store in working memory");
        }

        let episode_entry = MemoryEntry {
            key: episode_key.clone(),
            value: format!("Query: {}", user_input),
            score: 0.7,
            layer: MemoryLayer::Episodic,
            timestamp: now,
        };
        if let Err(e) = self.memory.store(episode_entry).await {
            warn!(error = %e, "Failed to store in episodic memory");
        }

        // Registra a associação working <-> episodic para o grafo de
        // memória (FI-124).
        self.memory_links.write().await.push((turn_key.clone(), episode_key, 1.0));
        let memory_ms = phase_start.elapsed().as_millis() as u64;

        // 5. Atualizar histórico
        {
            let mut h = self.history.write().await;
            h.push_turn(user_input, &response.content, response.usage);
        }

        let duration = start.elapsed();
        let timings = PhaseTimings {
            safety_ms,
            inference_ms,
            geometric_ms,
            memory_ms,
            total_ms: duration.as_millis() as u64,
        };
        self.metrics.record(timings).await;

        info!(
            session = %session_id,
            duration_ms = timings.total_ms,
            safety_ms = timings.safety_ms,
            inference_ms = timings.inference_ms,
            geometric_ms = timings.geometric_ms,
            memory_ms = timings.memory_ms,
            tokens = response.usage.total_tokens,
            "Response generated"
        );

        Ok(response.content)
    }

    /// Decomposes `user_input` via [`planner::decompose`] into an ordered
    /// [`crate::planner::Plan`], then runs each step through
    /// [`Self::process`] in sequence, returning each step's response in
    /// order. Not an LLM-based planner — see the [`planner`] module docs
    /// for why: this is deterministic sentence-boundary splitting, so a
    /// plain single-sentence input decomposes to exactly one step and
    /// behaves identically to calling [`Self::process`] directly. Each
    /// step becomes its own fully-attested turn — its own safety check,
    /// inference call, geometric verification, and provenance/memory
    /// record — rather than one opaque blob covering the whole input.
    ///
    /// If any step fails (e.g. safety rejection), processing stops and
    /// the error is returned; steps already completed keep their history/
    /// provenance/memory records, they are not rolled back.
    pub async fn process_planned(&self, user_input: &str) -> ArkheResult<Vec<String>> {
        let plan = planner::decompose(user_input);
        let mut responses = Vec::with_capacity(plan.steps.len());
        for step in &plan.steps {
            responses.push(self.process(&step.text).await?);
        }
        Ok(responses)
    }

    /// Avalia a sessão atual.
    pub async fn evaluate_session(&self) -> ArkheResult<arkhe_session_evaluator::EvaluationResult> {
        let h = self.history.read().await;
        let trajectory = SessionTrajectory {
            id: h.session_id().to_string(),
            agent_did: "arkhe:agi".to_string(),
            turns: h.messages().iter().enumerate().map(|(i, m)| {
                arkhe_session_evaluator::Turn {
                    id: format!("t{}", i),
                    role: match m.role {
                        ChatRole::System => arkhe_session_evaluator::TurnRole::System,
                        ChatRole::User => arkhe_session_evaluator::TurnRole::User,
                        ChatRole::Assistant => arkhe_session_evaluator::TurnRole::Assistant,
                        ChatRole::Tool => arkhe_session_evaluator::TurnRole::Tool,
                    },
                    content: m.content.clone(),
                    timestamp: chrono::Utc::now(),
                    reasoning: None,
                    validation: None,
                    artifacts: Vec::new(),
                    confidence: None,
                    causal_links: Vec::new(),
                }
            }).collect(),
            artifacts: Vec::new(),
            start_time: chrono::Utc::now(),
            end_time: Some(chrono::Utc::now()),
            metadata: std::collections::HashMap::new(),
        };
        drop(h);

        Ok(self.evaluator.evaluate(&trajectory).await)
    }

    /// Retorna estatísticas da sessão.
    pub async fn stats(&self) -> (String, u64, arkhe_inference::TokenUsage) {
        let h = self.history.read().await;
        (h.session_id().to_string(), h.turn_count(), *h.total_usage())
    }

    /// Per-phase latency over the most recent turns processed so far —
    /// see [`crate::metrics::PhaseTimings`] for what each phase covers.
    pub async fn latency_stats(&self) -> LatencyStats {
        self.metrics.stats().await
    }

    /// Builds and validates the provenance graph (FI-122) over every turn
    /// processed so far, from the tamper-evident evidence chain each
    /// `process()` call appends to. `Ok` implies the graph is acyclic and
    /// every node's `prev_hash` agrees with its predecessor's `hash`.
    pub async fn provenance_graph(&self) -> Result<TypedGraph<ProvenanceNode, ProvenanceEdge>, ProvenanceError> {
        provenance_graph::build_from_chain(&self.evidence).await
    }

    /// Builds the memory graph (FI-124) over the working/episodic key
    /// associations recorded by every `process()` call so far.
    pub async fn memory_graph(&self) -> Result<TypedGraph<MemoryNode, MemoryEdge>, MemoryGraphError> {
        let links = self.memory_links.read().await.clone();
        memory_graph::build_from_memory(self.memory.as_ref(), &links).await
    }
}
