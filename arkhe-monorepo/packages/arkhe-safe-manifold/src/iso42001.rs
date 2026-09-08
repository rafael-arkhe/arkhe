//! ISO 42001 AI Management System — PDCA Cycle.
//!
//! Tracks the Plan-Do-Check-Act cycle for AI management system compliance.

use serde::{Deserialize, Serialize};
use crate::invariants::SystemState;

/// ISO 42001 PDCA phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Iso42001Phase {
    Plan,
    Do,
    Check,
    Act,
}

impl Iso42001Phase {
    pub fn all() -> Vec<Self> {
        vec![Self::Plan, Self::Do, Self::Check, Self::Act]
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Plan => "plan",
            Self::Do => "do",
            Self::Check => "check",
            Self::Act => "act",
        }
    }

    pub fn next(&self) -> Self {
        match self {
            Self::Plan => Self::Do,
            Self::Do => Self::Check,
            Self::Check => Self::Act,
            Self::Act => Self::Plan,
        }
    }
}

/// PDCA state tracking.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PdcaState {
    pub current_phase: Iso42001Phase,
    pub cycle_count: u64,
    pub compliance_score: f64,
    pub last_audit_timestamp: Option<String>,
}

impl Default for PdcaState {
    fn default() -> Self {
        Self {
            current_phase: Iso42001Phase::Plan,
            cycle_count: 0,
            compliance_score: 1.0,
            last_audit_timestamp: None,
        }
    }
}

impl PdcaState {
    /// Advance to the next PDCA phase.
    pub fn advance(&mut self) {
        if self.current_phase == Iso42001Phase::Act {
            self.cycle_count += 1;
        }
        self.current_phase = self.current_phase.next();
    }

    /// Record an audit timestamp.
    pub fn record_audit(&mut self, timestamp: impl Into<String>) {
        self.last_audit_timestamp = Some(timestamp.into());
    }

    /// Check if the system is in a compliant state.
    pub fn is_compliant(&self, state: &SystemState) -> bool {
        state.check_all() && self.compliance_score >= 0.9
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::invariants::SystemConfig;

    #[test]
    fn pdca_default_is_plan() {
        let state = PdcaState::default();
        assert_eq!(state.current_phase, Iso42001Phase::Plan);
    }

    #[test]
    fn pdca_advance_through_cycle() {
        let mut state = PdcaState::default();
        state.advance();
        assert_eq!(state.current_phase, Iso42001Phase::Do);
        state.advance();
        assert_eq!(state.current_phase, Iso42001Phase::Check);
        state.advance();
        assert_eq!(state.current_phase, Iso42001Phase::Act);
        state.advance();
        assert_eq!(state.current_phase, Iso42001Phase::Plan);
        assert_eq!(state.cycle_count, 1);
    }

    #[test]
    fn pdca_compliant_check() {
        let sys_state = SystemState::safe(SystemConfig::default());
        let mut pdca = PdcaState::default();
        assert!(pdca.is_compliant(&sys_state));
        pdca.compliance_score = 0.5;
        assert!(!pdca.is_compliant(&sys_state));
    }
}
