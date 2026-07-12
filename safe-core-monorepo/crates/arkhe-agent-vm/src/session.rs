//! Bridges AAVM's [`crate::policy::AgentPolicy`] and
//! [`crate::lifecycle::Lifecycle`] into `arkhe_core::SafetyVerifier`, the
//! trait `arkhe_agi::AgiCoordinator` already calls before every
//! `process()`. This is what makes [`crate::manager::AAVMManager::spawn_agent_session`]
//! a real integration rather than a cosmetic one: no new hook was invented,
//! an existing, already-used one is implemented against real AAVM state.
//!
//! **What this checks:** the invariant this crate now enforces, formalized
//! in `proofs/lean/AgentVm/PolicyGate.lean`, is:
//!
//! > An agent action is only ever allowed if `AgentPolicy::allows(action)`
//! > holds **and** the VM's `Lifecycle` is `Running`.
//!
//! **What this does not check:** this is in-process, type-system-level
//! enforcement — it does not sandbox the inference engine `I` itself (no
//! process isolation, no resource limits, no syscall filtering; nothing
//! like that exists anywhere in this workspace yet — see
//! `crates/arkhe-tool-sandbox` and `crates/arkhe-syscall-bridge`, both
//! non-functional). A `PolicyVerifier` stops `AgiCoordinator::process` from
//! *starting* an inference call it shouldn't; it does not constrain what
//! that call can do once it starts.

use std::sync::Arc;

use arkhe_core::{SafetyVerdict, SafetyVerifier};
use arkhe_web3_security::agents::{AuditEvidence, EvidenceBus};
use arkhe_web3_security::InvariantVerdict;
use async_trait::async_trait;
use tokio::sync::RwLock;

use crate::lifecycle::{Lifecycle, LifecycleState};
use crate::policy::AgentPolicy;

/// FI-A04: the invariant this type exists to enforce and record — see the
/// module doc comment for its exact statement.
const FI_A04: &str = "FI-A04";

pub struct PolicyVerifier {
    aavm_id: String,
    policy: Arc<AgentPolicy>,
    lifecycle: Arc<RwLock<Lifecycle>>,
    evidence_bus: Arc<EvidenceBus>,
}

impl PolicyVerifier {
    pub fn new(
        aavm_id: String,
        policy: Arc<AgentPolicy>,
        lifecycle: Arc<RwLock<Lifecycle>>,
        evidence_bus: Arc<EvidenceBus>,
    ) -> Self {
        Self { aavm_id, policy, lifecycle, evidence_bus }
    }

    async fn record(&self, verdict: InvariantVerdict) -> SafetyVerdict {
        let safety_verdict = match &verdict {
            InvariantVerdict::Holds => SafetyVerdict::Allowed,
            InvariantVerdict::Violated { reason } => SafetyVerdict::Rejected(reason.clone()),
        };
        self.evidence_bus.store(AuditEvidence { invariant_id: FI_A04, verdict }).await;
        safety_verdict
    }
}

#[async_trait]
impl SafetyVerifier for PolicyVerifier {
    /// `action` is checked against the AAVM's `AgentPolicy` capability
    /// list; `context` is not currently used (kept for trait
    /// compatibility — `AgiCoordinator` passes the user's raw input there,
    /// which this verifier deliberately does not inspect, since capability
    /// gating shouldn't depend on message content).
    async fn verify(&self, action: &str, _context: &str) -> SafetyVerdict {
        let state = self.lifecycle.read().await.state();
        if state != LifecycleState::Running {
            let reason = format!("AAVM {} is not Running (state: {state:?})", self.aavm_id);
            return self.record(InvariantVerdict::violated(reason)).await;
        }

        if !self.policy.allows(action) {
            let reason = format!("AAVM {} policy does not allow action '{action}'", self.aavm_id);
            return self.record(InvariantVerdict::violated(reason)).await;
        }

        self.record(InvariantVerdict::Holds).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy_allowing(actions: &[&str]) -> Arc<AgentPolicy> {
        Arc::new(AgentPolicy {
            max_lifetime_secs: 3600,
            allowed_capabilities: actions.iter().map(|s| s.to_string()).collect(),
        })
    }

    async fn running_lifecycle() -> Arc<RwLock<Lifecycle>> {
        let mut lc = Lifecycle::new();
        lc.start().unwrap();
        Arc::new(RwLock::new(lc))
    }

    #[tokio::test]
    async fn allowed_action_on_running_vm_is_allowed() {
        let verifier = PolicyVerifier::new(
            "vm-1".into(),
            policy_allowing(&["llm_inference"]),
            running_lifecycle().await,
            Arc::new(EvidenceBus::new()),
        );
        assert_eq!(verifier.verify("llm_inference", "hello").await, SafetyVerdict::Allowed);
    }

    #[tokio::test]
    async fn action_not_in_policy_is_rejected() {
        let verifier = PolicyVerifier::new(
            "vm-1".into(),
            policy_allowing(&["fs.read"]),
            running_lifecycle().await,
            Arc::new(EvidenceBus::new()),
        );
        assert!(matches!(verifier.verify("llm_inference", "hello").await, SafetyVerdict::Rejected(_)));
    }

    #[tokio::test]
    async fn action_is_rejected_once_lifecycle_leaves_running() {
        let lifecycle = running_lifecycle().await;
        let evidence_bus = Arc::new(EvidenceBus::new());
        let verifier =
            PolicyVerifier::new("vm-1".into(), policy_allowing(&["llm_inference"]), lifecycle.clone(), evidence_bus);

        assert_eq!(verifier.verify("llm_inference", "hello").await, SafetyVerdict::Allowed);

        // Simulates AAVMManager::destroy_vm mutating the *same* shared
        // lifecycle a spawned session already holds a clone of.
        {
            let mut lc = lifecycle.write().await;
            lc.begin_terminate().unwrap();
        }

        assert!(matches!(verifier.verify("llm_inference", "hello").await, SafetyVerdict::Rejected(_)));
    }

    #[tokio::test]
    async fn every_verify_call_is_recorded_as_fi_a04_evidence() {
        let evidence_bus = Arc::new(EvidenceBus::new());
        let verifier = PolicyVerifier::new(
            "vm-1".into(),
            policy_allowing(&["llm_inference"]),
            running_lifecycle().await,
            evidence_bus.clone(),
        );

        verifier.verify("llm_inference", "hello").await;
        verifier.verify("fs.write", "hello").await;

        let evidence = evidence_bus.all().await;
        assert_eq!(evidence.len(), 2);
        assert!(evidence.iter().all(|e| e.invariant_id == FI_A04));
        assert!(evidence[0].verdict.holds());
        assert!(!evidence[1].verdict.holds());
    }
}
