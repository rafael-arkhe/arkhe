//! Proxy & Upgradeability Vulnerabilities (SC10:2026).
//!
//! `∀ proxy, (upgrade_called ∧ onlyAdmin_verified) →
//!    implementation_changed ∧ initialized(proxy) ∧ proxy_safe(proxy)`

use crate::InvariantVerdict;

/// Estado mínimo de um proxy atualizável.
#[derive(Debug, Clone)]
pub struct ProxyState {
    pub admin: String,
    pub implementation: String,
    pub initialized: bool,
}

/// Erro de upgrade de proxy.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ProxyError {
    #[error("caller '{0}' is not the proxy admin")]
    NotAdmin(String),
    #[error("new implementation address is zero/empty")]
    ZeroAddress,
    #[error("proxy not initialized")]
    NotInitialized,
}

impl ProxyState {
    /// FI: só o admin pode disparar upgrade, e o endereço novo deve ser válido.
    /// Retorna o novo estado se a operação é segura.
    pub fn upgrade(
        &self,
        caller: &str,
        new_implementation: &str,
    ) -> Result<ProxyState, ProxyError> {
        if caller != self.admin {
            return Err(ProxyError::NotAdmin(caller.to_string()));
        }
        if new_implementation.is_empty() {
            return Err(ProxyError::ZeroAddress);
        }
        Ok(ProxyState {
            admin: self.admin.clone(),
            implementation: new_implementation.to_string(),
            initialized: self.initialized,
        })
    }

    /// Initializer com proteção contra re-inicialização.
    pub fn initialize(&mut self, admin: &str) -> Result<(), ProxyError> {
        if self.initialized {
            // Re-initialization is the exact vector used to seize ownership
            // of an already-deployed proxy.
            return Err(ProxyError::NotInitialized);
        }
        self.admin = admin.to_string();
        self.initialized = true;
        Ok(())
    }

    /// Invariante composto: proxy só é seguro se inicializado e com admin definido.
    pub fn check_proxy_safe(&self) -> InvariantVerdict {
        if !self.initialized {
            return InvariantVerdict::violated("proxy not initialized");
        }
        if self.admin.is_empty() {
            return InvariantVerdict::violated("proxy has no admin");
        }
        InvariantVerdict::Holds
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> ProxyState {
        ProxyState {
            admin: "admin".to_string(),
            implementation: "impl_v1".to_string(),
            initialized: true,
        }
    }

    #[test]
    fn admin_can_upgrade() {
        let proxy = sample();
        let upgraded = proxy.upgrade("admin", "impl_v2").unwrap();
        assert_eq!(upgraded.implementation, "impl_v2");
    }

    #[test]
    fn non_admin_upgrade_is_rejected() {
        let proxy = sample();
        let err = proxy.upgrade("attacker", "impl_evil").unwrap_err();
        assert_eq!(err, ProxyError::NotAdmin("attacker".to_string()));
    }

    #[test]
    fn double_initialization_is_rejected() {
        let mut proxy = sample();
        assert_eq!(proxy.initialize("new_admin"), Err(ProxyError::NotInitialized));
    }

    #[test]
    fn uninitialized_proxy_is_unsafe() {
        let proxy = ProxyState {
            admin: "admin".to_string(),
            implementation: "impl_v1".to_string(),
            initialized: false,
        };
        assert!(!proxy.check_proxy_safe().holds());
    }
}
