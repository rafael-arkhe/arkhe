//! FI-W07 — Não-replay de assinaturas via nonce monotônico.
//!
//! `∀ i,j, i < j ⇒ nonce_i < nonce_j`

use crate::InvariantVerdict;
use std::collections::HashMap;

/// Rastreador de nonces por conta — impede replay e permit front-running
/// (o atacante não pode reusar/antecipar um nonce já consumido).
#[derive(Debug, Default)]
pub struct NonceTracker {
    last_used: HashMap<String, u64>,
}

impl NonceTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// FI-W07: aceita `nonce` para `account` apenas se for estritamente maior
    /// que o último nonce consumido. Em caso de sucesso, avança o contador.
    pub fn consume(&mut self, account: &str, nonce: u64) -> InvariantVerdict {
        let last = self.last_used.get(account).copied();
        match last {
            Some(last) if nonce <= last => InvariantVerdict::violated(format!(
                "nonce {nonce} is not greater than last used nonce {last} for account '{account}'"
            )),
            _ => {
                self.last_used.insert(account.to_string(), nonce);
                InvariantVerdict::Holds
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strictly_increasing_nonces_hold() {
        let mut tracker = NonceTracker::new();
        assert!(tracker.consume("alice", 1).holds());
        assert!(tracker.consume("alice", 2).holds());
        assert!(tracker.consume("alice", 3).holds());
    }

    #[test]
    fn replayed_nonce_is_rejected() {
        let mut tracker = NonceTracker::new();
        assert!(tracker.consume("alice", 1).holds());
        assert!(!tracker.consume("alice", 1).holds());
    }

    #[test]
    fn out_of_order_nonce_is_rejected() {
        let mut tracker = NonceTracker::new();
        assert!(tracker.consume("alice", 5).holds());
        assert!(!tracker.consume("alice", 3).holds());
    }

    #[test]
    fn accounts_are_tracked_independently() {
        let mut tracker = NonceTracker::new();
        assert!(tracker.consume("alice", 1).holds());
        assert!(tracker.consume("bob", 1).holds());
    }
}
