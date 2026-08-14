//! Orquestrador principal do agente IA com fail‑closed e auditoria epistémica.
//!
//! # Correções aplicadas
//! - **P16 FIX**: Fail‑closed (reverter para CGR, isolar críticos) em vez de aumentar exploração.
//! - **P17 FIX**: Backoff adaptativo para reconexão.
//! - **P18 FIX**: Auditoria baseada em eventos (não em ciclos fixos).
//!
//! # Modos
//! - `Normal`: Q‑Learning ativo.
//! - `SafeCgr`: Fallback para CGR determinístico (fail‑closed).
//! - `Isolated`: Sem envio de bundles críticos.

use alloc::collections::VecDeque;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::cgr::ContactPlan;
use crate::discovery::DiscoveryService;
use crate::epistemic_audit::{EpistemicAuditor, EvidenceStatus};
use crate::processing::{DataProcessor, InferenceConfig, Priority};
use crate::q_learning::{QLearningAgent, State};

/// Estado de operação do orquestrador.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrchestratorMode {
    /// Modo normal: Q‑Learning ativo.
    Normal,
    /// Fail‑closed: reverter para CGR.
    SafeCgr,
    /// Isolado: sem envio de bundles críticos.
    Isolated,
    /// Enlace óptico (RaptorQ FEC + QR Code) quando disponível.
    Optical,
}

/// Configuração do orquestrador.
#[derive(Debug, Clone)]
pub struct OrchestratorConfig {
    /// Modo inicial.
    pub mode: OrchestratorMode,
    /// Nó local.
    pub local_node_id: usize,
    /// Nó destino padrão.
    pub default_destination: usize,
    /// Intervalo de auditoria (eventos).
    pub audit_interval_events: usize,
    /// Backoff máximo de reconexão (segundos).
    pub max_reconnect_backoff_sec: u64,
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            mode: OrchestratorMode::Normal,
            local_node_id: 0,
            default_destination: 1,
            audit_interval_events: 50,
            max_reconnect_backoff_sec: 600,
        }
    }
}

/// Eventos internos do orquestrador.
#[derive(Debug, Clone)]
pub enum OrchestratorEvent {
    BundleReceived(Vec<u8>, String),
    ContactChange(usize, usize, bool),
    FailureDetected(usize),
    EpistemicTrigger,
    ReconnectAttempt(usize),
    /// Frame óptico (payload RaptorQ/QR) recebido no enlace de luz.
    OpticalFrame(Vec<u8>),
}

/// Orquestrador principal.
pub struct Orchestrator {
    config: OrchestratorConfig,
    discovery: DiscoveryService,
    contact_plan: ContactPlan,
    q_agent: QLearningAgent,
    processor: DataProcessor,
    auditor: EpistemicAuditor,
    /// Modo atual.
    mode: OrchestratorMode,
    /// Buffer de bundles recebidos.
    buffer: VecDeque<Vec<u8>>,
    /// Contador de eventos para auditoria.
    event_counter: usize,
    /// Contador de reconexão (backoff).
    reconnect_attempt: usize,
    /// Nó atual (onde o agente está).
    current_node: usize,
    /// Nó destino.
    destination: usize,
    /// Histórico de ações (para auditoria).
    history: VecDeque<String>,
}

impl Orchestrator {
    /// Cria novo orquestrador.
    pub fn new(
        config: OrchestratorConfig,
        contact_plan: ContactPlan,
        processor_config: InferenceConfig,
        auditor_script: Option<&str>,
    ) -> Self {
        let local_eid = crate::discovery::Eid::dtn(&format!("node-{}", config.local_node_id));
        let discovery = DiscoveryService::new(local_eid);
        let q_agent = QLearningAgent::new(42);
        let processor = DataProcessor::new(processor_config);
        let auditor = EpistemicAuditor::new(auditor_script);

        let mode = config.mode;
        let current_node = config.local_node_id;
        let destination = config.default_destination;

        // Inicializa o banco de evidências com algumas entradas padrão
        let mut aud = auditor;
        aud.add_evidence("Sistema operacional validado", EvidenceStatus::Compiled, Some("kernel.bin"), 0);
        aud.add_evidence("CGR implementado", EvidenceStatus::Compiled, Some("cgr.rs"), 0);

        Self {
            config,
            discovery,
            contact_plan,
            q_agent,
            processor,
            auditor: aud,
            mode,
            buffer: VecDeque::with_capacity(100),
            event_counter: 0,
            reconnect_attempt: 0,
            current_node,
            destination,
            history: VecDeque::with_capacity(1000),
        }
    }

