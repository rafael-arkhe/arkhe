//! AAVM lifecycle state machine.
//!
//! The earlier sketch of this module had unconditional setter methods
//! (`start()`/`terminate()`/`destroy()`) that mutated `state` directly with
//! no validation — calling `destroy()` twice, or `start()` after
//! `destroy()`, would silently "succeed" and leave nonsensical state. This
//! version enforces the transition graph explicitly: `Creating -> Running
//! -> Terminating -> Destroyed`, no skipping or reversing.

use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleState {
    Creating,
    Running,
    Terminating,
    Destroyed,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("invalid lifecycle transition: {from:?} -> {to:?}")]
pub struct InvalidTransition {
    pub from: LifecycleState,
    pub to: LifecycleState,
}

#[derive(Debug)]
pub struct Lifecycle {
    state: LifecycleState,
    created_at: u64,
    terminated_at: Option<u64>,
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).expect("system clock is after 1970").as_secs()
}

impl Lifecycle {
    pub fn new() -> Self {
        Self { state: LifecycleState::Creating, created_at: now_secs(), terminated_at: None }
    }

    pub fn state(&self) -> LifecycleState {
        self.state
    }

    pub fn age_secs(&self) -> u64 {
        now_secs().saturating_sub(self.created_at)
    }

    pub fn terminated_at(&self) -> Option<u64> {
        self.terminated_at
    }

    /// `Creating -> Running`. Errors on any other starting state.
    pub fn start(&mut self) -> Result<(), InvalidTransition> {
        self.transition(LifecycleState::Running)
    }

    /// `Running -> Terminating`. Errors on any other starting state.
    pub fn begin_terminate(&mut self) -> Result<(), InvalidTransition> {
        self.transition(LifecycleState::Terminating)?;
        self.terminated_at = Some(now_secs());
        Ok(())
    }

    /// `Terminating -> Destroyed`. Errors on any other starting state.
    pub fn destroy(&mut self) -> Result<(), InvalidTransition> {
        self.transition(LifecycleState::Destroyed)
    }

    fn transition(&mut self, to: LifecycleState) -> Result<(), InvalidTransition> {
        let valid = matches!(
            (self.state, to),
            (LifecycleState::Creating, LifecycleState::Running)
                | (LifecycleState::Running, LifecycleState::Terminating)
                | (LifecycleState::Terminating, LifecycleState::Destroyed)
        );
        if !valid {
            return Err(InvalidTransition { from: self.state, to });
        }
        self.state = to;
        Ok(())
    }
}

impl Default for Lifecycle {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn happy_path_transitions_in_order() {
        let mut lc = Lifecycle::new();
        assert_eq!(lc.state(), LifecycleState::Creating);
        assert!(lc.start().is_ok());
        assert_eq!(lc.state(), LifecycleState::Running);
        assert!(lc.begin_terminate().is_ok());
        assert_eq!(lc.state(), LifecycleState::Terminating);
        assert!(lc.terminated_at().is_some());
        assert!(lc.destroy().is_ok());
        assert_eq!(lc.state(), LifecycleState::Destroyed);
    }

    #[test]
    fn skipping_running_is_rejected() {
        let mut lc = Lifecycle::new();
        assert_eq!(lc.begin_terminate(), Err(InvalidTransition { from: LifecycleState::Creating, to: LifecycleState::Terminating }));
    }

    #[test]
    fn destroying_twice_is_rejected() {
        let mut lc = Lifecycle::new();
        lc.start().unwrap();
        lc.begin_terminate().unwrap();
        lc.destroy().unwrap();
        assert_eq!(
            lc.destroy(),
            Err(InvalidTransition { from: LifecycleState::Destroyed, to: LifecycleState::Destroyed })
        );
    }

    #[test]
    fn starting_after_destroy_is_rejected() {
        let mut lc = Lifecycle::new();
        lc.start().unwrap();
        lc.begin_terminate().unwrap();
        lc.destroy().unwrap();
        assert_eq!(lc.start(), Err(InvalidTransition { from: LifecycleState::Destroyed, to: LifecycleState::Running }));
    }
}
