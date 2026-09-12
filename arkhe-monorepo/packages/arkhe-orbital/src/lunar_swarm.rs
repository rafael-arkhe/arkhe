//! # LunarSwarm — a população de especialistas
//!
//! Analogia: as **293 luas de Saturno** — capturas caóticas e colisões dão
//! origem a uma população diversa de corpos, cada um com órbita e composição
//! próprias. No sistema de agentes, cada "lua" é um agente especializado com
//! capacidade, especialidade, carga e propensão à novidade.
//!
//! A seleção é determinística: uma função de utilidade
//! `capacidade × afinidade × disponibilidade`, modulada pelo modo
//! explore/exploit e pela política hexagonal.

use crate::decagonal_field::FieldMode;
use crate::hexagonal_core::Policy;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Identificador de um agente especialista.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AgentId(pub String);

/// Agente especialista ("lua" do swarm).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    /// Identificador do agente.
    pub id: AgentId,
    /// Especialidade declarada.
    pub specialty: String,
    /// Capacidade técnica (0.0–1.0).
    pub capability: f64,
    /// Carga atual (0.0–1.0); acima do teto o agente fica indisponível.
    pub load: f64,
    /// Propensão à novidade (0.0–1.0) — bônus no modo Explore.
    pub novelty_score: f64,
}

impl Agent {
    /// Cria um agente com os atributos explicitados.
    pub fn new(
        id: &str,
        specialty: &str,
        capability: f64,
        load: f64,
        novelty_score: f64,
    ) -> Self {
        Self {
            id: AgentId(id.to_string()),
            specialty: specialty.to_string(),
            capability: capability.clamp(0.0, 1.0),
            load: load.clamp(0.0, 1.0),
            novelty_score: novelty_score.clamp(0.0, 1.0),
        }
    }
}

/// Erros da seleção no swarm.
#[derive(Debug, Error, PartialEq)]
pub enum SwarmError {
    /// Nenhum agente tem a especialidade requisitada.
    #[error("no agent with specialty '{0}' in the swarm")]
    NoSpecialist(String),
    /// Todos os candidatos estão com carga acima do teto permitido.
    #[error("all candidates are over-loaded (max load {0:.2})")]
    AllOverloaded(f64),
    /// O swarm está vazio.
    #[error("swarm is empty")]
    Empty,
}

/// Tarefa entregue ao orquestrador orbital (tipo leve, sem arco de
/// dependências — Simplicity-2).
#[derive(Debug, Clone)]
pub struct Task {
    /// Identificador da tarefa.
    pub id: String,
    /// Prioridade da tarefa (0.0–1.0), usada pelo HexagonalCore.
    pub priority: f64,
    /// Especialidade requisitada.
    pub specialty: Option<String>,
}

/// Seleção de especialista — seletor determinístico por utilidade.
#[derive(Debug, Clone)]
pub struct LunarSwarm {
    /// População de especialistas.
    pub agents: Vec<Agent>,
    /// Teto de carga para políticas conservadoras.
    pub conservative_max_load: f64,
    /// Teto de carga padrão.
    pub standard_max_load: f64,
    /// Bônus de exploração aplicado no modo Explore.
    pub explore_bonus: f64,
}

impl LunarSwarm {
    /// Cria um swarm com os tetos e bônus padrão.
    pub fn new(agents: Vec<Agent>) -> Self {
        Self {
            agents,
            conservative_max_load: 0.6,
            standard_max_load: 0.85,
            explore_bonus: 0.2,
        }
    }

    /// Teto de carga imposto pela política.
    fn max_load_for(&self, policy: &Policy) -> f64 {
        match policy.kind {
            crate::hexagonal_core::PolicyKind::Conservative => self.conservative_max_load,
            crate::hexagonal_core::PolicyKind::Standard => self.standard_max_load,
            crate::hexagonal_core::PolicyKind::Aggressive => 0.95,
        }
    }

    /// Utilidade de `agent` para `task` no modo atual.
    ///
    /// ```text
    /// base = capability · (1 − load) · (0.3 + 0.7 · affinity)
    /// bonus = novelty · explore_bonus   (apenas no modo Explore)
    /// ```
    pub fn score(&self, agent: &Agent, task: &Task, mode: FieldMode) -> f64 {
        let requested = task.specialty.as_deref().unwrap_or("");
        let affinity = if agent.specialty == requested { 1.0 } else { 0.0 };
        let base = agent.capability * (1.0 - agent.load) * (0.3 + 0.7 * affinity);
        let bonus = if mode == FieldMode::Explore {
            agent.novelty_score * self.explore_bonus
        } else {
            0.0
        };
        base + bonus
    }

