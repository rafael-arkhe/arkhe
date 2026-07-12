//! FI-075 — every message has a unique nonce per sender.
//!
//! Same strictly-monotonic-nonce pattern as FI-007
//! (`arkhe-web3-security::wallets::signature::NonceTracker`, formally
//! proven in `NonceMonotonic.lean`), reimplemented locally here rather than
//! depending on `arkhe-web3-security` from this lower-level crate — that
//! would be backwards layering (a generic networking primitive depending on
//! a Web3-specific invariants crate for ~15 lines of logic).

use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct MessageNonceTracker {
    last_used: HashMap<String, u64>,
}

impl MessageNonceTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Accepts `nonce` for `sender_id` only if strictly greater than the
    /// last nonce seen from that sender. Returns `true` (and advances the
    /// tracker) on acceptance, `false` on a replay/out-of-order nonce.
    pub fn consume(&mut self, sender_id: &str, nonce: u64) -> bool {
        let last = self.last_used.get(sender_id).copied();
        match last {
            Some(last) if nonce <= last => false,
            _ => {
                self.last_used.insert(sender_id.to_string(), nonce);
                true
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strictly_increasing_nonces_are_accepted() {
        let mut t = MessageNonceTracker::new();
        assert!(t.consume("vm-1", 1));
        assert!(t.consume("vm-1", 2));
    }

    #[test]
    fn replayed_nonce_is_rejected() {
        let mut t = MessageNonceTracker::new();
        assert!(t.consume("vm-1", 1));
        assert!(!t.consume("vm-1", 1));
    }

    #[test]
    fn out_of_order_nonce_is_rejected() {
        let mut t = MessageNonceTracker::new();
        assert!(t.consume("vm-1", 5));
        assert!(!t.consume("vm-1", 3));
    }

    #[test]
    fn senders_are_tracked_independently() {
        let mut t = MessageNonceTracker::new();
        assert!(t.consume("vm-1", 1));
        assert!(t.consume("vm-2", 1));
    }
}
