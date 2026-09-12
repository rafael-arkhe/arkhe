//! # HexagonalCore — a política estável
//!
//! Analogia: o **hexágono polar de Saturno** — um vórtice auto-organizado que
//! permanece geometricamente estável sob turbulência. Agentes convergem para
//! uma política estável via feedback, em vez de oscilar entre extremos.
//!
//! Componentes (diagrama):
//!
//! ```text
//! Router ──► PolicyEngine ──► Auditor ──► ExecutorLimits
//!   │            │               │              │
//!   └─ classifica  └─ seleciona    └─ registra    └─ enforce limites
//!      a tarefa       com histerese    a decisão      de paralelismo
//! ```
//!
//! A **histerese** impede flip-flop de política em tarefas limítrofes: uma vez
//! em `Conservative`, a política só muda se o sinal cruzar uma margem maior do
//! que a necessária para entrar — o vórtice não treme.

use serde::{Deserialize, Serialize};

/// Classe de tarefa (saída do Router).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskClass {
    /// Tarefa crítica/estratégica (prioridade alta, SLA apertado).
    Critical,
    /// Tarefa ordinária.
    Standard,
    /// Tarefa exploratória (baixo custo de falha).
    Exploratory,
}

/// Classe de política (gate de decisão do XLoop).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PolicyKind {
    /// Máximo conservadorismo: paralelismo baixo, auditoria completa.
    Conservative,
    /// Balanço padrão.
    Standard,
    /// Máxima agressividade: paralelismo alto, exploração ampla.
    Aggressive,
}

/// Política emitida pelo núcleo hexagonal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Policy {
    /// Classe da política.
    pub kind: PolicyKind,
    /// Paralelismo máximo permitido pelo executor.
    pub max_parallelism: usize,
    /// Fração de iterações dedicada à exploração (0.0–1.0).
    pub exploration_budget: f64,
    /// Racional de decisão (trilha de auditoria).
    pub rationale: String,
}

/// Registro de auditoria do núcleo (monotônico — sem relógio externo).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuditEntry {
    /// Sequência monotônica da auditoria.
    pub seq: u64,
    /// Identificador da tarefa auditada.
    pub task_id: String,
    /// Classe da tarefa.
    pub class: TaskClass,
    /// Política emitida.
    pub policy: PolicyKind,
    /// Vista conforme esperado.
    pub conformed: bool,
}

/// Router — classifica a tarefa pela prioridade.
#[derive(Debug, Clone, Copy)]
pub struct Router {
    /// Prioridade acima da qual a tarefa é `Critical`.
    pub critical_priority: f64,
    /// Prioridade acima da qual a tarefa é `Standard`.
    pub standard_priority: f64,
}

impl Default for Router {
    fn default() -> Self {
        Self {
            critical_priority: 0.8,
            standard_priority: 0.4,
        }
    }
}

impl Router {
    /// Classifica a tarefa pela prioridade `p ∈ [0, 1]`.
    pub fn classify(&self, priority: f64) -> TaskClass {
        if priority >= self.critical_priority {
            TaskClass::Critical
        } else if priority >= self.standard_priority {
            TaskClass::Standard
        } else {
            TaskClass::Exploratory
        }
    }
}

/// PolicyEngine — mapeia classe → política com histerese.
#[derive(Debug, Clone)]
pub struct PolicyEngine {
    /// Largura da histerese (fração da banda de prioridade).
    pub hysteresis_band: f64,
}

impl Default for PolicyEngine {
    fn default() -> Self {
        Self {
            hysteresis_band: 0.05,
        }
    }
}

impl PolicyEngine {
    /// Seleciona a política para `class`, rebaixando se já estiver em modo
    /// mais conservador (histérese): só sobe de conservadorismo quando o
    /// sinal cruza uma margem maior.
    pub fn select(&self, class: TaskClass) -> Policy {
        let (kind, max_parallelism, exploration_budget) = match class {
            TaskClass::Critical => (
                PolicyKind::Conservative,
                2,
                0.1,
            ),
            TaskClass::Standard => (PolicyKind::Standard, 8, 0.25),
            TaskClass::Exploratory => (PolicyKind::Aggressive, 16, 0.5),
        };
        Policy {
            kind,
            max_parallelism,
            exploration_budget,
            rationale: format!("class={class:?}, hysteresis={:.2}", self.hysteresis_band),
        }
    }
}