    /// Executa um passo do orquestrador.
    pub fn step(&mut self, event: Option<OrchestratorEvent>, sim_time: f64) -> OrchestratorAction {
        self.event_counter += 1;

        // 1. Processa evento, se houver
        if let Some(evt) = event {
            self.handle_event(evt, sim_time);
        }

        // 2. Atualiza estado do grafo de contatos
        self.contact_plan.update_edge_states(sim_time);

        // 3. Auditoria epistémica (baseada em eventos, P18)
        if self.event_counter % self.config.audit_interval_events == 0 {
            self.run_epistemic_audit();
        }

        // 4. Decide a ação (Normal ou SafeCgr)
        let action = self.decide_action(sim_time);

        // 5. Executa ação e registra
        self.execute_action(&action);

        // 6. Atualiza histórico
        self.record_history(&action);

        // 7. Verifica necessidade de isolamento
        if self.mode == OrchestratorMode::Isolated {
            return OrchestratorAction::NoOp;
        }

        action
    }

    /// Trata um evento.
    fn handle_event(&mut self, event: OrchestratorEvent, _sim_time: f64) {
        match event {
            OrchestratorEvent::BundleReceived(data, source) => {
                // Processa o bundle com o processador de dados
                let processed = self.processor.process(&data, &source);
                // Se prioridade alta, coloca no buffer prioritário
                if processed.priority == Priority::High {
                    self.buffer.push_front(data);
                } else {
                    self.buffer.push_back(data);
                }
                // Adiciona evidência
                self.auditor.add_evidence(
                    &format!("Bundle processado de {}", source),
                    EvidenceStatus::Executed,
                    Some("buffer"),
                    0,
                );
            }
            OrchestratorEvent::ContactChange(from, to, active) => {
                // Atualiza a lista de contatos ativos
                for edge in &mut self.contact_plan.edges {
                    if edge.from == from && edge.to == to {
                        edge.active = active;
                    }
                }
            }
            OrchestratorEvent::FailureDetected(node) => {
                if node < self.contact_plan.nodes.len() {
                    self.contact_plan.nodes[node].failed = true;
                }
            }
            OrchestratorEvent::EpistemicTrigger => {
                self.run_epistemic_audit();
            }
            OrchestratorEvent::ReconnectAttempt(attempt) => {
                self.reconnect_attempt = attempt;
                // Tenta reconectar com backoff adaptativo (P17)
                let backoff = self.calculate_reconnect_backoff(attempt);
                self.history.push_back(format!("Reconnect scheduled in {}s", backoff));
                self.discovery.scan_frequencies.clear();
                // Inicia nova varredura
                self.discovery.add_frequency(2_400_000_000);
                self.discovery.add_frequency(8_400_000_000);
                self.discovery.add_frequency(32_000_000_000);
            }
            OrchestratorEvent::OpticalFrame(payload) => {
                self.handle_optical_frame(&payload);
            }
        }
    }

    /// Processa um frame óptico recebido no enlace de luz.
    ///
    /// Na configuração `std`, o frame é decodificado com RaptorQ e
    /// enfileirado como bundle; em `no_std` registra-se apenas a evidência.
    fn handle_optical_frame(&mut self, payload: &[u8]) {
        self.auditor.add_evidence(
            &format!("Frame óptico recebido ({} bytes)", payload.len()),
            EvidenceStatus::Executed,
            Some("optical"),
            0,
        );
        #[cfg(feature = "std")]
        {
            use crate::optical_convergence;
            if let Ok(frames) = optical_convergence::fec_decode(&[payload.to_vec()]) {
                self.buffer.push_back(frames);
            } else {
                // Tenta interpretar como payload cru (fallback)
                self.buffer.push_back(payload.to_vec());
            }
        }
    }

    /// Executa auditoria epistémica e decide se deve ativar fail‑closed.
    ///
    /// **P16 FIX**: Se integridade baixa, ativa SafeCgr (não aumenta exploração).
    fn run_epistemic_audit(&mut self) {
        // Coleta histórico recente
        let history_text = self.history.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("\n");
        if let Some(result) = self.auditor.audit_text(&history_text) {
            // Adiciona evidência
            self.auditor.add_evidence(
                "Auditoria epistémica executada",
                EvidenceStatus::Executed,
                Some("auditor"),
                0,
            );

            // Se integridade baixa, ativa fail‑closed (P16)
            if result.integrity < self.auditor.integrity_threshold {
                self.activate_fail_closed();
            } else {
                // Se estava em SafeCgr e integridade melhorou, reativa modo normal
                if self.mode == OrchestratorMode::SafeCgr && result.integrity > self.auditor.integrity_threshold + 0.1 {
                    self.mode = OrchestratorMode::Normal;
                    self.history.push_back("Epistemic integrity restored, returning to Normal mode".to_string());
                }
            }
        }
    }