    /// Seleciona o melhor especialista.
    ///
    /// Regras:
    /// 1. Sem especialidade requisitada: todo o swarm pode servir.
    /// 2. Especialidade requisitada e nenhum agente a domina → `NoSpecialist`.
    /// 3. Dentre os candidatos, filtro por teto de carga (política).
    ///    Todos acima do teto → `AllOverloaded`.
    /// 4. Melhor utilidade (`score`), empate resolvido pelo primeiro.
    pub async fn select(
        &self,
        task: &Task,
        policy: &Policy,
        mode: FieldMode,
    ) -> Result<Agent, SwarmError> {
        if self.agents.is_empty() {
            return Err(SwarmError::Empty);
        }

        if let Some(specialty) = &task.specialty {
            let any_specialist = self
                .agents
                .iter()
                .any(|a| &a.specialty == specialty);
            if !any_specialist {
                return Err(SwarmError::NoSpecialist(specialty.clone()));
            }
        }

        let max_load = self.max_load_for(policy);
        let mut best: Option<(&Agent, f64)> = None;

        for agent in self.agents.iter() {
            let affinity = task
                .specialty
                .as_deref()
                .map(|s| s == agent.specialty)
                .unwrap_or(true);
            if !affinity || agent.load > max_load {
                continue;
            }
            let score = self.score(agent, task, mode);
            let ties_break = best
                .as_ref()
                .map(|(_, s)| *s < score)
                .unwrap_or(true);
            if ties_break {
                best = Some((agent, score));
            }
        }

        match best {
            Some((agent, score)) => {
                tracing::debug!(
                    agent = %agent.id.0,
                    score = %(score * 10000.0).trunc() / 10000.0,
                    "lunar swarm selected specialist"
                );
                Ok(agent.clone())
            }
            None => Err(SwarmError::AllOverloaded(max_load)),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// TESTES
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decagonal_field::FieldMode;
    use crate::hexagonal_core::{PolicyEngine, TaskClass};

    fn swarm() -> LunarSwarm {
        LunarSwarm::new(vec![
            Agent::new("titan", "coding", 0.8, 0.9, 0.5),
            Agent::new("enceladus", "coding", 0.7, 0.1, 0.9),
            Agent::new("rhea", "retrieval", 0.9, 0.2, 0.1),
        ])
    }

    fn coding_task() -> Task {
        Task {
            id: "task-coding".to_string(),
            priority: 0.7,
            specialty: Some("coding".to_string()),
        }
    }

    fn standard_policy() -> Policy {
        PolicyEngine::default().select(TaskClass::Standard)
    }

    #[tokio::test]
    async fn selects_best_available_specialist() {
        let swarm = swarm();
        let agent = swarm
            .select(&coding_task(), &standard_policy(), FieldMode::Exploit)
            .await
            .unwrap();
        // Titan (load 0.9 > 0.85) fica fora; Encélado vence a corrida.
        assert_eq!(agent.id.0, "enceladus");
    }

    #[tokio::test]
    async fn conservative_enforces_low_load_and_exact_specialty() {
        let swarm = swarm();
        let policy = PolicyEngine::default().select(TaskClass::Critical);
        // Conservative: teto 0.6. Encélado (0.1) atende e vence.
        let agent = swarm
            .select(&coding_task(), &policy, FieldMode::Exploit)
            .await
            .unwrap();
        assert_eq!(agent.id.0, "enceladus");
    }

    #[tokio::test]
    async fn all_loaded_conservative_reports_error() {
        let swarm = LunarSwarm::new(vec![
            Agent::new("a", "coding", 0.9, 0.7, 0.1),
            Agent::new("b", "coding", 0.8, 0.9, 0.1),
        ]);
        let policy = PolicyEngine::default().select(TaskClass::Critical);
        let err = swarm
            .select(&coding_task(), &policy, FieldMode::Exploit)
            .await
            .unwrap_err();
        assert_eq!(err, SwarmError::AllOverloaded(0.6));
    }

    #[tokio::test]
    async fn no_specialist_reports_error() {
        let swarm = swarm();
        let err = swarm
            .select(
                &Task {
                    id: "t".to_string(),
                    priority: 0.5,
                    specialty: Some("astronomy".to_string()),
                },
                &standard_policy(),
                FieldMode::Exploit,
            )
            .await
            .unwrap_err();
        assert_eq!(err, SwarmError::NoSpecialist("astronomy".to_string()));
    }

    #[tokio::test]
    async fn explore_bonus_rewards_novelty() {
        // Dois especialistas empatados em explotação; a novidade decide
        // apenas no modo Explore.
        let swarm = LunarSwarm::new(vec![
            Agent::new("mimas", "coding", 0.8, 0.1, 0.1),
            Agent::new("enceladus", "coding", 0.7, 0.1, 0.9),
        ]);
        let exploit = swarm
            .select(&coding_task(), &standard_policy(), FieldMode::Exploit)
            .await
            .unwrap();
        assert_eq!(exploit.id.0, "mimas");

        let explore = swarm
            .select(&coding_task(), &standard_policy(), FieldMode::Explore)
            .await
            .unwrap();
        assert_eq!(explore.id.0, "enceladus");
    }
}