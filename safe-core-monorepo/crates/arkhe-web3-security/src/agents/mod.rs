//! Continuous audit pipeline: Recon → Hunting → Validation → GapFilling,
//! with an async [`evidence_bus::EvidenceBus`] recording every verdict.
//!
//! This operates entirely over the crate's own already-tested invariant
//! checks (`analyzers::reentrancy`, `contracts::reentrancy`,
//! `wallets::signature::NonceTracker`) — it does not parse real Solidity
//! bytecode/AST (that would need a full EVM disassembler, out of scope here
//! and not something to fabricate), and it does not invoke Lean or Kani at
//! runtime (those are checked separately, ahead of time, in CI — see
//! `.github/workflows/web3-security.yml`). [`validation::ValidationAgent`]
//! is explicit about this in its own doc comment.

pub mod evidence_bus;
pub mod gap_filling;
pub mod hunting;
pub mod pipeline;
pub mod recon;
pub mod validation;

pub use evidence_bus::{AuditEvidence, EvidenceBus};
pub use gap_filling::GapFillingAgent;
pub use hunting::HuntingAgent;
pub use pipeline::{AuditError, AuditInput, AuditPipeline, AuditReport};
pub use recon::{FunctionSignature, ReconAgent};
pub use validation::ValidationAgent;
