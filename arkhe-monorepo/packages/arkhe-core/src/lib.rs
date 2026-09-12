//! arkhe-core — pure transition + append-only attestation.
//!
//! Two layers, deliberately separate:
//!   - `apply(state, event) -> state` : the mutable computation.
//!   - `Ledger`                       : the immutable attestation of it.

pub mod ledger;

pub use ledger::{Ledger, LedgerEntry, LedgerError, GENESIS_HASH};

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct State {
    pub tick: u64,
    pub phi_milli: u32, // 0..=1000
}

impl State {
    pub fn genesis() -> Self {
        Self { tick: 0, phi_milli: 0 }
    }

    pub fn phi(&self) -> f64 {
        self.phi_milli as f64 / 1000.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Event {
    Observe { phi_milli: u32 },
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TransitionError {
    #[error("phi_milli out of range: {0}")]
    PhiOutOfRange(u32),
}

/// Pure transition. Same (state, event) always yields the same result.
pub fn apply(state: &State, event: &Event) -> Result<State, TransitionError> {
    match event {
        Event::Observe { phi_milli } => {
            if *phi_milli > 1000 {
                return Err(TransitionError::PhiOutOfRange(*phi_milli));
            }
            Ok(State {
                tick: state.tick + 1,
                phi_milli: *phi_milli,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn genesis_state_is_zero() {
        let g = State::genesis();
        assert_eq!(g.tick, 0);
        assert_eq!(g.phi_milli, 0);
        assert_eq!(g.phi(), 0.0);
    }

    #[test]
    fn observe_applies_phi() {
        let g = State::genesis();
        let s = apply(&g, &Event::Observe { phi_milli: 330 }).unwrap();
        assert_eq!(s.tick, 1);
        assert_eq!(s.phi_milli, 330);
        assert!((s.phi() - 0.33).abs() < 1e-9);
    }

    #[test]
    fn observe_advances_tick() {
        let g = State::genesis();
        let s1 = apply(&g, &Event::Observe { phi_milli: 500 }).unwrap();
        let s2 = apply(&s1, &Event::Observe { phi_milli: 700 }).unwrap();
        assert_eq!(s2.tick, 2);
    }

    #[test]
    fn observe_rejects_out_of_range() {
        let g = State::genesis();
        assert!(matches!(
            apply(&g, &Event::Observe { phi_milli: 1001 }),
            Err(TransitionError::PhiOutOfRange(1001))
        ));
        // Boundary value 1000 is valid.
        assert!(apply(&g, &Event::Observe { phi_milli: 1000 }).is_ok());
    }

    #[test]
    fn apply_is_pure_and_deterministic() {
        let g = State::genesis();
        let a = apply(&g, &Event::Observe { phi_milli: 250 }).unwrap();
        let b = apply(&g, &Event::Observe { phi_milli: 250 }).unwrap();
        assert_eq!(a, b);
        // Original state unchanged (immutable borrow).
        assert_eq!(g, State::genesis());
    }
}