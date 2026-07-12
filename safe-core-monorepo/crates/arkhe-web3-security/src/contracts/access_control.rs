//! FI-W01 — Controle de acesso (SC01:2026).
//!
//! `∀ (caller, action): execute(caller, action) → has_capability(caller, action)`

use crate::InvariantVerdict;
use std::collections::{HashMap, HashSet};

/// Registro de capacidades: quem pode executar quais ações.
#[derive(Debug, Default, Clone)]
pub struct CapabilityRegistry {
    capabilities: HashMap<String, HashSet<String>>,
}

impl CapabilityRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Concede a `caller` a capacidade de executar `action`.
    pub fn grant(&mut self, caller: impl Into<String>, action: impl Into<String>) {
        self.capabilities
            .entry(caller.into())
            .or_default()
            .insert(action.into());
    }

    /// Revoga a capacidade de `caller` executar `action`.
    pub fn revoke(&mut self, caller: &str, action: &str) {
        if let Some(actions) = self.capabilities.get_mut(caller) {
            actions.remove(action);
        }
    }

    fn has_capability(&self, caller: &str, action: &str) -> bool {
        self.capabilities
            .get(caller)
            .map(|a| a.contains(action))
            .unwrap_or(false)
    }

    /// FI-W01: verifica se `caller` pode executar `action`.
    /// Retorna `Holds` apenas se a capacidade existe no registro.
    pub fn check_execute(&self, caller: &str, action: &str) -> InvariantVerdict {
        if self.has_capability(caller, action) {
            InvariantVerdict::Holds
        } else {
            InvariantVerdict::violated(format!(
                "caller '{caller}' has no capability for action '{action}'"
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn granted_capability_allows_execution() {
        let mut reg = CapabilityRegistry::new();
        reg.grant("admin", "upgrade");
        assert!(reg.check_execute("admin", "upgrade").holds());
    }

    #[test]
    fn unauthorized_caller_is_rejected() {
        let reg = CapabilityRegistry::new();
        let verdict = reg.check_execute("attacker", "upgrade");
        assert!(!verdict.holds());
    }

    #[test]
    fn revoked_capability_is_rejected() {
        let mut reg = CapabilityRegistry::new();
        reg.grant("admin", "upgrade");
        reg.revoke("admin", "upgrade");
        assert!(!reg.check_execute("admin", "upgrade").holds());
    }
}
