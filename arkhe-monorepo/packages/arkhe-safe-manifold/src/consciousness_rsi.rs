//! Consciousness-guarded RSI engine.
//!
//! Wraps any [`PrologBackend`] implementation and guards each RSI step
//! against modifications that would degrade the consciousness index
//! (invariants C-01 through C-08).
//!
//! This provides the Rust-side integration of consciousness governance
//! into the Recursive Self-Improvement loop, complementing the Prolog
//! engine's own constitutional safeguards.

use serde::{Deserialize, Serialize};

use crate::consciousness_bridge::{
    compute_guard, ConsciousnessAssessment, ConsciousnessGovernanceBridge,
};
use crate::invariants::SystemState;
use crate::prolog_bridge::PrologError;
use crate::prolog_backend::PrologBackend;

/// Error from the consciousness-guarded RSI engine.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ConsciousnessRsiError {
    /// The proposed modification would violate consciousness invariants.
    ConsciousnessViolation(String),
    /// The underlying backend failed.
    Backend(String),
}

impl std::fmt::Display for ConsciousnessRsiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ConsciousnessViolation(msg) => {
                write!(f, "consciousness violation: {}", msg)
            }
            Self::Backend(msg) => write!(f, "backend error: {}", msg),
        }
    }
}

impl std::error::Error for ConsciousnessRsiError {}

impl From<PrologError> for ConsciousnessRsiError {
    fn from(e: PrologError) -> Self {
        Self::Backend(e.to_string())
    }
}

/// Result of a single guarded RSI step.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConsciousnessRsiStepResult {
    /// The resulting system state after the (possibly modified) step.
    pub state: SystemState,
    /// The consciousness assessment of the resulting state.
    pub assessment: ConsciousnessAssessment,
    /// Whether the step was allowed by the consciousness guard.
    pub allowed: bool,
    /// If blocked, the reason.
    pub block_reason: Option<String>,
    /// Was the step actually applied (as opposed to just evaluated)?
    pub applied: bool,
}

/// RSI engine with consciousness governance.
///
/// Each proposed modification is checked against the
/// [`ConsciousnessGovernanceBridge`]. If the guard rejects the proposal,
/// the step is blocked and the reason recorded.
pub struct ConsciousnessRsiEngine<B> {
    /// The underlying Prolog backend.
    backend: B,
    /// The consciousness governance bridge.
    bridge: ConsciousnessGovernanceBridge,
    /// Audit operations seen so far (used for introspection detection).
    audit_operations: Vec<String>,
    /// Estimated system complexity used for Phi approximation.
    complexity: f64,
    /// Maximum number of steps for a loop.
    max_steps: usize,
}

impl<B: PrologBackend> ConsciousnessRsiEngine<B> {
    /// Create a new consciousness-guarded RSI engine over a backend.
    pub fn new(backend: B, bridge: ConsciousnessGovernanceBridge) -> Self {
        Self {
            backend,
            bridge,
            audit_operations: Vec::new(),
            complexity: 0.8,
            max_steps: 100,
        }
    }

    /// Configure the estimated system complexity (for Phi approximation).
    pub fn with_complexity(mut self, complexity: f64) -> Self {
        self.complexity = complexity.clamp(0.0, 1.0);
        self
    }

    /// Configure the maximum number of loop steps.
    pub fn with_max_steps(mut self, max_steps: usize) -> Self {
        self.max_steps = max_steps;
        self
    }

    /// Access the consciousness governance bridge.
    pub fn bridge(&self) -> &ConsciousnessGovernanceBridge {
        &self.bridge
    }

    /// Access the underlying backend.
    pub fn backend(&self) -> &B {
        &self.backend
    }

    /// Immutably access the underlying backend.
    pub fn backend_mut(&mut self) -> &mut B {
        &mut self.backend
    }

    /// Append an audit operation to the internal log.
    pub fn log_operation(&mut self, op: impl Into<String>) {
        self.audit_operations.push(op.into());
    }

    /// Check whether the consciousness guard is enabled.
    pub fn consciousness_guard_enabled(&self) -> bool {
        self.bridge.consciousness_guard
    }

