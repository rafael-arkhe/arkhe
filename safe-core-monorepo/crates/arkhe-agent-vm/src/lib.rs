//! Arkhe Agent VM (AAVM): lifecycle-managed, policy-constrained agent
//! identities backed by hybrid PQC signatures (`arkhe-crypto-pqc`) and
//! GDID (`arkhe-identity`), with creation/destruction checks recorded to
//! `arkhe-web3-security`'s `EvidenceBus` under invariant IDs `FI-A01`
//! (identity self-attestation), `FI-A02` (policy well-formedness),
//! `FI-A03` (graceful termination), and `FI-A04` (per-action policy gate —
//! see `session.rs`).
//!
//! [`manager::AAVMManager::spawn_agent_session`] is what makes an AAVM an
//! actual execution environment: it wraps a real `arkhe_agi::AgiCoordinator`
//! (a real agent loop — safety check, inference call, memory write, history
//! update) so that every `process()` call is gated by that VM's live
//! `AgentPolicy` and `LifecycleState`, not a permission taken once at spawn
//! time. See `session.rs` for exactly what is and isn't enforced (this is
//! in-process policy enforcement, not OS-level sandboxing — none exists
//! anywhere in this workspace).
//!
//! See `manager.rs` for the secret-custody model (the manager does not
//! retain a VM's signing key past `create_vm`) and
//! `../../docs/decisions/ADR-0001-two-pqc-crates.md` for why this crate
//! uses `arkhe-crypto-pqc` specifically, not `arkhe-pqc-core`.

#![deny(unsafe_code)]

pub mod lifecycle;
pub mod manager;
pub mod policy;
pub mod session;
pub mod snapshot;

pub use lifecycle::{InvalidTransition, Lifecycle, LifecycleState};
pub use manager::{AAVMManager, AavmError, AavmSummary};
pub use policy::AgentPolicy;
pub use session::PolicyVerifier;
pub use snapshot::AavmSnapshot;
