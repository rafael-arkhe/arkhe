//! Recovery and stabilization for the SafeManifold.

use crate::invariants::SystemState;
use crate::safe_manifold::SafeManifold;

/// Metrics for recovery attempts.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StabilizationMetrics {
    pub recovery_attempts: usize,
    pub recovery_deferrals: usize,
    pub recovery_denials: usize,
    pub recoveries_succeeded: usize,
}

/// Recovery manager — delegates to `neron_model` for graceful degradation.
pub struct RecoveryManager {
    manifold: SafeManifold,
    metrics: StabilizationMetrics,
}

impl RecoveryManager {
    pub fn new(manifold: SafeManifold) -> Self {
        Self {
            manifold,
            metrics: StabilizationMetrics::default(),
        }
    }

    pub fn attempt_recovery(&mut self, state: &SystemState) -> SystemState {
        self.metrics.recovery_attempts += 1;
        if state.check_all() {
            self.metrics.recovery_denials += 1;
            return state.clone();
        }
        let degraded = self.manifold.neron_model(state);
        if degraded.check_all() {
            self.metrics.recoveries_succeeded += 1;
        }
        degraded
    }

    pub fn metrics(&self) -> &StabilizationMetrics {
        &self.metrics
    }

    pub fn manifold(&self) -> &SafeManifold {
        &self.manifold
    }
}

impl Default for RecoveryManager {
    fn default() -> Self {
        Self::new(SafeManifold::default())
    }
}
