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
use arkhe_identity::{CapabilityBitmap, Gdid, GdidCertificate};
use arkhe_web3_security::agents::{AuditEvidence, EvidenceBus};
use arkhe_web3_security::InvariantVerdict;
use tokio::sync::{Mutex, RwLock};

use crate::lifecycle::{now_secs, InvalidTransition, Lifecycle, LifecycleState};
use crate::policy::AgentPolicy;
use crate::snapshot::AavmSnapshot;

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
    #[error("capability certificate is invalid: {0}")]
    CapabilityCertificateInvalid(String),
}

/// Maps `AgentPolicy::allowed_capabilities` (free-form action strings, this
/// crate's own vocabulary) onto `arkhe_identity::CapabilityBitmap`'s four
/// fixed hardware/network-level flags — a genuinely different vocabulary.
/// Only the capability strings below have a corresponding bit; anything
/// else in the policy is a real, enforced capability at the `PolicyVerifier`
/// level (FI-A04) but simply has no GDID-level bit to set, which is
/// expected, not a bug — the two systems cover different scopes.
fn capability_bitmap_from_policy(policy: &AgentPolicy) -> CapabilityBitmap {
    let mut bits = 0u16;
    for cap in &policy.allowed_capabilities {
        bits |= match cap.as_str() {
            "consensus" => CapabilityBitmap::CONSENSUS,
            "llm_inference" => CapabilityBitmap::INFERENCE,
            "governance_vote" => CapabilityBitmap::GOVERNANCE_VOTE,
            "hubble_relay" => CapabilityBitmap::HUBBLE_RELAY,
            _ => 0,
        };
    }
    CapabilityBitmap(bits)
}

/// Fixed challenge every AAVM signs at creation time to prove it holds the
/// secret key matching its own public identity — see the module doc
/// comment for what this does and doesn't cover.
const IDENTITY_CHALLENGE: &[u8] = b"arkhe-agent-vm/identity-attestation/v1";

struct AavmHandle {
    id: String,
    gdid: Gdid,
    verifying_key: HybridVerifyingKey,
    /// Shared, not owned outright: `session::PolicyVerifier` (built by
    /// [`AAVMManager::spawn_agent_session`]) holds a clone of this same
    /// `Arc`, so a `destroy_vm`/`sweep_expired` call takes effect
    /// immediately for any agent session already spawned from this VM —
    /// not just for VMs created after the fact. A plain owned `Lifecycle`
    /// (as in the first version of this module) couldn't do that: it would
    /// only ever reflect the state at the moment a session was spawned.
    lifecycle: Arc<RwLock<Lifecycle>>,
    policy: Arc<AgentPolicy>,
    /// GDID-level capability certificate (FI-A08) — a separate vocabulary
    /// from `policy.allowed_capabilities`, see [`capability_bitmap_from_policy`].
    capability_cert: GdidCertificate,
}