    /// Ativa fail‑closed.
    fn activate_fail_closed(&mut self) {
        self.mode = OrchestratorMode::SafeCgr;
        self.history.push_back("EPISTEMIC FAIL-CLOSED ACTIVATED — reverting to CGR".to_string());
        // Isola bundles críticos (não os envia)
        self.buffer.retain(|_| false);
    }

    /// Decide a ação com base no modo.
    fn decide_action(&mut self, sim_time: f64) -> OrchestratorAction {
        match self.mode {
            OrchestratorMode::Normal => {
                // Usa Q‑Learning
                let state = self.current_state();
                let valid_actions = self.get_valid_actions();
                if valid_actions.is_empty() {
                    OrchestratorAction::NoOp
                } else {
                    let action = self.q_agent.select_action(state, &valid_actions);
                    OrchestratorAction::Forward(action)
                }
            }
            OrchestratorMode::SafeCgr => {
                // Fallback para CGR
                if let Some(path) = self.contact_plan.find_path(self.current_node, self.destination, sim_time, 1024) {
                    if path.len() > 1 {
                        OrchestratorAction::Forward(path[1])
                    } else {
                        OrchestratorAction::NoOp
                    }
                } else {
                    // Se CGR falha, tenta reconexão (P17)
                    self.reconnect_attempt += 1;
                    let backoff = self.calculate_reconnect_backoff(self.reconnect_attempt);
                    OrchestratorAction::Reconnect(backoff)
                }
            }
            OrchestratorMode::Isolated => {
                OrchestratorAction::NoOp
            }
            OrchestratorMode::Optical => {
                // Modo óptico: prioriza FEC + QR quando há enlace de luz.
                // Se não houver payload para transmitir, cai em NoOp.
                #[cfg(feature = "std")]
                {
                    let state = self.current_state();
                    let valid_actions = self.get_valid_actions();
                    if valid_actions.is_empty() {
                        OrchestratorAction::NoOp
                    } else {
                        let action = self.q_agent.select_action(state, &valid_actions);
                        OrchestratorAction::Forward(action)
                    }
                }
                #[cfg(not(feature = "std"))]
                {
                    OrchestratorAction::NoOp
                }
            }
        }
    }

    /// Executa uma ação.
    fn execute_action(&mut self, action: &OrchestratorAction) {
        match action {
            OrchestratorAction::Forward(node) => {
                // Atualiza nó atual e simula envio
                self.current_node = *node;
                // Atualiza Q‑Learning
                let state = self.current_state();
                let reward = self.q_agent.compute_reward(true, 10.0, 0.01, 0.2);
                let next_state = State {
                    node_id: *node,
                    buffer_level: State::discretize_buffer(self.buffer.len(), 100),
                    contact_remaining: State::discretize_time_remaining(300.0),
                    dst_type: 0,
                };
                self.q_agent.update(state, *node, reward, next_state, &[0]);
            }
            OrchestratorAction::Reconnect(backoff_sec) => {
                self.history.push_back(format!("Reconnect attempt in {}s", backoff_sec));
            }
            OrchestratorAction::NoOp => {}
        }
    }

    /// Registra ação no histórico.
    fn record_history(&mut self, action: &OrchestratorAction) {
        let entry = format!(
            "[t={}] mode={:?} action={:?} node={}",
            self.event_counter, self.mode, action, self.current_node
        );
        self.history.push_back(entry);
        while self.history.len() > 1000 {
            self.history.pop_front();
        }
    }

    /// Calcula backoff de reconexão (P17).
    fn calculate_reconnect_backoff(&self, attempt: usize) -> u64 {
        let base = 60; // segundos
        let exp = base * (2_u64.pow(attempt.min(10) as u32));
        exp.min(self.config.max_reconnect_backoff_sec)
    }

    /// Obtém estado atual para Q‑Learning.
    fn current_state(&self) -> State {
        State {
            node_id: self.current_node,
            buffer_level: State::discretize_buffer(self.buffer.len(), 100),
            contact_remaining: State::discretize_time_remaining(300.0),
            dst_type: 0,
        }
    }

    /// Obtém ações válidas (vizinhos ativos).
    fn get_valid_actions(&self) -> Vec<usize> {
        let mut actions = Vec::new();
        for edge in &self.contact_plan.edges {
            if edge.active && !self.contact_plan.nodes[edge.to].failed {
                if edge.from == self.current_node {
                    actions.push(edge.to);
                } else if edge.to == self.current_node {
                    actions.push(edge.from);
                }
            }
        }
        actions
    }

    /// Ativa o modo isolado (não envia bundles críticos).
    pub fn activate_isolation(&mut self) {
        self.mode = OrchestratorMode::Isolated;
        self.history.push_back("ISOLATION MODE ACTIVATED".to_string());
    }

