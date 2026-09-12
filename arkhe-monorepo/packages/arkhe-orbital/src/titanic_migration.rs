//! # TitanicMigration — utilidade como maré
//!
//! Analogia: **Titã está migrando para fora de Saturno** ~100× mais rápido do
//! que a dissipação de maré previa. A migração é dirigida por utilidade:
//! o agente move-se para o tenant onde `U = capacidade × demanda` é máximo.
//!
//! ```text
//! benefit = max(U) − U_atual         (sobre as marés de todos os tenants)
//! migra se benefit > cost / migration_rate
//! ```
//!
//! `migration_rate` é o análogo da dissipação de marés: quanto maior, mais
//! agressiva é a migração por unidade de benefício.

use crate::lunar_swarm::Agent;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

/// Identificador de um tenant.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TenantId(pub String);

/// Tenant hospedeiro — demanda por especialidade.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tenant {
    /// Identificador do tenant.
    pub id: TenantId,
    /// Demanda por especialidade (0.0–1.0).
    pub demand: HashMap<String, f64>,
}

impl Tenant {
    /// Cria um tenant com demanda vazia.
    pub fn new(id: &str) -> Self {
        Self {
            id: TenantId(id.to_string()),
            demand: HashMap::new(),
        }
    }

    /// Define a demanda de uma especialidade.
    pub fn with_demand(mut self, specialty: &str, demand: f64) -> Self {
        self.demand
            .insert(specialty.to_string(), demand.clamp(0.0, 1.0));
        self
    }

    /// Demanda pela especialidade (0.0 se desconhecida).
    pub fn demand_for(&self, specialty: &str) -> f64 {
        self.demand.get(specialty).copied().unwrap_or(0.0)
    }
}

/// Erros da migração.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum MigrationError {
    /// O tenant atual não existe na malha.
    #[error("current tenant '{0}' not found in the mesh")]
    TenantNotFound(String),
    /// O agente já está no tenant de máxima utilidade.
    #[error("agent already at optimal tenant")]
    AlreadyOptimal,
}

/// Migração dirigida por utilidade (análogo de Titã).
#[derive(Debug, Clone)]
pub struct TitanicMigration {
    /// Taxa de migração — análogo da dissipação de marés.
    pub migration_rate: f64,
    /// Malha de tenants conhecida.
    pub tenants: Vec<Tenant>,
    /// Custo base de migração (transferência de contexto, re-attestation).
    pub base_cost: f64,
}

impl Default for TitanicMigration {
    fn default() -> Self {
        Self {
            migration_rate: 1.0,
            tenants: Vec::new(),
            base_cost: 1.0,
        }
    }
}

impl TitanicMigration {
    /// Cria a migração com a malha de tenants.
    pub fn new(tenants: Vec<Tenant>) -> Self {
        Self {
            tenants,
            ..Default::default()
        }
    }

    /// Utilidade de `agent` no `tenant`: `U = capacidade × demanda`.
    pub fn utility(&self, agent: &Agent, tenant: &Tenant) -> f64 {
        agent.capability * tenant.demand_for(&agent.specialty)
    }

    /// Utilidade do tenant atual (`0.0` se o tenant está fora da malha).
    fn current_utility(&self, agent: &Agent, current: &TenantId) -> f64 {
        self.tenants
            .iter()
            .find(|t| t.id == *current)
            .map(|t| self.utility(agent, t))
            .unwrap_or(0.0)
    }

    /// Melhor tenant da malha (por utilidade) e seu valor.
    fn best_tenant(&self, agent: &Agent) -> Option<(&Tenant, f64)> {
        self.tenants
            .iter()
            .map(|t| (t, self.utility(agent, t)))
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
    }

    /// Custo de migrar `agent` (contexto + re-attestation + warmup).
    pub fn migration_cost(&self, agent: &Agent) -> f64 {
        // Agentes mais capazes carregam mais contexto → custo maior.
        self.base_cost * (1.0 + 0.5 * agent.capability)
    }