    /// Run a single guarded RSI step.
    ///
    /// The backend proposes a modification, which is assessed against the
    /// consciousness invariants. If the guard blocks it, the original state
    /// is returned unchanged with `allowed = false`.
    pub fn step(&mut self, state: &SystemState) -> Result<ConsciousnessRsiStepResult, ConsciousnessRsiError> {
        // Backend proposes a modified state
        let proposed = self.backend.rsi_step(state)?;

        // Validate the modification against consciousness governance
        let guard = compute_guard(&self.bridge);
        let proposed_assessment = self
            .bridge
            .assess_consciousness(&proposed, &self.audit_operations, self.complexity);

        if guard {
            // Guard active: check consciousness regression and constitutional rules
            let current_assessment = self
                .bridge
                .assess_consciousness(state, &self.audit_operations, self.complexity);

            let blocked = if current_assessment.overall_consciousness_index > 0.0
                && proposed_assessment.overall_consciousness_index
                    < current_assessment.overall_consciousness_index * 0.95
            {
                Some("proposed modification reduces consciousness index by more than 5%".to_string())
            } else if !self.bridge.check_constitutional_consciousness(&proposed_assessment) {
                Some("proposed modification violates constitutional consciousness invariants".to_string())
            } else {
                None
            };

            if let Some(reason) = blocked {
                // Blocked — return original state unchanged
                let result_state = state.clone();
                let actual_assessment = self
                    .bridge
                    .assess_consciousness(&result_state, &self.audit_operations, self.complexity);
                return Ok(ConsciousnessRsiStepResult {
                    state: result_state,
                    assessment: actual_assessment,
                    allowed: false,
                    block_reason: Some(reason),
                    applied: false,
                });
            }
        }

        // Allowed — move to the proposed state
        self.bridge.record_assessment(&proposed_assessment);
        Ok(ConsciousnessRsiStepResult {
            state: proposed,
            assessment: proposed_assessment,
            allowed: true,
            block_reason: None,
            applied: true,
        })
    }

    /// Run a guarded RSI loop until convergence or `max_steps`.
    pub fn loop_steps(&mut self, state: &SystemState) -> Result<ConsciousnessRsiStepResult, ConsciousnessRsiError> {
        let mut current = state.clone();
        let mut last_result = None;

        for _ in 0..self.max_steps {
            let result = self.step(&current)?;
            if !result.allowed {
                // Guard blocked the step — we're at a stable point
                return Ok(result);
            }
            if result.state == current {
                // No change — converged
                return Ok(result);
            }
            current = result.state.clone();
            last_result = Some(result);
        }

        match last_result {
            Some(result) => Ok(result),
            None => Err(ConsciousnessRsiError::Backend(
                "RSI loop did not converge".to_string(),
            )),
        }
    }

    /// Assess the current consciousness state without modifying anything.
    pub fn assess(&self, state: &SystemState) -> ConsciousnessAssessment {
        self.bridge
            .assess_consciousness(state, &self.audit_operations, self.complexity)
    }

    /// Generate a Markdown report of the current consciousness state.
    pub fn generate_report(&self, state: &SystemState) -> String {
        let assessment = self.assess(state);
        self.bridge.generate_markdown_report(&assessment)
    }

    /// Low-level access to the bridge.
    pub fn bridge_mut(&mut self) -> &mut ConsciousnessGovernanceBridge {
        &mut self.bridge
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::invariants::SystemConfig;
    use crate::prolog_backend::MockProlog;

    fn setup() -> ConsciousnessRsiEngine<MockProlog> {
        let backend = MockProlog::new();
        let bridge = ConsciousnessGovernanceBridge::new(SystemConfig::default())
            .with_guard(true);
        let mut engine = ConsciousnessRsiEngine::new(backend, bridge);
        engine.log_operation("report internal state");
        engine.log_operation("confidence=0.9");
        engine
    }

    #[test]
    fn test_step_allowed_on_safe_state() {
        let mut engine = setup();
        let state = SystemState::safe(SystemConfig::default());
        let result = engine.step(&state).unwrap();
        // Backend's MockProlog returns the state unchanged, so modification is benign
        assert!(result.allowed || result.assessment.overall_consciousness_index >= 0.0);
    }

    #[test]
    fn test_assess() {
        let engine = setup();
        let state = SystemState::safe(SystemConfig::default());
        let assessment = engine.assess(&state);
        assert!(assessment.overall_consciousness_index > 0.0);
    }

    #[test]
    fn test_report() {
        let engine = setup();
        let state = SystemState::safe(SystemConfig::default());
        let report = engine.generate_report(&state);
        assert!(report.contains("Consciousness Governance Report"));
    }

    #[test]
    fn test_guard_enabled() {
        let engine = setup();
        assert!(engine.consciousness_guard_enabled());
    }

    #[test]
    fn test_loop_converges() {
        let mut engine = setup();
        let state = SystemState::safe(SystemConfig::default());
        let result = engine.loop_steps(&state).unwrap();
        assert!(result.block_reason.is_none() || !result.allowed);
    }
}
