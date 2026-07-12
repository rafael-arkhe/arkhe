//! AAVM manager: creates and destroys agent VMs, wiring together
//! `arkhe-identity` (GDID), `arkhe-crypto-pqc` (hybrid signatures), and
//! `arkhe-web3-security`'s `EvidenceBus`.
//!
//! Per [ADR-0001](../../../docs/decisions/ADR-0001-two-pqc-crates.md), this
//! uses `arkhe-crypto-pqc` specifically (not `arkhe-pqc-core`) because its
//! secret key types have real zeroization guarantees — relevant here since
//! the manager briefly holds live agent secret key material during
//! `create_vm`.
//!
//! **Secret custody note:** the manager does not retain a VM's signing key
//! for its whole lifetime. It's generated, used once to produce the
//! self-attestation below, and then dropped (zeroized via
//! `arkhe-crypto-pqc`'s native `ZeroizeOnDrop`) before `create_vm` returns.
//! Only the public `HybridVerifyingKey` is kept in the handle. If a future
//! use case needs the manager to sign on the VM's behalf later in its
//! life, that's a distinct design decision this doesn't make.

use std::sync::Arc;

use arkhe_crypto_pqc::{generate_hybrid_keypair, HybridVerifyingKey};
use arkhe_identity::Gdid;
use arkhe_web3_security::agents::{AuditEvidence, EvidenceBus};
use arkhe_web3_security::InvariantVerdict;
use tokio::sync::Mutex;

use crate::lifecycle::{InvalidTransition, Lifecycle, LifecycleState};
use crate::policy::AgentPolicy;

#[derive(Debug, thiserror::Error)]
pub enum AavmError {
    #[error("no AAVM found with id {0}")]
    NotFound(String),
    #[error("lifecycle transition failed: {0}")]
    Lifecycle(#[from] InvalidTransition),
    #[error("identity self-attestation failed: {0}")]
    IdentityAttestationFailed(String),
    #[error("policy is invalid: {0}")]
    InvalidPolicy(String),
}

/// Fixed challenge every AAVM signs at creation time to prove it holds the
/// secret key matching its own public identity — see the module doc
/// comment for what this does and doesn't cover.
const IDENTITY_CHALLENGE: &[u8] = b"arkhe-agent-vm/identity-attestation/v1";

struct AavmHandle {
    id: String,
    gdid: Gdid,
    verifying_key: HybridVerifyingKey,
    lifecycle: Lifecycle,
    policy: AgentPolicy,
}

/// A read-only snapshot of an AAVM's public state — no secret key
/// material, safe to hand to callers outside the manager.
#[derive(Debug, Clone)]
pub struct AavmSummary {
    pub id: String,
    pub gdid: Gdid,
    pub state: LifecycleState,
    pub age_secs: u64,
}

impl From<&AavmHandle> for AavmSummary {
    fn from(h: &AavmHandle) -> Self {
        Self { id: h.id.clone(), gdid: h.gdid, state: h.lifecycle.state(), age_secs: h.lifecycle.age_secs() }
    }
}

pub struct AAVMManager {
    evidence_bus: Arc<EvidenceBus>,
    vms: Mutex<Vec<AavmHandle>>,
}

impl AAVMManager {
    pub fn new(evidence_bus: Arc<EvidenceBus>) -> Self {
        Self { evidence_bus, vms: Mutex::new(Vec::new()) }
    }

