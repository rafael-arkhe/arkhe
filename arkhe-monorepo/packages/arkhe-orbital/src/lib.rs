//! # arkhe-orbital — Consciência orbital (padrões de Saturno)
//!
//! Quatro padrões emergentes de Saturno mapeados como arquitetura de agentes
//! auto-organizáveis:
//!
//! | Padrão de Saturno | Mecanismo físico | Análogo em agentes |
//! |:---|:---|:---|
//! | Hexágono polar | Turbulência auto-organizada em vórtice estável | [`hexagonal_core`] — política estável |
//! | Decágono austral | Onda planetária acoplada ao jato | [`decagonal_field`] — oscilação explore/exploit (período 32) |
//! | 293 luas | Capturas caóticas + colisões | [`lunar_swarm`] — população de especialistas |
//! | Titã migrando | Dissipação de marés 100× maior | [`titanic_migration`] — migração por utilidade |
//!
//! O [`OrbitalOrchestrator`] combina os quatro: política estável → modo do
//! pêndulo → especialista → migração titânica.

pub mod decagonal_field;
pub mod hexagonal_core;
pub mod lunar_swarm;
pub mod titanic_migration;

pub use decagonal_field::{DecagonalField, FieldMode};
pub use hexagonal_core::{HexagonalCore, Policy, PolicyKind, TaskClass};
pub use lunar_swarm::{Agent, AgentId, LunarSwarm, SwarmError, Task as OrbitalTask};
pub use titanic_migration::{MigrationError, Tenant, TenantId, TitanicMigration};

use thiserror::Error;

/// Erros do orquestrador orbital.
#[derive(Debug, Error)]
pub enum OrbitalError {
    /// Falha na seleção do swarm.
    #[error("swarm error: {0}")]
    Swarm(#[from] SwarmError),
    /// Falha na migração titânica.
    #[error("migration error: {0}")]
    Migration(#[from] MigrationError),
}

/// Atribuição final: agente + política + modo + tenant.
#[derive(Debug, Clone)]
pub struct AgentAssignment {
    /// Especialista selecionado.
    pub agent: Agent,
    /// Política do núcleo hexagonal.
    pub policy: Policy,
    /// Modo do campo decagonal.
    pub mode: FieldMode,
    /// Tenant de residência (após migração, se houver).
    pub tenant: TenantId,
    /// Indica se houve migração titânica.
    pub migrated: bool,
}

/// Orquestrador orbital — combina os quatro padrões de Saturno.
///
/// Fluxo:
/// 1. **Hexagonal core**: política estável (com histerese e auditoria).
/// 2. **Campo decagonal**: modo explore/exploit (período 32).
/// 3. **Swarm lunar**: seleção do especialista (utilidade determinística).
/// 4. **Migração titânica**: verifica e executa migração por utilidade.
#[derive(Debug)]
pub struct OrbitalOrchestrator {
    /// Núcleo hexagonal (política estável).
    pub core: HexagonalCore,
    /// Campo decagonal (oscilação).
    pub field: DecagonalField,
    /// Swarm lunar (especialistas).
    pub swarm: LunarSwarm,
    /// Migração titânica (utilidade como maré).
    pub migration: TitanicMigration,
}

impl OrbitalOrchestrator {
    /// Cria o orquestrador com as quatro camadas.
    pub fn new(
        core: HexagonalCore,
        field: DecagonalField,
        swarm: LunarSwarm,
        migration: TitanicMigration,
    ) -> Self {
        Self {
            core,
            field,
            swarm,
            migration,
        }
    }

    /// Roteia uma tarefa pelo ciclo orbital completo.
    pub async fn route(&mut self, task: OrbitalTask) -> Result<AgentAssignment, OrbitalError> {
        // 1. Política estável (hexágono polar).
        let policy = self.core.select_policy(&task.id, task.priority);

        // 2. Modo do pêndulo (decágono austral).
        let mode = self.field.current_mode().await;

        // 3. Especialista (293 luas).
        let agent = self.swarm.select(&task, &policy, mode).await?;

        // 4. Migração titânica (Titã).
        let current_tenant = TenantId("default".to_string());
        let mut migrated = false;
        if self.migration.should_migrate(&agent, &current_tenant).await {
            // Se o tenant default não existe na malha, a migração falha com
            // TenantNotFound — o orquestrador tolera e mantém o tenant atual.
            if let Ok(to) = self.migration.migrate(&agent, &current_tenant).await {
                migrated = true;
                tracing::info!(agent = %agent.id.0, to = %to.0, "titanic migration executed");
                return Ok(AgentAssignment {
                    agent,
                    policy,
                    mode,
                    tenant: to,
                    migrated,
                });
            }
        }

        Ok(AgentAssignment {
            agent,
            policy,
            mode,
            tenant: current_tenant,
            migrated,
        })
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// TESTES
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lunar_swarm::Agent;
    use crate::titanic_migration::Tenant;

    #[tokio::test]
    async fn route_full_cycle() {
        let core = HexagonalCore::default();
        let field = DecagonalField::default();
        let swarm = LunarSwarm::new(vec![Agent::new(
            "rhea",
            "retrieval",
            0.9,
            0.1,
            0.1,
        )]);
        let migration = TitanicMigration::new(vec![
            Tenant::new("default").with_demand("retrieval", 0.5),
        ]);

        let mut orchestrator = OrbitalOrchestrator::new(core, field, swarm, migration);
        let assignment = orchestrator
            .route(OrbitalTask {
                id: "task-orb".to_string(),
                priority: 0.6,
                specialty: Some("retrieval".to_string()),
            })
            .await
            .unwrap();

        assert_eq!(assignment.agent.id, AgentId("rhea".to_string()));
        assert!(!assignment.migrated);
        // O tenant default é ele mesmo o melhor (não migra).
        assert_eq!(assignment.tenant, TenantId("default".to_string()));
    }

    #[tokio::test]
    async fn route_tolerates_unknown_tenant() {
        let core = HexagonalCore::default();
        let field = DecagonalField::default();
        let swarm = LunarSwarm::new(vec![Agent::new(
            "enceladus",
            "coding",
            0.7,
            0.1,
            0.9,
        )]);
        // A malha NÃO contém "default": migração falha com TenantNotFound,
        // que é engolido → assignment mantém o tenant corrente.
        let migration = TitanicMigration {
            migration_rate: 100.0,
            tenants: vec![Tenant::new("enterprise").with_demand("coding", 0.9)],
            ..Default::default()
        };

        let mut orchestrator = OrbitalOrchestrator::new(core, field, swarm, migration);
        let assignment = orchestrator
            .route(OrbitalTask {
                id: "task-2".to_string(),
                priority: 0.7,
                specialty: Some("coding".to_string()),
            })
            .await
            .unwrap();

        assert!(!assignment.migrated);
        assert_eq!(assignment.tenant, TenantId("default".to_string()));
    }
}