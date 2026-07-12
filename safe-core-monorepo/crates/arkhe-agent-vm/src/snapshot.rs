//! FI-A05 — audit snapshots.
//!
//! Scoped deliberately narrow, per the explicit note this was requested
//! under: *"O BrainDumpLoop é um conceito; sua implementação pode começar
//! com snapshots simples (hash do estado da VM)."* This is that — a
//! BLAKE3 hash over `{gdid, lifecycle state, policy, timestamp}`, stored to
//! `EvidenceBus`, with an integrity check. It is **not** a resumable
//! process checkpoint: nothing in `arkhe-agent-vm` (or the `AgiCoordinator`
//! it spawns sessions around) persists an agent's conversation history
//! anywhere the manager can reach — `SessionHistory` lives inside
//! `AgiCoordinator` itself, private to it. "Restoring" a snapshot here
//! means confirming it hasn't been tampered with since capture, not
//! reviving a live agent from it.

use arkhe_core::hash::blake3_hash;
use arkhe_core::ArkheHash;
use arkhe_identity::Gdid;

use crate::lifecycle::LifecycleState;
use crate::policy::AgentPolicy;

/// A point-in-time, hash-verifiable record of an AAVM's public state. No
/// secret key material — same custody boundary as [`crate::manager::AavmSummary`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AavmSnapshot {
    pub id: String,
    pub gdid: Gdid,
    pub state: LifecycleState,
    pub age_secs: u64,
    pub max_lifetime_secs: u64,
    pub allowed_capabilities: Vec<String>,
    pub taken_at: u64,
    pub content_hash: ArkheHash,
}

impl AavmSnapshot {
    pub(crate) fn capture(
        id: String,
        gdid: Gdid,
        state: LifecycleState,
        age_secs: u64,
        policy: &AgentPolicy,
        taken_at: u64,
    ) -> Self {
        let payload = Self::payload_bytes(
            &id,
            &gdid,
            state,
            age_secs,
            policy.max_lifetime_secs,
            &policy.allowed_capabilities,
            taken_at,
        );
        let content_hash = blake3_hash(&payload);
        Self {
            id,
            gdid,
            state,
            age_secs,
            max_lifetime_secs: policy.max_lifetime_secs,
            allowed_capabilities: policy.allowed_capabilities.clone(),
            taken_at,
            content_hash,
        }
    }

    fn payload_bytes(
        id: &str,
        gdid: &Gdid,
        state: LifecycleState,
        age_secs: u64,
        max_lifetime_secs: u64,
        allowed_capabilities: &[String],
        taken_at: u64,
    ) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(id.as_bytes());
        buf.extend_from_slice(gdid.as_bytes());
        buf.push(state as u8);
        buf.extend_from_slice(&age_secs.to_be_bytes());
        buf.extend_from_slice(&max_lifetime_secs.to_be_bytes());
        for cap in allowed_capabilities {
            buf.extend_from_slice(cap.as_bytes());
            buf.push(0); // separator, so "ab","c" and "a","bc" don't collide
        }
        buf.extend_from_slice(&taken_at.to_be_bytes());
        buf
    }

    /// Recomputes `content_hash` from this snapshot's own fields and
    /// compares. `false` means the snapshot was mutated (in memory, or
    /// corrupted in whatever storage it came from) since [`Self::capture`]
    /// produced it — see the module doc comment for why this is the
    /// "restoration" check in this scoped-down version, not live-state
    /// resumption.
    pub fn verify_integrity(&self) -> bool {
        let payload = Self::payload_bytes(
            &self.id,
            &self.gdid,
            self.state,
            self.age_secs,
            self.max_lifetime_secs,
            &self.allowed_capabilities,
            self.taken_at,
        );
        blake3_hash(&payload) == self.content_hash
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> AavmSnapshot {
        let gdid = Gdid::from_fingerprint(&[7u8; 32], Gdid::NS_ARKHE, 1);
        let policy = AgentPolicy { max_lifetime_secs: 3600, allowed_capabilities: vec!["llm_inference".to_string()] };
        AavmSnapshot::capture("vm-1".to_string(), gdid, LifecycleState::Running, 42, &policy, 1_000_000)
    }

    #[test]
    fn fresh_snapshot_passes_integrity_check() {
        assert!(sample().verify_integrity());
    }

    #[test]
    fn tampered_age_fails_integrity_check() {
        let mut snap = sample();
        snap.age_secs += 1;
        assert!(!snap.verify_integrity());
    }

    #[test]
    fn tampered_state_fails_integrity_check() {
        let mut snap = sample();
        snap.state = LifecycleState::Destroyed;
        assert!(!snap.verify_integrity());
    }

    #[test]
    fn tampered_capabilities_fails_integrity_check() {
        let mut snap = sample();
        snap.allowed_capabilities.push("fs.write".to_string());
        assert!(!snap.verify_integrity());
    }

    #[test]
    fn capturing_the_same_state_twice_is_deterministic() {
        let gdid = Gdid::from_fingerprint(&[7u8; 32], Gdid::NS_ARKHE, 1);
        let policy = AgentPolicy { max_lifetime_secs: 3600, allowed_capabilities: vec!["llm_inference".to_string()] };
        let a = AavmSnapshot::capture("vm-1".to_string(), gdid, LifecycleState::Running, 42, &policy, 1_000_000);
        let b = AavmSnapshot::capture("vm-1".to_string(), gdid, LifecycleState::Running, 42, &policy, 1_000_000);
        assert_eq!(a.content_hash, b.content_hash);
    }
}