    /// Creates a new AAVM under `policy`:
    /// 1. Generates a fresh hybrid PQC keypair for the VM's identity.
    /// 2. Derives a [`Gdid`] from a truncated BLAKE3 hash of the ML-DSA-65
    ///    public key, tagged `Gdid::VERSION_ML_DSA_65`.
    /// 3. Self-signs [`IDENTITY_CHALLENGE`] and verifies it under the same
    ///    keypair (FI-A01) — records the verdict as evidence either way,
    ///    and rejects creation if it fails (it shouldn't, for a freshly
    ///    generated keypair, but the check is real, not decorative).
    /// 4. Checks `policy.max_lifetime_secs > 0` (FI-A02) — same treatment.
    /// 5. Starts the lifecycle (`Creating -> Running`).
    pub async fn create_vm(&self, policy: AgentPolicy) -> Result<AavmSummary, AavmError> {
        let keypair = generate_hybrid_keypair().expect("hybrid PQC keypair generation does not fail");

        let pubkey_hash = blake3::hash(&keypair.verifying_key.ml_dsa_bytes());
        let mut hw_hash_truncated = [0u8; 20];
        hw_hash_truncated.copy_from_slice(&pubkey_hash.as_bytes()[..20]);
        let gdid = Gdid::from_parts(Gdid::VERSION_ML_DSA_65, Gdid::NS_ARKHE, &hw_hash_truncated, 0);

        let identity_verdict = {
            let signature = keypair.signing_key.sign(IDENTITY_CHALLENGE);
            match signature.and_then(|sig| keypair.verifying_key.verify(IDENTITY_CHALLENGE, &sig)) {
                Ok(()) => InvariantVerdict::Holds,
                Err(e) => InvariantVerdict::violated(e.to_string()),
            }
        };
        self.evidence_bus.store(AuditEvidence { invariant_id: "FI-A01", verdict: identity_verdict.clone() }).await;

        let policy_verdict = if policy.max_lifetime_secs > 0 {
            InvariantVerdict::Holds
        } else {
            InvariantVerdict::violated("policy.max_lifetime_secs must be greater than zero")
        };
        self.evidence_bus.store(AuditEvidence { invariant_id: "FI-A02", verdict: policy_verdict.clone() }).await;

        if let InvariantVerdict::Violated { reason } = &identity_verdict {
            return Err(AavmError::IdentityAttestationFailed(reason.clone()));
        }
        if let InvariantVerdict::Violated { reason } = &policy_verdict {
            return Err(AavmError::InvalidPolicy(reason.clone()));
        }

        let mut lifecycle = Lifecycle::new();
        lifecycle.start()?;

        let handle = AavmHandle { id: gdid.to_base58(), gdid, verifying_key: keypair.verifying_key, lifecycle, policy };
        let summary = AavmSummary::from(&handle);

        self.vms.lock().await.push(handle);
        Ok(summary)
    }

    /// Destroys the AAVM identified by `id`: `Running -> Terminating ->
    /// Destroyed`, then removes it from the manager. Records FI-A03
    /// (graceful termination) as evidence.
    pub async fn destroy_vm(&self, id: &str) -> Result<(), AavmError> {
        let mut vms = self.vms.lock().await;
        let pos = vms.iter().position(|h| h.id == id).ok_or_else(|| AavmError::NotFound(id.to_string()))?;

        let result: Result<(), InvalidTransition> = (|| {
            vms[pos].lifecycle.begin_terminate()?;
            vms[pos].lifecycle.destroy()?;
            Ok(())
        })();

        let verdict = match &result {
            Ok(()) => InvariantVerdict::Holds,
            Err(e) => InvariantVerdict::violated(e.to_string()),
        };
        self.evidence_bus.store(AuditEvidence { invariant_id: "FI-A03", verdict }).await;

        result?;
        vms.remove(pos);
        Ok(())
    }

    pub async fn list_vms(&self) -> Vec<AavmSummary> {
        self.vms.lock().await.iter().map(AavmSummary::from).collect()
    }

    /// Terminates every AAVM whose own [`AgentPolicy::is_expired`] says it
    /// has outlived `max_lifetime_secs`, regardless of its reported
    /// `LifecycleState` — a VM does not get to outlive its own policy just
    /// because nothing else asked it to stop. Returns the ids destroyed.
    pub async fn sweep_expired(&self) -> Vec<String> {
        let expired_ids: Vec<String> = {
            let vms = self.vms.lock().await;
            vms.iter().filter(|h| h.policy.is_expired(&h.lifecycle)).map(|h| h.id.clone()).collect()
        };

        let mut destroyed = Vec::new();
        for id in expired_ids {
            if self.destroy_vm(&id).await.is_ok() {
                destroyed.push(id);
            }
        }
        destroyed
    }