impl AavmHandle {
    async fn summary(&self) -> AavmSummary {
        let lifecycle = self.lifecycle.read().await;
        AavmSummary { id: self.id.clone(), gdid: self.gdid, state: lifecycle.state(), age_secs: lifecycle.age_secs() }
    }
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
    /// 5. Issues a GDID `CapabilityBitmap` certificate, self-signed with the
    ///    VM's Ed25519 sub-key, and self-verifies it (FI-A08).
    /// 6. Starts the lifecycle (`Creating -> Running`, FI-A07).
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
            tracing::warn!(id = %gdid.to_base58(), reason, "AAVM creation rejected: identity self-attestation failed");
            return Err(AavmError::IdentityAttestationFailed(reason.clone()));
        }
        if let InvariantVerdict::Violated { reason } = &policy_verdict {
            tracing::warn!(id = %gdid.to_base58(), reason, "AAVM creation rejected: invalid policy");
            return Err(AavmError::InvalidPolicy(reason.clone()));
        }

        let issued_at = now_secs();
        let expires_at = issued_at.saturating_add(policy.max_lifetime_secs);
        let capability_cert = GdidCertificate::issue(
            gdid,
            keypair.signing_key.ed25519_signing_key(),
            capability_bitmap_from_policy(&policy),
            issued_at,
            expires_at,
        );
        let cert_verdict = match capability_cert.verify(&keypair.signing_key.ed25519_signing_key().verifying_key(), issued_at) {
            Ok(()) => InvariantVerdict::Holds,
            Err(e) => InvariantVerdict::violated(e.to_string()),
        };
        self.evidence_bus.store(AuditEvidence { invariant_id: "FI-A08", verdict: cert_verdict.clone() }).await;
        if let InvariantVerdict::Violated { reason } = &cert_verdict {
            tracing::warn!(id = %gdid.to_base58(), reason, "AAVM creation rejected: capability certificate invalid");
            return Err(AavmError::CapabilityCertificateInvalid(reason.clone()));
        }

        let mut lifecycle = Lifecycle::new();
        let start_result = lifecycle.start();
        self.record_transition("Creating->Running", &start_result).await;
        if let Err(e) = start_result {
            tracing::warn!(id = %gdid.to_base58(), error = %e, "AAVM creation rejected: lifecycle start failed");
            return Err(e.into());
        }

        let handle = AavmHandle {
            id: gdid.to_base58(),
            gdid,
            verifying_key: keypair.verifying_key,
            lifecycle: Arc::new(RwLock::new(lifecycle)),
            policy: Arc::new(policy),
            capability_cert,
        };
        let summary = handle.summary().await;

        self.vms.lock().await.push(handle);
        tracing::info!(id = %summary.id, state = ?summary.state, "AAVM created");
        Ok(summary)
    }

    /// Records FI-A07 (per-transition evidence) for a single `Lifecycle`
    /// transition attempt — the general "every state transition generates
    /// evidence" principle, at the granularity of individual transitions
    /// rather than only the higher-level FI-A01..FI-A03 summaries.
    async fn record_transition(&self, label: &str, result: &Result<(), InvalidTransition>) {
        let verdict = match result {
            Ok(()) => InvariantVerdict::Holds,
            Err(e) => InvariantVerdict::violated(format!("{label}: {e}")),
        };
        self.evidence_bus.store(AuditEvidence { invariant_id: "FI-A07", verdict }).await;
    }

    /// Destroys the AAVM identified by `id`, then removes it from the
    /// manager. Records FI-A03 (graceful termination) as evidence. Takes
    /// effect immediately for any agent session already spawned from this
    /// VM via [`Self::spawn_agent_session`] — see [`AavmHandle::lifecycle`].
    ///
    /// From `Running`, this goes through the full `Running -> Terminating
    /// -> Destroyed` path (two FI-A07 entries). From [`LifecycleState::SafeClosed`]
    /// (a prior [`Self::fault_vm`] call) it goes directly to `Destroyed` —
    /// there is no `Terminating` step to skip, by design (see
    /// `LifecycleState::SafeClosed`'s doc comment).
    pub async fn destroy_vm(&self, id: &str) -> Result<(), AavmError> {
        let mut vms = self.vms.lock().await;
        let pos = vms.iter().position(|h| h.id == id).ok_or_else(|| AavmError::NotFound(id.to_string()))?;

        let current_state = vms[pos].lifecycle.read().await.state();

        if current_state == LifecycleState::Running {
            let begin_result = {
                let mut lifecycle = vms[pos].lifecycle.write().await;
                lifecycle.begin_terminate()
            };
            self.record_transition("Running->Terminating", &begin_result).await;
            if let Err(e) = begin_result {
                self.evidence_bus
                    .store(AuditEvidence { invariant_id: "FI-A03", verdict: InvariantVerdict::violated(e.to_string()) })
                    .await;
                tracing::warn!(id, error = %e, "AAVM destroy failed");
                return Err(e.into());
            }
        }

        let destroy_result = {
            let mut lifecycle = vms[pos].lifecycle.write().await;
            lifecycle.destroy()
        };
        self.record_transition(&format!("{current_state:?}->Destroyed"), &destroy_result).await;

        let verdict = match &destroy_result {
            Ok(()) => InvariantVerdict::Holds,
            Err(e) => InvariantVerdict::violated(e.to_string()),
        };
        self.evidence_bus.store(AuditEvidence { invariant_id: "FI-A03", verdict }).await;

        if let Err(e) = &destroy_result {
            tracing::warn!(id, error = %e, "AAVM destroy failed");
            destroy_result?;
        }
        vms.remove(pos);
        tracing::info!(id, "AAVM destroyed");
        Ok(())
    }

    /// FI-023 — fail-closed: forces AAVM `id` immediately into
    /// [`LifecycleState::SafeClosed`], skipping any graceful-shutdown path,
    /// and records the verdict as FI-A09. This is a manual/explicit
    /// trigger — a caller (supervisory code that itself detects a critical
    /// failure) invokes it; this crate does not attempt automatic failure
    /// detection, since defining "critical component failure" system-wide
    /// is a distinct, much larger design question this method doesn't
    /// answer. Once faulted, [`Self::spawn_agent_session`]'s
    /// `PolicyVerifier` rejects every action immediately (any non-`Running`
    /// state does), and [`Self::destroy_vm`] is the only way out.
    pub async fn fault_vm(&self, id: &str) -> Result<(), AavmError> {
        let vms = self.vms.lock().await;
        let handle = vms.iter().find(|h| h.id == id).ok_or_else(|| AavmError::NotFound(id.to_string()))?;

        let result = {
            let mut lifecycle = handle.lifecycle.write().await;
            lifecycle.fault()
        };

        let verdict = match &result {
            Ok(()) => InvariantVerdict::Holds,
            Err(e) => InvariantVerdict::violated(e.to_string()),
        };
        self.evidence_bus.store(AuditEvidence { invariant_id: "FI-A09", verdict }).await;

        if let Err(e) = &result {
            tracing::warn!(id, error = %e, "AAVM fault_vm failed");
        } else {
            tracing::warn!(id, "AAVM forced to SafeClosed (fail-closed)");
        }
        result?;
        Ok(())
    }

    pub async fn list_vms(&self) -> Vec<AavmSummary> {
        let vms = self.vms.lock().await;
        let mut out = Vec::with_capacity(vms.len());
        for h in vms.iter() {
            out.push(h.summary().await);
        }
        out
    }

    /// Terminates every AAVM whose own [`AgentPolicy::is_expired`] says it
    /// has outlived `max_lifetime_secs`, regardless of its reported
    /// `LifecycleState` — a VM does not get to outlive its own policy just
    /// because nothing else asked it to stop. Returns the ids destroyed.
    pub async fn sweep_expired(&self) -> Vec<String> {
        let expired_ids: Vec<String> = {
            let vms = self.vms.lock().await;
            let mut ids = Vec::new();
            for h in vms.iter() {
                if h.policy.is_expired(&*h.lifecycle.read().await) {
                    ids.push(h.id.clone());
                }
            }
            ids
        };

        let mut destroyed = Vec::new();
        for id in expired_ids {
            if self.destroy_vm(&id).await.is_ok() {
                destroyed.push(id);
            }
        }
        destroyed
    }

    /// FI-055: sets (or clears, via `None`) an absolute-unix-timestamp
    /// deadline for AAVM `id`'s current unit of work. Distinct from
    /// `AgentPolicy::max_lifetime_secs`/`sweep_expired`: that's a
    /// VM-level "don't outlive your whole policy" bound, this is a
    /// per-task "this specific piece of work should finish by T" bound —
    /// a caller (e.g. whatever drives `spawn_agent_session`) sets it before
    /// starting a bounded task and clears it (`None`) when done, so a
    /// service/monitoring task that never sets a deadline never times out.
    pub async fn set_deadline(&self, id: &str, deadline: Option<u64>) -> Result<(), AavmError> {
        let vms = self.vms.lock().await;
        let handle = vms.iter().find(|h| h.id == id).ok_or_else(|| AavmError::NotFound(id.to_string()))?;
        handle.lifecycle.write().await.set_deadline(deadline);
        Ok(())
    }

    /// FI-055: faults (FI-023's fail-closed path — see
    /// `LifecycleState::SafeClosed`) every `Running` AAVM whose deadline
    /// has passed, records FI-A10 in addition to `fault_vm`'s own FI-A09,
    /// and returns the ids faulted. A VM with no deadline set is never
    /// touched. Mirrors `sweep_expired`'s shape.
    pub async fn sweep_timed_out(&self) -> Vec<String> {
        let timed_out_ids: Vec<String> = {
            let vms = self.vms.lock().await;
            let mut ids = Vec::new();
            for h in vms.iter() {
                let lifecycle = h.lifecycle.read().await;
                if lifecycle.state() == LifecycleState::Running && lifecycle.is_past_deadline() {
                    ids.push(h.id.clone());
                }
            }
            ids
        };

        let mut faulted = Vec::new();
        for id in timed_out_ids {
            let result = self.fault_vm(&id).await;
            let verdict = match &result {
                Ok(()) => InvariantVerdict::Holds,
                Err(e) => InvariantVerdict::violated(e.to_string()),
            };
            self.evidence_bus.store(AuditEvidence { invariant_id: "FI-A10", verdict }).await;
            if result.is_ok() {
                tracing::warn!(id, "AAVM timed out (FI-055) — faulted via FI-023");
                faulted.push(id);
            }
        }
        faulted
    }

    /// Public verifying key for a live AAVM, if it exists — for callers
    /// that need to verify signatures the VM itself produces later (this
    /// manager doesn't retain the secret key to sign on its behalf; see
    /// the module doc comment).
    pub async fn verifying_key(&self, id: &str) -> Option<HybridVerifyingKey> {
        self.vms.lock().await.iter().find(|h| h.id == id).map(|h| h.verifying_key.clone())
    }

    /// FI-A05: captures an [`AavmSnapshot`] of `id`'s current public state
    /// and records its integrity verdict to the evidence bus. See
    /// `snapshot.rs`'s module doc comment for the deliberately narrow scope
    /// (a state hash, not a resumable checkpoint).
    pub async fn snapshot(&self, id: &str) -> Result<AavmSnapshot, AavmError> {
        let vms = self.vms.lock().await;
        let handle = vms.iter().find(|h| h.id == id).ok_or_else(|| AavmError::NotFound(id.to_string()))?;
        let lifecycle = handle.lifecycle.read().await;
        let snapshot = AavmSnapshot::capture(
            handle.id.clone(),
            handle.gdid,
            lifecycle.state(),
            lifecycle.age_secs(),
            &handle.policy,
            now_secs(),
        );
        drop(lifecycle);
        drop(vms);

        let verdict = if snapshot.verify_integrity() {
            InvariantVerdict::Holds
        } else {
            InvariantVerdict::violated("snapshot failed its own integrity check immediately after capture")
        };
        self.evidence_bus.store(AuditEvidence { invariant_id: "FI-A05", verdict: verdict.clone() }).await;
        tracing::info!(id, holds = verdict.holds(), "AAVM snapshot captured");

        Ok(snapshot)
    }

    /// The GDID capability certificate issued at `create_vm` time, if `id`
    /// is a live AAVM. No secret material — safe to hand to callers outside
    /// the manager, same custody boundary as [`AavmSummary`].
    pub async fn capability_certificate(&self, id: &str) -> Option<GdidCertificate> {
        self.vms.lock().await.iter().find(|h| h.id == id).map(|h| h.capability_cert.clone())
    }

    /// Returns the shared `Lifecycle` and `AgentPolicy` handles for a live
    /// AAVM — used by [`Self::spawn_agent_session`] to build a
    /// `SafetyVerifier` that enforces this VM's *live* state, not a
    /// point-in-time snapshot.
    async fn session_handles(&self, id: &str) -> Option<(Arc<RwLock<Lifecycle>>, Arc<AgentPolicy>)> {
        self.vms.lock().await.iter().find(|h| h.id == id).map(|h| (h.lifecycle.clone(), h.policy.clone()))
    }

    /// Spawns an `AgiCoordinator` "inside" AAVM `id`: every `process()` call
    /// the coordinator makes is gated by `id`'s live `AgentPolicy` and
    /// `LifecycleState` via [`crate::session::PolicyVerifier`], which
    /// implements `arkhe_core::SafetyVerifier` — the exact hook
    /// `AgiCoordinator` already calls before every inference request. If
    /// `destroy_vm`/`sweep_expired` later terminates this VM, the *next*
    /// `process()` call on the returned coordinator is rejected — the
    /// enforcement is live, not a permission check taken once at spawn
    /// time. See `crate::session` for what's checked and what isn't (this
    /// is in-process policy enforcement, not OS-level sandboxing).
    pub async fn spawn_agent_session<M, I>(
        &self,
        id: &str,
        memory: Arc<M>,
        inference: Arc<I>,
        session_id: &str,
        system_prompt: &str,
    ) -> Result<arkhe_agi::AgiCoordinator<crate::session::PolicyVerifier, M, I>, AavmError>
    where
        M: arkhe_core::AgentMemory,
        I: arkhe_inference::InferenceEngine,
    {
        let (lifecycle, policy) =
            self.session_handles(id).await.ok_or_else(|| AavmError::NotFound(id.to_string()))?;

        let verifier = Arc::new(crate::session::PolicyVerifier::new(
            id.to_string(),
            policy,
            lifecycle,
            self.evidence_bus.clone(),
        ));

        let evaluator = Arc::new(arkhe_session_evaluator::SessionEvaluator::new());
        Ok(arkhe_agi::AgiCoordinator::new(verifier, memory, inference, evaluator, session_id, system_prompt))
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

    fn null_inference() -> Arc<arkhe_inference::NullEngine> {
        Arc::new(arkhe_inference::NullEngine::new(arkhe_inference::ModelId::new("test", "null")))
    }

    #[tokio::test]
    async fn spawned_agent_session_rejects_when_llm_inference_not_in_policy() {
        let manager = AAVMManager::new(Arc::new(EvidenceBus::new()));
        let summary = manager.create_vm(permissive_policy()).await.unwrap();

        let coordinator = manager
            .spawn_agent_session(
                &summary.id,
                Arc::new(arkhe_core::InMemoryAgentMemory::new()),
                null_inference(),
                "test-session",
                "You are helpful.",
            )
            .await
            .unwrap();

        // permissive_policy() only lists "web3.audit" — AgiCoordinator
        // always checks the fixed action "llm_inference", so this must be
        // rejected even though the VM itself is Running.
        assert!(coordinator.process("hello").await.is_err());
    }

    #[tokio::test]
    async fn spawned_agent_session_processes_when_llm_inference_is_allowed() {
        let manager = AAVMManager::new(Arc::new(EvidenceBus::new()));
        let policy = AgentPolicy { max_lifetime_secs: 3600, allowed_capabilities: vec!["llm_inference".to_string()] };
        let summary = manager.create_vm(policy).await.unwrap();

        let coordinator = manager
            .spawn_agent_session(
                &summary.id,
                Arc::new(arkhe_core::InMemoryAgentMemory::new()),
                null_inference(),
                "test-session",
                "You are helpful.",
            )
            .await
            .unwrap();

        let response = coordinator.process("hello").await.unwrap();
        assert!(!response.is_empty());
    }

    #[tokio::test]
    async fn destroying_the_vm_blocks_further_process_calls_on_an_already_spawned_session() {
        let manager = AAVMManager::new(Arc::new(EvidenceBus::new()));
        let policy = AgentPolicy { max_lifetime_secs: 3600, allowed_capabilities: vec!["llm_inference".to_string()] };
        let summary = manager.create_vm(policy).await.unwrap();

        let coordinator = manager
            .spawn_agent_session(
                &summary.id,
                Arc::new(arkhe_core::InMemoryAgentMemory::new()),
                null_inference(),
                "test-session",
                "You are helpful.",
            )
            .await
            .unwrap();

        assert!(coordinator.process("first call, before destroy").await.is_ok());

        manager.destroy_vm(&summary.id).await.unwrap();

        // The coordinator handle is still held by the caller (it doesn't
        // know the VM was destroyed) — but the *shared* Lifecycle it reads
        // through PolicyVerifier reflects the change immediately.
        assert!(coordinator.process("second call, after destroy").await.is_err());
    }

    #[tokio::test]
    async fn spawn_agent_session_fails_for_unknown_vm() {
        let manager = AAVMManager::new(Arc::new(EvidenceBus::new()));
        let result = manager
            .spawn_agent_session(
                "nonexistent",
                Arc::new(arkhe_core::InMemoryAgentMemory::new()),
                null_inference(),
                "test-session",
                "sys",
            )
            .await;
        assert!(matches!(result, Err(AavmError::NotFound(_))));
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

    #[tokio::test]
    async fn snapshot_captures_live_vm_state() {
        let manager = AAVMManager::new(Arc::new(EvidenceBus::new()));
        let summary = manager.create_vm(permissive_policy()).await.unwrap();

        let snap = manager.snapshot(&summary.id).await.unwrap();
        assert_eq!(snap.id, summary.id);
        assert_eq!(snap.gdid, summary.gdid);
        assert_eq!(snap.state, LifecycleState::Running);
        assert!(snap.verify_integrity());
    }

    #[tokio::test]
    async fn snapshot_records_fi_a05_evidence() {
        let evidence_bus = Arc::new(EvidenceBus::new());
        let manager = AAVMManager::new(evidence_bus.clone());
        let summary = manager.create_vm(permissive_policy()).await.unwrap();

        manager.snapshot(&summary.id).await.unwrap();

        let evidence = evidence_bus.all().await;
        assert!(evidence.iter().any(|e| e.invariant_id == "FI-A05" && e.verdict.holds()));
    }

    #[tokio::test]
    async fn snapshot_reflects_state_after_destroy() {
        let manager = AAVMManager::new(Arc::new(EvidenceBus::new()));
        let summary = manager.create_vm(permissive_policy()).await.unwrap();

        // Snapshot after destroy_vm removes the handle from the manager —
        // there's nothing left to snapshot, matching NotFound elsewhere.
        manager.destroy_vm(&summary.id).await.unwrap();
        assert!(matches!(manager.snapshot(&summary.id).await, Err(AavmError::NotFound(_))));
    }

    #[tokio::test]
    async fn snapshot_fails_for_unknown_vm() {
        let manager = AAVMManager::new(Arc::new(EvidenceBus::new()));
        assert!(matches!(manager.snapshot("nonexistent").await, Err(AavmError::NotFound(_))));
    }

    #[tokio::test]
    async fn create_vm_issues_a_self_verifying_capability_certificate() {
        let manager = AAVMManager::new(Arc::new(EvidenceBus::new()));
        let summary = manager.create_vm(permissive_policy()).await.unwrap();

        let cert = manager.capability_certificate(&summary.id).await.unwrap();
        assert_eq!(cert.gdid, summary.gdid);
        let issuer_key = ed25519_dalek::VerifyingKey::from_bytes(&cert.pubkey).unwrap();
        assert!(cert.verify(&issuer_key, cert.issued_at).is_ok());
    }

    #[tokio::test]
    async fn capability_certificate_maps_known_policy_strings_to_bitmap_flags() {
        let manager = AAVMManager::new(Arc::new(EvidenceBus::new()));
        let policy = AgentPolicy {
            max_lifetime_secs: 3600,
            allowed_capabilities: vec!["consensus".to_string(), "llm_inference".to_string(), "fs.write".to_string()],
        };
        let summary = manager.create_vm(policy).await.unwrap();

        let cert = manager.capability_certificate(&summary.id).await.unwrap();
        assert!(cert.capabilities.has(arkhe_identity::CapabilityBitmap::CONSENSUS));
        assert!(cert.capabilities.has(arkhe_identity::CapabilityBitmap::INFERENCE));
        assert!(!cert.capabilities.has(arkhe_identity::CapabilityBitmap::GOVERNANCE_VOTE));
        // "fs.write" has no GDID-level bit — expected, different vocabulary,
        // not a bug (see capability_bitmap_from_policy's doc comment).
    }

    #[tokio::test]
    async fn capability_certificate_is_absent_for_unknown_vm() {
        let manager = AAVMManager::new(Arc::new(EvidenceBus::new()));
        assert!(manager.capability_certificate("nonexistent").await.is_none());
    }

    #[tokio::test]
    async fn create_vm_records_fi_a07_and_fi_a08_evidence() {
        let evidence_bus = Arc::new(EvidenceBus::new());
        let manager = AAVMManager::new(evidence_bus.clone());
        manager.create_vm(permissive_policy()).await.unwrap();

        let evidence = evidence_bus.all().await;
        assert!(evidence.iter().any(|e| e.invariant_id == "FI-A07" && e.verdict.holds()));
        assert!(evidence.iter().any(|e| e.invariant_id == "FI-A08" && e.verdict.holds()));
    }

    #[tokio::test]
    async fn destroy_vm_records_two_fi_a07_transitions() {
        let evidence_bus = Arc::new(EvidenceBus::new());
        let manager = AAVMManager::new(evidence_bus.clone());
        let summary = manager.create_vm(permissive_policy()).await.unwrap();

        let before = evidence_bus.all().await.iter().filter(|e| e.invariant_id == "FI-A07").count();
        manager.destroy_vm(&summary.id).await.unwrap();
        let after = evidence_bus.all().await.iter().filter(|e| e.invariant_id == "FI-A07").count();

        // 1 from create_vm's Creating->Running, +2 from destroy_vm's
        // Running->Terminating and Terminating->Destroyed.
        assert_eq!(before, 1);
        assert_eq!(after, 3);
    }

    #[tokio::test]
    async fn fault_vm_forces_safe_closed_and_records_fi_a09() {
        let evidence_bus = Arc::new(EvidenceBus::new());
        let manager = AAVMManager::new(evidence_bus.clone());
        let summary = manager.create_vm(permissive_policy()).await.unwrap();

        manager.fault_vm(&summary.id).await.unwrap();

        let listed = manager.list_vms().await;
        assert_eq!(listed[0].state, LifecycleState::SafeClosed);

        let evidence = evidence_bus.all().await;
        assert!(evidence.iter().any(|e| e.invariant_id == "FI-A09" && e.verdict.holds()));
    }

    #[tokio::test]
    async fn faulted_vm_can_still_be_destroyed() {
        let manager = AAVMManager::new(Arc::new(EvidenceBus::new()));
        let summary = manager.create_vm(permissive_policy()).await.unwrap();

        manager.fault_vm(&summary.id).await.unwrap();
        manager.destroy_vm(&summary.id).await.unwrap();

        assert!(manager.list_vms().await.is_empty());
    }

    #[tokio::test]
    async fn faulted_vm_rejects_further_agent_actions() {
        let manager = AAVMManager::new(Arc::new(EvidenceBus::new()));
        let policy = AgentPolicy { max_lifetime_secs: 3600, allowed_capabilities: vec!["llm_inference".to_string()] };
        let summary = manager.create_vm(policy).await.unwrap();

        let coordinator = manager
            .spawn_agent_session(
                &summary.id,
                Arc::new(arkhe_core::InMemoryAgentMemory::new()),
                null_inference(),
                "test-session",
                "You are helpful.",
            )
            .await
            .unwrap();

        assert!(coordinator.process("before fault").await.is_ok());
        manager.fault_vm(&summary.id).await.unwrap();
        assert!(coordinator.process("after fault").await.is_err());
    }

    #[tokio::test]
    async fn fault_vm_fails_for_unknown_vm() {
        let manager = AAVMManager::new(Arc::new(EvidenceBus::new()));
        assert!(matches!(manager.fault_vm("nonexistent").await, Err(AavmError::NotFound(_))));
    }

    #[tokio::test]
    async fn sweep_timed_out_leaves_vms_without_a_deadline_alone() {
        let manager = AAVMManager::new(Arc::new(EvidenceBus::new()));
        manager.create_vm(permissive_policy()).await.unwrap();
        assert!(manager.sweep_timed_out().await.is_empty());
        assert_eq!(manager.list_vms().await[0].state, LifecycleState::Running);
    }

    #[tokio::test]
    async fn sweep_timed_out_leaves_vms_with_a_future_deadline_alone() {
        let manager = AAVMManager::new(Arc::new(EvidenceBus::new()));
        let summary = manager.create_vm(permissive_policy()).await.unwrap();
        manager.set_deadline(&summary.id, Some(now_secs() + 3600)).await.unwrap();
        assert!(manager.sweep_timed_out().await.is_empty());
    }

    #[tokio::test]
    async fn sweep_timed_out_faults_vms_past_their_deadline_and_records_evidence() {
        let evidence_bus = Arc::new(EvidenceBus::new());
        let manager = AAVMManager::new(evidence_bus.clone());
        let summary = manager.create_vm(permissive_policy()).await.unwrap();
        // No sleep needed — set a deadline that's already in the past,
        // same deterministic pattern as the age_secs()-truncation lesson
        // learned earlier this session.
        manager.set_deadline(&summary.id, Some(now_secs().saturating_sub(10))).await.unwrap();

        let faulted = manager.sweep_timed_out().await;
        assert_eq!(faulted, vec![summary.id.clone()]);
        assert_eq!(manager.list_vms().await[0].state, LifecycleState::SafeClosed);

        let evidence = evidence_bus.all().await;
        assert!(evidence.iter().any(|e| e.invariant_id == "FI-A10" && e.verdict.holds()));
        assert!(evidence.iter().any(|e| e.invariant_id == "FI-A09" && e.verdict.holds()));
    }

    #[tokio::test]
    async fn timed_out_vm_rejects_further_agent_actions() {
        let manager = AAVMManager::new(Arc::new(EvidenceBus::new()));
        let policy = AgentPolicy { max_lifetime_secs: 3600, allowed_capabilities: vec!["llm_inference".to_string()] };
        let summary = manager.create_vm(policy).await.unwrap();

        let coordinator = manager
            .spawn_agent_session(
                &summary.id,
                Arc::new(arkhe_core::InMemoryAgentMemory::new()),
                null_inference(),
                "test-session",
                "You are helpful.",
            )
            .await
            .unwrap();

        assert!(coordinator.process("before timeout").await.is_ok());
        manager.set_deadline(&summary.id, Some(now_secs().saturating_sub(10))).await.unwrap();
        manager.sweep_timed_out().await;
        assert!(coordinator.process("after timeout").await.is_err());
    }

    #[tokio::test]
    async fn set_deadline_fails_for_unknown_vm() {
        let manager = AAVMManager::new(Arc::new(EvidenceBus::new()));
        assert!(matches!(manager.set_deadline("nonexistent", Some(0)).await, Err(AavmError::NotFound(_))));
    }
}
