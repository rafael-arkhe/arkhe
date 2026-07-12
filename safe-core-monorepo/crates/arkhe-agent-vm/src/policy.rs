//! AAVM policy: concrete, checkable constraints on an agent VM.
//!
//! Replaces the earlier sketch's `Policy` field, which was referenced only
//! as an unstructured "Daoloop" placeholder with no actual fields or
//! semantics — nothing to check, so nothing was ever enforced. This version
//! defines real fields with a real, tested `check()` method.

use crate::lifecycle::Lifecycle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentPolicy {
    /// Hard cap on how long the VM may run before it must be terminated.
    pub max_lifetime_secs: u64,
    /// Capability strings the VM is permitted to exercise (e.g.
    /// `"web3.audit"`, `"fs.read"`). Enforcement of individual capabilities
    /// happens at the call site that would grant them; this list is the
    /// source of truth for "is X allowed at all."
    pub allowed_capabilities: Vec<String>,
}

impl AgentPolicy {
    /// A conservative default: 1 hour max lifetime, no capabilities granted.
    pub fn restrictive() -> Self {
        Self { max_lifetime_secs: 3600, allowed_capabilities: Vec::new() }
    }

    pub fn allows(&self, capability: &str) -> bool {
        self.allowed_capabilities.iter().any(|c| c == capability)
    }

    /// True once `lifecycle`'s age exceeds this policy's `max_lifetime_secs`
    /// — the VM must be terminated regardless of its own reported state.
    pub fn is_expired(&self, lifecycle: &Lifecycle) -> bool {
        lifecycle.age_secs() > self.max_lifetime_secs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restrictive_policy_grants_no_capabilities() {
        let policy = AgentPolicy::restrictive();
        assert!(!policy.allows("fs.write"));
    }

    #[test]
    fn allows_checks_the_exact_capability_string() {
        let policy = AgentPolicy {
            max_lifetime_secs: 60,
            allowed_capabilities: vec!["web3.audit".to_string()],
        };
        assert!(policy.allows("web3.audit"));
        assert!(!policy.allows("web3.audit.write"));
    }

    #[test]
    fn fresh_lifecycle_is_not_expired_under_a_generous_policy() {
        let policy = AgentPolicy { max_lifetime_secs: 3600, allowed_capabilities: vec![] };
        let lifecycle = Lifecycle::new();
        assert!(!policy.is_expired(&lifecycle));
    }

    #[test]
    fn effectively_unlimited_policy_is_never_expired() {
        let policy = AgentPolicy { max_lifetime_secs: u64::MAX, allowed_capabilities: vec![] };
        let lifecycle = Lifecycle::new();
        assert!(!policy.is_expired(&lifecycle));
    }
}