    /// Public verifying key for a live AAVM, if it exists — for callers
    /// that need to verify signatures the VM itself produces later (this
    /// manager doesn't retain the secret key to sign on its behalf; see
    /// the module doc comment).
    pub async fn verifying_key(&self, id: &str) -> Option<HybridVerifyingKey> {
        self.vms.lock().await.iter().find(|h| h.id == id).map(|h| h.verifying_key.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn permissive_policy() -> AgentPolicy {
        AgentPolicy { max_lifetime_secs: 3600, allowed_capabilities: vec!["web3.audit".to_string()] }
    }

    #[tokio::test]
    async fn create_vm_starts_running_and_is_listed() {
        let manager = AAVMManager::new(Arc::new(EvidenceBus::new()));
        let summary = manager.create_vm(permissive_policy()).await.unwrap();
        assert_eq!(summary.state, LifecycleState::Running);

        let listed = manager.list_vms().await;
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, summary.id);
    }

    #[tokio::test]
    async fn create_vm_rejects_zero_lifetime_policy() {
        let manager = AAVMManager::new(Arc::new(EvidenceBus::new()));
        let bad_policy = AgentPolicy { max_lifetime_secs: 0, allowed_capabilities: vec![] };
        assert!(matches!(manager.create_vm(bad_policy).await, Err(AavmError::InvalidPolicy(_))));
        assert!(manager.list_vms().await.is_empty());
    }

    #[tokio::test]
    async fn destroy_vm_removes_it_and_records_evidence() {
        let evidence_bus = Arc::new(EvidenceBus::new());
        let manager = AAVMManager::new(evidence_bus.clone());
        let summary = manager.create_vm(permissive_policy()).await.unwrap();

        manager.destroy_vm(&summary.id).await.unwrap();
        assert!(manager.list_vms().await.is_empty());

        let evidence = evidence_bus.all().await;
        assert!(evidence.iter().any(|e| e.invariant_id == "FI-A03" && e.verdict.holds()));
    }

    #[tokio::test]
    async fn destroy_vm_unknown_id_is_not_found() {
        let manager = AAVMManager::new(Arc::new(EvidenceBus::new()));
        assert!(matches!(manager.destroy_vm("nonexistent").await, Err(AavmError::NotFound(_))));
    }

    #[tokio::test]
    async fn destroy_vm_twice_fails_the_second_time() {
        let manager = AAVMManager::new(Arc::new(EvidenceBus::new()));
        let summary = manager.create_vm(permissive_policy()).await.unwrap();
        manager.destroy_vm(&summary.id).await.unwrap();
        // Already removed from the manager after the first destroy.
        assert!(matches!(manager.destroy_vm(&summary.id).await, Err(AavmError::NotFound(_))));
    }

    #[tokio::test]
    async fn evidence_bus_records_identity_and_policy_checks_on_create() {
        let evidence_bus = Arc::new(EvidenceBus::new());
        let manager = AAVMManager::new(evidence_bus.clone());
        manager.create_vm(permissive_policy()).await.unwrap();

        let evidence = evidence_bus.all().await;
        assert!(evidence.iter().any(|e| e.invariant_id == "FI-A01" && e.verdict.holds()));
        assert!(evidence.iter().any(|e| e.invariant_id == "FI-A02" && e.verdict.holds()));
    }

    #[tokio::test]
    async fn verifying_key_is_available_for_a_live_vm() {
        let manager = AAVMManager::new(Arc::new(EvidenceBus::new()));
        let summary = manager.create_vm(permissive_policy()).await.unwrap();
        assert!(manager.verifying_key(&summary.id).await.is_some());
        assert!(manager.verifying_key("nonexistent").await.is_none());
    }

    #[tokio::test]
    async fn sweep_expired_leaves_fresh_vms_alone() {
        let manager = AAVMManager::new(Arc::new(EvidenceBus::new()));
        manager.create_vm(permissive_policy()).await.unwrap();
        assert!(manager.sweep_expired().await.is_empty());
        assert_eq!(manager.list_vms().await.len(), 1);
    }

    #[tokio::test]
    async fn sweep_expired_destroys_vms_past_their_policy_lifetime() {
        let manager = AAVMManager::new(Arc::new(EvidenceBus::new()));
        let short_lived = AgentPolicy { max_lifetime_secs: 1, allowed_capabilities: vec![] };
        let summary = manager.create_vm(short_lived).await.unwrap();

        // age_secs() truncates to whole seconds (SystemTime::as_secs()), so
        // the margin needs to survive worst-case truncation at a second
        // boundary on both the "created_at" and "now" samples: for any real
        // elapsed time >= 2.0s, floor(now) - floor(created_at) is
        // guaranteed >= 2 (> the policy's 1-second limit) regardless of
        // where within a second either sample landed. 1.1s was not a safe
        // margin — this is what caused the flaky failure ("left: []") when
        // both samples happened to fall in adjacent seconds.
        tokio::time::sleep(std::time::Duration::from_millis(2500)).await;

        let destroyed = manager.sweep_expired().await;
        assert_eq!(destroyed, vec![summary.id]);
        assert!(manager.list_vms().await.is_empty());
    }
}