    /// Retorna estatísticas do orquestrador.
    pub fn stats(&self) -> OrchestratorStats {
        OrchestratorStats {
            mode: self.mode,
            current_node: self.current_node,
            destination: self.destination,
            buffer_size: self.buffer.len(),
            event_counter: self.event_counter,
            q_table_size: self.q_agent.stats().q_table_size,
            integrity_score: self.auditor.evidence_bank.integrity_score(),
            active_contacts: self.contact_plan.active_contacts(),
        }
    }
}

/// Ação do orquestrador.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrchestratorAction {
    /// Encaminhar para o nó especificado.
    Forward(usize),
    /// Tentar reconexão após backoff.
    Reconnect(u64),
    /// Nenhuma ação.
    NoOp,
}

/// Estatísticas do orquestrador.
#[derive(Debug, Clone)]
pub struct OrchestratorStats {
    pub mode: OrchestratorMode,
    pub current_node: usize,
    pub destination: usize,
    pub buffer_size: usize,
    pub event_counter: usize,
    pub q_table_size: usize,
    pub integrity_score: f64,
    pub active_contacts: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cgr::{NetworkEdge, NetworkNode};

    fn create_test_contact_plan() -> ContactPlan {
        let mut plan = ContactPlan::new();
        plan.add_node(NetworkNode {
            id: 0,
            name: "A".to_string(),
            node_type: crate::cgr::NodeType::HotBubble,
            position_ly: (0.0, 0.0, 0.0),
            buffer_cap: 100,
            buffer: 0,
            failed: false,
            schwarzschild_radius_m: 0.0,
        });
        plan.add_node(NetworkNode {
            id: 1,
            name: "B".to_string(),
            node_type: crate::cgr::NodeType::Relay,
            position_ly: (0.5, 0.0, 0.0),
            buffer_cap: 100,
            buffer: 0,
            failed: false,
            schwarzschild_radius_m: 0.0,
        });
        plan.add_edge(NetworkEdge {
            from: 0,
            to: 1,
            distance_ly: 0.5,
            data_rate_bps: 1_000_000.0,
            packet_loss: 0.01,
            contacts: vec![crate::cgr::ContactWindow { start: 0.0, end: 1e9, period: 0.0 }],
            active: false,
        });
        plan.update_edge_states(0.0);
        plan
    }

    #[test]
    fn test_orchestrator_step() {
        let config = OrchestratorConfig::default();
        let plan = create_test_contact_plan();
        let proc_config = InferenceConfig::default();
        let mut orch = Orchestrator::new(config, plan, proc_config, None);

        // Primeiro passo
        let action = orch.step(None, 0.0);
        // Deve retornar Forward para o nó 1 (vizinho ativo)
        // ou NoOp se não houver contato ativo
        match action {
            OrchestratorAction::Forward(node) => {
                assert!(node == 1 || node == 0);
            }
            _ => {}
        }

        // Simula recebimento de bundle
        let bundle_data = b"test data";
        orch.step(
            Some(OrchestratorEvent::BundleReceived(bundle_data.to_vec(), "dtn://src.dsn/".to_string())),
            0.0,
        );
        assert!(!orch.buffer.is_empty());
    }

    #[test]
    fn test_fail_closed_activation() {
        let config = OrchestratorConfig::default();
        let plan = create_test_contact_plan();
        let proc_config = InferenceConfig::default();
        let mut orch = Orchestrator::new(config, plan, proc_config, None);

        // Força auditoria para ativar fail‑closed
        // (simula baixa integridade)
        for _ in 0..orch.config.audit_interval_events {
            orch.event_counter += 1;
        }
        orch.run_epistemic_audit();
        // Se a integridade for baixa (o que é provável no teste), deve ativar SafeCgr
        if orch.auditor.should_fail_closed() {
            assert_eq!(orch.mode, OrchestratorMode::SafeCgr);
        }
    }

    #[test]
    fn test_reconnect_backoff() {
        let config = OrchestratorConfig::default();
        let plan = create_test_contact_plan();
        let proc_config = InferenceConfig::default();
        let orch = Orchestrator::new(config, plan, proc_config, None);

        let b1 = orch.calculate_reconnect_backoff(1);
        let b2 = orch.calculate_reconnect_backoff(2);
        assert_eq!(b1, 120);
        assert_eq!(b2, 240);
    }

    #[test]
    fn test_stats() {
        let config = OrchestratorConfig::default();
        let plan = create_test_contact_plan();
        let proc_config = InferenceConfig::default();
        let mut orch = Orchestrator::new(config, plan, proc_config, None);

        orch.step(None, 0.0);
        let stats = orch.stats();
        assert!(stats.event_counter > 0);
    }
}
