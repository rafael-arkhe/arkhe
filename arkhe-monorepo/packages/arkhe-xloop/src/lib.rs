//! # arkhe-xloop — O loop de execução como campo métrico
//!
//! Implementa a **Equação de Kronos** para o ARKHE OS:
//!
//! ```text
//! ds² = -c² dt² + τ_eff · dℓ²
//! Continar se dτ_eff / dℓ > 0
//! ```
//!
//! - [`temporal_field::TemporalField`] — a métrica do loop (dilatação,
//!   curvatura, energia, tempo próprio). O loop não é um conjunto de regras
//!   discretas (`max_iterations`, `timeout`): é uma **geodésica** no campo.
//! - [`objective_field::ObjectiveField`] — a correção de Carlip: o colapso
//!   temporal requer **divergência de objetivos** (`E_Δ ≠ 0`).
//! - [`XLoop`] — o orquestrador que percorre a geodésica e decide
//!   continuar/avisar/colapsar.
//!
//! Decisão: ADR-002 (`docs/ADR-002-xloop-metric-field.md`).

pub mod objective_field;
pub mod temporal_field;

pub use objective_field::ObjectiveField;
pub use temporal_field::{CollapseAction, GeodesicDecision, TemporalField, TemporalFieldSnapshot};

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Backstop discreto do campo (mitigação do ADR-002): mesmo se a dilatação
/// proteger o loop de colapsar, um watchdog interrompe após
/// `HARD_STEP_CAP_MULTIPLIER · estimated_steps` passos.
pub const HARD_STEP_CAP_MULTIPLIER: u64 = 4;

/// Chave de estado usada quando o executor falha.
pub const ERROR_STATE_KEY: &str = "__executor_error__";

/// Descrição serializada de uma tarefa entregue ao XLoop.
///
/// `estimated_steps` é a previsão de passos (análoga ao tempo objetivo);
/// o campo temporal converte essa previsão em `dℓ` logarítmica.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    /// Identificador único da tarefa.
    pub id: String,
    /// Nome legível da tarefa.
    pub name: String,
    /// Tenant de origem (para calibração por SLA).
    pub tenant: String,
    /// Previsão de passos (`total_steps` na distância).
    pub estimated_steps: u64,
    /// Especialidade desejada (para integração com `arkhe-orbital`).
    pub specialty: Option<String>,
    /// Parâmetros arbitrários da tarefa.
    pub parameters: std::collections::HashMap<String, serde_json::Value>,
}

impl Task {
    /// Cria uma tarefa simples com `estimated_steps`.
    pub fn simple(id: &str, estimated_steps: u64) -> Self {
        Self {
            id: id.to_string(),
            name: id.to_string(),
            tenant: "default".to_string(),
            estimated_steps: estimated_steps.max(1),
            specialty: None,
            parameters: std::collections::HashMap::new(),
        }
    }
}

/// Resultado de um passo do executor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepResult {
    /// Chave semântica do estado alcançado (para detecção de curvatura).
    pub state_key: String,
    /// `true` quando o passo é terminal (tarefa concluída).
    pub terminal: bool,
}

/// Erro de um passo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepError(pub String);

/// Contrato do executor de passos.
///
/// Mantido síncrono de propósito (Simplicity-2): o XLoop orquestra o loop e a
/// latência de cada passo alimenta o campo. Executores assíncronos reais devem
/// ser adaptados a este contrato via uma ponte (`StepExecutor` ↔ trait async).
pub trait StepExecutor: Send + Sync {
    /// Executa um passo; retorna o próximo estado.
    fn step(&self, task: &Task) -> Result<StepResult, StepError>;
    /// Nome do executor (para trilha de auditoria).
    fn name(&self) -> &'static str {
        "anonymous"
    }
}

