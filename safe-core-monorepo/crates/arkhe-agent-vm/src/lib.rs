//! Arkhe Agent VM (AAVM): lifecycle-managed, policy-constrained agent
//! identities backed by hybrid PQC signatures (`arkhe-crypto-pqc`) and
//! GDID (`arkhe-identity`), with creation/destruction checks recorded to
//! `arkhe-web3-security`'s `EvidenceBus` under invariant IDs `FI-A01`
//! (identity self-attestation), `FI-A02` (policy well-formedness), and
//! `FI-A03` (graceful termination).
//!
//! See `manager.rs` for the secret-custody model (the manager does not
//! retain a VM's signing key past `create_vm`) and
//! `../../docs/decisions/ADR-0001-two-pqc-crates.md` for why this crate
//! uses `arkhe-crypto-pqc` specifically, not `arkhe-pqc-core`.

#![deny(unsafe_code)]

pub mod lifecycle;
pub mod manager;
pub mod policy;

pub use lifecycle::{InvalidTransition, Lifecycle, LifecycleState};
pub use manager::{AAVMManager, AavmError, AavmSummary};
pub use policy::AgentPolicy;