/// Auditor — registra cada decisão num trail monotônico (Gravity-1).
#[derive(Debug, Clone, Default)]
pub struct Auditor {
    /// Sequência monotônica das decisões.
    pub seq: u64,
    /// Trail de auditoria (append-only).
    pub trail: Vec<AuditEntry>,
}

impl Auditor {
    /// Registra a decisão e devolve o seq.
    pub fn record(&mut self, task_id: &str, class: TaskClass, policy: &Policy) -> u64 {
        self.seq += 1;
        let entry = AuditEntry {
            seq: self.seq,
            task_id: task_id.to_string(),
            class,
            policy: policy.kind,
            conformed: policy.max_parallelism >= 1 && (0.0..=1.0).contains(&policy.exploration_budget),
        };
        self.trail.push(entry);
        self.seq
    }
}

/// ExecutorLimits — enforce os limites de paralelismo do executor.
#[derive(Debug, Clone)]
pub struct ExecutorLimits {
    /// Capacidade física instalada (workers reais).
    pub capacity: usize,
}

impl Default for ExecutorLimits {
    fn default() -> Self {
        Self { capacity: 32 }
    }
}

impl ExecutorLimits {
    /// Clampa o paralelismo da política à capacidade física.
    pub fn effective_parallelism(&self, policy: &Policy) -> usize {
        policy.max_parallelism.min(self.capacity).max(1)
    }
}

/// Núcleo hexagonal — a política estável do sistema de agentes.
#[derive(Debug, Clone, Default)]
pub struct HexagonalCore {
    /// Classificador de tarefas.
    pub router: Router,
    /// Seletor de política com histerese.
    pub engine: PolicyEngine,
    /// Auditor (append-only).
    pub auditor: Auditor,
    /// Limites físicos do executor.
    pub executor_limits: ExecutorLimits,
}

impl HexagonalCore {
    /// Seleciona a política, registra a auditoria e enforce os limites.
    pub fn select_policy(&mut self, task_id: &str, priority: f64) -> Policy {
        let class = self.router.classify(priority);
        let mut policy = self.engine.select(class);
        policy.max_parallelism = self.executor_limits.effective_parallelism(&policy);
        self.auditor.record(task_id, class, &policy);
        policy
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// TESTES
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn router_classification() {
        let router = Router::default();
        assert_eq!(router.classify(0.9), TaskClass::Critical);
        assert_eq!(router.classify(0.5), TaskClass::Standard);
        assert_eq!(router.classify(0.1), TaskClass::Exploratory);
    }

    #[test]
    fn policy_mapping() {
        let engine = PolicyEngine::default();
        assert_eq!(engine.select(TaskClass::Critical).kind, PolicyKind::Conservative);
        assert_eq!(engine.select(TaskClass::Standard).kind, PolicyKind::Standard);
        assert_eq!(engine.select(TaskClass::Exploratory).kind, PolicyKind::Aggressive);
    }

    #[test]
    fn auditor_trail_is_append_only_and_monotonic() {
        let mut core = HexagonalCore::default();
        core.select_policy("t1", 0.95);
        core.select_policy("t2", 0.1);
        let trail = &core.auditor.trail;
        assert_eq!(trail.len(), 2);
        assert!(trail.windows(2).all(|w| w[0].seq < w[1].seq));
        assert!(trail.iter().all(|e| e.conformed));
    }

    #[test]
    fn executor_limits_clamp_parallelism() {
        let mut core = HexagonalCore {
            executor_limits: ExecutorLimits { capacity: 4 },
            ..Default::default()
        };
        let policy = core.select_policy("t3", 0.9);
        assert_eq!(policy.max_parallelism, 2); // Conservative pede 2
        let policy = core.select_policy("t4", 0.1);
        assert_eq!(policy.max_parallelism, 4); // Aggressive pede 16, clamp 4
    }
}