/// Erros do XLoop.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// O campo temporal dobrou sobre si — Kronos devorou o loop.
    #[error("temporal collapse: {reason}")]
    TemporalCollapse {
        /// Por que o campo colapsou.
        reason: String,
    },
    /// Curvatura crítica persistente (loop infinito confirmado).
    #[error("infinite loop detected by temporal field")]
    TemporalLoopDetected,
    /// Colapso por escalação — intervenção humana solicitada.
    #[error("escalation required: {reason}")]
    EscalationRequired {
        /// Por que a escalação foi solicitada.
        reason: String,
    },
    /// Watchdog separado (mitigação ADR-002): teto duro de passos.
    #[error("hard step cap reached ({cap} steps) — watchdog")]
    HardStepCapReached {
        /// Teto duro atingido.
        cap: u64,
    },
    /// Falha do executor de passos.
    #[error("executor step failed: {0}")]
    ExecutorStep(String),
}

/// Por que o loop terminou antes do normal (além de `terminal`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TruncationReason {
    /// Colapso geodésico por expansão temporal — tentou fallback e saiu.
    FellBack,
    /// Watchdog separado atingiu o teto duro.
    HardCap,
}

/// Resultado da execução de uma tarefa.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    /// Identificador da tarefa.
    pub task_id: String,
    /// Passos completados.
    pub completed_steps: u64,
    /// Tempo objetivo decorrido (ms).
    pub elapsed_ms: u64,
    /// Razão de truncamento, se houve.
    pub truncation: Option<TruncationReason>,
    /// Snapshot do campo temporal no fim da execução (trilha auditável).
    pub temporal_snapshot: Option<temporal_field::TemporalFieldSnapshot>,
}

/// O orquestrador do loop — percorre a geodésica no campo temporal.
///
/// O `TemporalField` é compartilhado via `Arc<RwLock<..>>`; cada
/// `execute_task` reinicia o campo sob o lock (isolamento por tarefa).
#[derive(Clone)]
pub struct XLoop {
    temporal_field: Arc<RwLock<TemporalField>>,
    /// Campo de objetivos (correção de Carlip) — `E_Δ = 0` degenera o campo.
    pub objective_field: ObjectiveField,
    /// Taxa máxima de iteração do campo (iterações/segundo).
    max_rate: f64,
    /// Executor de passos.
    executor: Arc<dyn StepExecutor>,
}

impl XLoop {
    /// Cria um XLoop com um executor.
    pub fn new(executor: Arc<dyn StepExecutor>, max_rate: f64) -> Self {
        Self {
            temporal_field: Arc::new(RwLock::new(TemporalField::new(max_rate))),
            objective_field: ObjectiveField::new(),
            max_rate,
            executor,
        }
    }

