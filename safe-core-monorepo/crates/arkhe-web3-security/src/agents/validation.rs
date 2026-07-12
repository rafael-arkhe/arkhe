//! Validation stage: runs the checkable-at-runtime invariants
//! (Checks-Effects-Interactions ordering, nonce monotonicity) over the
//! audit input.
//!
//! **Not** wired to Lean 4 or Kani: those verify the *implementation* of
//! `contracts::reentrancy`/`wallets::signature` ahead of time (in CI, via
//! `lake build` / `cargo kani` — see `proofs/lean/` and `src/verify/`), not
//! at pipeline-run time against a specific transaction. Claiming this agent
//! "runs" a formal proof against live input would be the same kind of
//! unbacked claim the audit flagged — it doesn't, and doesn't need to:
//! the proofs establish that the checked functions behave correctly for
//! *all* inputs, once, not per-call.

use crate::contracts::reentrancy::{check_effects_interactions_order, Op};
use crate::wallets::signature::NonceTracker;
use crate::InvariantVerdict;

pub struct ValidationAgent;

impl ValidationAgent {
    pub fn check_cei(ops: &[Op]) -> InvariantVerdict {
        check_effects_interactions_order(ops)
    }

    /// Replays `nonce_events` (in order) through a fresh [`NonceTracker`]
    /// and returns one verdict per event.
    pub fn check_nonces(nonce_events: &[(String, u64)]) -> Vec<InvariantVerdict> {
        let mut tracker = NonceTracker::new();
        nonce_events
            .iter()
            .map(|(account, nonce)| tracker.consume(account, *nonce))
            .collect()
    }
}