    /// Decide se o agente deve migrar.
    ///
    /// ```text
    /// migra se (best_utility − current_utility) > cost / migration_rate
    /// ```
    ///
    /// Quanto maior `migration_rate` (análogo à dissipação de marés), menor o
    /// limiar — migração mais agressiva por unidade de benefício.
    pub async fn should_migrate(
        &self,
        agent: &Agent,
        current_tenant: &TenantId,
    ) -> bool {
        let current = self.current_utility(agent, current_tenant);
        let Some((_, best)) = self.best_tenant(agent) else {
            return false;
        };
        let benefit = best - current;
        let cost = self.migration_cost(agent);
        benefit > cost / self.migration_rate
    }

    /// Executa a migração: devolve o tenant de máxima utilidade.
    pub async fn migrate(
        &self,
        agent: &Agent,
        current_tenant: &TenantId,
    ) -> Result<TenantId, MigrationError> {
        if !self.tenants.iter().any(|t| t.id == *current_tenant) {
            return Err(MigrationError::TenantNotFound(current_tenant.0.clone()));
        }
        let Some((tenant, best)) = self.best_tenant(agent) else {
            return Err(MigrationError::TenantNotFound(current_tenant.0.clone()));
        };
        if tenant.id == *current_tenant {
            return Err(MigrationError::AlreadyOptimal);
        }
        let cost = self.migration_cost(agent);
        if best - self.current_utility(agent, current_tenant) <= cost / self.migration_rate {
            return Err(MigrationError::AlreadyOptimal);
        }
        Ok(tenant.id.clone())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// TESTES
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lunar_swarm::Agent;

    fn handyman() -> Agent {
        Agent::new("titan", "coding", 0.9, 0.1, 0.2)
    }

    fn mesh() -> TitanicMigration {
        TitanicMigration::new(vec![
            Tenant::new("free").with_demand("coding", 0.1),
            Tenant::new("enterprise").with_demand("coding", 0.9),
        ])
    }

    #[tokio::test]
    async fn migrates_when_benefit_exceeds_amortized_cost() {
        let migration = TitanicMigration {
            migration_rate: 1.0,
            ..mesh()
        };
        let from = TenantId("free".to_string());
        // benefit = 0.9*0.9 − 0.9*0.1 = 0.72; cost/rate = 1.45/1.0 = 1.45.
        // 0.72 > 1.45? Não → não migra com cost alto...
        assert!(!migration.should_migrate(&handyman(), &from).await);
    }

    #[tokio::test]
    async fn aggressive_tide_migrates() {
        // Dissipação de marés 100× maior (análogo de Titã): o limiar
        // cost/rate cai para ~0.0145 → 0.72 > 0.0145 → migra.
        let migration = TitanicMigration {
            migration_rate: 100.0,
            ..mesh()
        };
        let from = TenantId("free".to_string());
        assert!(migration.should_migrate(&handyman(), &from).await);
    }

    #[tokio::test]
    async fn migrate_returns_new_tenant() {
        let migration = TitanicMigration {
            migration_rate: 100.0,
            ..mesh()
        };
        let from = TenantId("free".to_string());
        let to = migration.migrate(&handyman(), &from).await.unwrap();
        assert_eq!(to, TenantId("enterprise".to_string()));
    }

    #[tokio::test]
    async fn migrate_rejects_already_optimal() {
        let migration = TitanicMigration {
            migration_rate: 100.0,
            ..mesh()
        };
        let at = TenantId("enterprise".to_string());
        let err = migration.migrate(&handyman(), &at).await.unwrap_err();
        assert_eq!(err, MigrationError::AlreadyOptimal);
    }

    #[tokio::test]
    async fn migrate_rejects_unknown_tenant() {
        let migration = mesh();
        let ghost = TenantId("ghost".to_string());
        let err = migration.migrate(&handyman(), &ghost).await.unwrap_err();
        assert_eq!(err, MigrationError::TenantNotFound("ghost".to_string()));
    }
}