    /// Executa uma tarefa no campo temporal.
    ///
    /// ## A Equação de Kronos
    ///
    /// A cada iteração:
    /// 1. Executa um passo (latência → alimenta o campo).
    /// 2. Atualiza `TemporalField` (erro, novidade, latência) → `τ_eff`.
    /// 3. `geodesic_decision(elapsed, completed, estimated)` → decide.
    ///    - Curvatura crítica → `Collapse::TerminateLoop`.
    ///    - `dτ/dℓ > 2.0` → `Collapse::Fallback` (sai com truncamento).
    ///    - `1.5 < dτ/dℓ ≤ 2.0` → `Warning` (apenas log).
    /// 4. Detecta curvatura (assinatura de estado) → `TemporalLoopDetected`.
    ///
    /// O watchdog `HARD_STEP_CAP_MULTIPLIER · estimated_steps` é o backstop
    /// discreto (mitigação do ADR-002), não a regra primária.
    pub async fn execute_task(&self, task: Task) -> Result<TaskResult, Error> {
        let start = Instant::now();
        let estimated_steps = task.estimated_steps.max(1);
        let hard_cap = estimated_steps.saturating_mul(HARD_STEP_CAP_MULTIPLIER);

        // Isolamento por tarefa: reinicia o campo sob o lock.
        let mut field = self.temporal_field.write().await;
        *field = TemporalField::new(self.max_rate);

        let divergence = self.objective_field.energy_divergence();
        if divergence <= 0.0 {
            debug!(
                "E_Δ = 0 — campo degenerado (crítica de Carlip): o loop só colapsa por expansão temporal, não por gravidade de objetivos"
            );
        }

        let mut completed: u64 = 0;
        let mut truncation: Option<TruncationReason> = None;
        let mut seen: HashSet<String> = HashSet::new();

        loop {
            // 1. Executa o passo e mede a latência real da iteração.
            let step_start = Instant::now();
            let step_result = self.executor.step(&task);
            let step_wall_ms = step_start.elapsed().as_millis() as u64;

            let (state_key, terminal, step_failed) = match step_result {
                Ok(s) => (s.state_key, s.terminal, false),
                Err(e) => {
                    warn!(detail = %e.0, "step failed, feeding the field");
                    (ERROR_STATE_KEY.to_string(), false, true)
                }
            };

            completed += 1;
            let signature = format!("{}|{}", task.id, state_key);
            let novelty = if seen.insert(signature.clone()) { 1.0 } else { 0.0 };
            let elapsed_ms = start.elapsed().as_millis() as u64;

            // 2. Atualiza o campo (erro, novidade, latência da iteração).
            field.update(if step_failed { 1.0 } else { 0.0 }, novelty, step_wall_ms);
            field.consume(1.0);

            // 3. A Equação de Kronos — decisão geodésica.
            let decision = field.geodesic_decision(elapsed_ms, completed, estimated_steps);
            match decision {
                GeodesicDecision::Collapse { reason, action } => {
                    warn!(%reason, ?action, "temporal collapse triggered");
                    return match action {
                        CollapseAction::TerminateLoop => {
                            Err(Error::TemporalCollapse { reason })
                        }
                        CollapseAction::Fallback => {
                            truncation = Some(TruncationReason::FellBack);
                            break;
                        }
                        CollapseAction::Escalate => Err(Error::EscalationRequired { reason }),
                    };
                }
                GeodesicDecision::Warning { reason } => {
                    info!(%reason, "temporal warning");
                }
                GeodesicDecision::Continue { .. } => {}
            }

            // 4. Curvatura — loop infinito confirmado.
            if field.detect_loop(&signature) {
                return Err(Error::TemporalLoopDetected);
            }

            // 5. Terminal ou watchdog (backstop discreto).
            if terminal {
                break;
            }
            if completed >= hard_cap {
                debug!(cap = %hard_cap, "watchdog hard cap reached");
                truncation = Some(TruncationReason::HardCap);
                break;
            }
        }

        let snapshot = field.snapshot();
        Ok(TaskResult {
            task_id: task.id.clone(),
            completed_steps: completed,
            elapsed_ms: start.elapsed().as_millis() as u64,
            truncation,
            temporal_snapshot: Some(snapshot),
        })
    }

    /// Retorna um instantâneo do campo (para observabilidade).
    pub async fn snapshot(&self) -> temporal_field::TemporalFieldSnapshot {
        self.temporal_field.read().await.snapshot()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// TESTES
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    struct StuckExecutor;
    impl StepExecutor for StuckExecutor {
        fn step(&self, _task: &Task) -> Result<StepResult, StepError> {
            Err(StepError("stuck".to_string()))
        }
    }

    struct FiniteExecutor {
        terminal_at: u64,
        calls: AtomicU64,
    }
    impl FiniteExecutor {
        fn new(terminal_at: u64) -> Self {
            Self {
                terminal_at,
                calls: AtomicU64::new(0),
            }
        }
    }
    impl StepExecutor for FiniteExecutor {
        fn step(&self, _task: &Task) -> Result<StepResult, StepError> {
            let n = self.calls.fetch_add(1, Ordering::Relaxed) + 1;
            Ok(StepResult {
                state_key: format!("step_{n}"),
                terminal: n >= self.terminal_at,
            })
        }
    }

    struct OscillatingExecutor {
        calls: AtomicU64,
    }
    impl StepExecutor for OscillatingExecutor {
        fn step(&self, _task: &Task) -> Result<StepResult, StepError> {
            let n = self.calls.fetch_add(1, Ordering::Relaxed);
            Ok(StepResult {
                state_key: if n.is_multiple_of(2) { "A" } else { "B" }.to_string(),
                terminal: false,
            })
        }
    }

    struct ConstantExecutor {
        state_key: &'static str,
    }
    impl StepExecutor for ConstantExecutor {
        fn step(&self, _task: &Task) -> Result<StepResult, StepError> {
            Ok(StepResult {
                state_key: self.state_key.to_string(),
                terminal: false,
            })
        }
    }

    #[tokio::test]
    async fn finite_task_completes() {
        let xloop = XLoop::new(Arc::new(FiniteExecutor::new(3)), 10.0);
        let result = xloop.execute_task(Task::simple("t1", 10)).await.unwrap();
        assert_eq!(result.completed_steps, 3);
        assert_eq!(result.truncation, None);
        let snap = result.temporal_snapshot.expect("snapshot present");
        assert!(snap.energy >= 3.0);
    }

    #[tokio::test]
    async fn stuck_executor_collapses() {
        // Executor sempre em erro (estado constante) → o campo vira sobre si:
        // curvatura crítica dispara `TemporalLoopDetected` ou, na iteração em
        // que a curvatura anterior já cruzou o limiar, `TemporalCollapse`.
        let xloop = XLoop::new(Arc::new(StuckExecutor), 10.0);
        let err = xloop
            .execute_task(Task::simple("stuck", 10))
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            Error::TemporalCollapse { .. } | Error::TemporalLoopDetected
        ));
    }

    #[tokio::test]
    async fn infinite_loop_terminates_or_watchdogs() {
        // Estado constante (não-terminal): cedo ou tarde o campo colapsa
        // (curvatura crítica) ou o watchdog dispara.
        let xloop = XLoop::new(
            Arc::new(ConstantExecutor {
                state_key: "same",
            }),
            10.0,
        );
        let task = Task::simple("loop", 2); // hard_cap = 8
        match xloop.execute_task(task).await {
            Err(Error::TemporalCollapse { .. } | Error::TemporalLoopDetected) => {}
            Ok(r) => assert_eq!(r.truncation, Some(TruncationReason::HardCap)),
            Err(other) => panic!("unexpected error: {other:?}"),
        }
    }

    #[tokio::test]
    async fn oscillation_is_not_premature_collapse() {
        // A→B→A→B...: curvatura ~0.5 (warning), nunca colapso prematuro;
        // termina pelo watchdog com truncamento HardCap (não como erro).
        let xloop = XLoop::new(
            Arc::new(OscillatingExecutor {
                calls: AtomicU64::new(0),
            }),
            10.0,
        );
        let result = xloop
            .execute_task(Task::simple("osc", 8)) // hard_cap = 32
            .await
            .expect("oscillatory loops must not error");
        assert_eq!(result.truncation, Some(TruncationReason::HardCap));
        assert!(result.completed_steps <= 32);
    }

    #[tokio::test]
    async fn objective_field_is_wired() {
        let xloop = XLoop::new(Arc::new(FiniteExecutor::new(1)), 10.0);
        // Degenerado (E_Δ = 0) — a execução ainda funciona: campo é degenerado,
        // mas o loop simplesmente termina (comportamento Carlip).
        assert_eq!(xloop.objective_field.energy_divergence(), 0.0);
        let result = xloop.execute_task(Task::simple("t2", 5)).await.unwrap();
        assert_eq!(result.completed_steps, 1);
    }

    #[tokio::test]
    async fn divergence_breaks_degeneracy() {
        let mut xloop = XLoop::new(Arc::new(FiniteExecutor::new(1)), 10.0);
        xloop.objective_field.set_objective("a", 1.0);
        xloop.objective_field.set_objective("b", 1.0);
        assert!((xloop.objective_field.energy_divergence() - 0.5).abs() < 1e-12);
    }
}