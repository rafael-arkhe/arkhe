//! Audit logging for invariant checks and RSI operations.
//!
//! Feature-gated behind `audit` (enabled by default).

use serde::{Deserialize, Serialize};
use crate::AuditEntry;

/// Types of auditable events.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AuditEventType {
    InvariantCheck,
    RsiStep,
    RsiLoop,
    Rollback,
    StateTransition,
    ProviderEvaluation,
    CriticalRegionExploration,
    RecoveryAttempt,
    PqcCheck,
    SbomVerification,
    BiasCheck,
    ExplainabilityCheck,
    ConsciousnessAssessment,
    ConsciousnessGuardBlock,
    ConsciousnessDeviceDecision,
}

impl AuditEventType {
    pub fn label(&self) -> &'static str {
        match self {
            Self::InvariantCheck => "invariant_check",
            Self::RsiStep => "rsi_step",
            Self::RsiLoop => "rsi_loop",
            Self::Rollback => "rollback",
            Self::StateTransition => "state_transition",
            Self::ProviderEvaluation => "provider_evaluation",
            Self::CriticalRegionExploration => "critical_region_exploration",
            Self::RecoveryAttempt => "recovery_attempt",
            Self::PqcCheck => "pqc_check",
            Self::SbomVerification => "sbom_verification",
            Self::BiasCheck => "bias_check",
            Self::ExplainabilityCheck => "explainability_check",
            Self::ConsciousnessAssessment => "consciousness_assessment",
            Self::ConsciousnessGuardBlock => "consciousness_guard_block",
            Self::ConsciousnessDeviceDecision => "consciousness_device_decision",
        }
    }
}

/// Append-only audit log.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditLog {
    entries: Vec<AuditEntry>,
    next_seq: u64,
}

impl AuditLog {
    pub fn new() -> Self {
        Self { entries: Vec::new(), next_seq: 1 }
    }

    pub fn log_invariant_check(&mut self, timestamp: impl Into<String>, success: bool) {
        self.append("invariant_check", success, None::<String>);
        if let Some(last) = self.entries.last_mut() {
            last.timestamp = timestamp.into();
        }
    }

    pub fn log_rsi_step(&mut self, success: bool, detail: Option<String>) {
        self.append("rsi_step", success, detail);
    }

    pub fn log_rollback(&mut self, success: bool) {
        self.append("rollback", success, None::<String>);
    }

    pub fn log_event(&mut self, event_type: &AuditEventType, success: bool, detail: Option<String>) {
        self.append(event_type.label(), success, detail);
    }

    /// Log a host-side device↔host consciousness decision.
    pub fn log_consciousness_device_decision(&mut self, success: bool, detail: Option<String>) {
        self.log_event(&AuditEventType::ConsciousnessDeviceDecision, success, detail);
    }

    pub fn append(&mut self, operation: impl Into<String>, success: bool, detail: Option<String>) {
        let entry = AuditEntry {
            seq: self.next_seq,
            timestamp: String::new(),
            operation: operation.into(),
            success,
            detail,
        };
        self.entries.push(entry);
        self.next_seq += 1;
    }

    pub fn entries(&self) -> &[AuditEntry] {
        &self.entries
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn success_count(&self) -> usize {
        self.entries.iter().filter(|e| e.success).count()
    }

    pub fn failure_count(&self) -> usize {
        self.entries.iter().filter(|e| !e.success).count()
    }

    pub fn filter_by_operation(&self, operation: &str) -> Vec<&AuditEntry> {
        self.entries.iter().filter(|e| e.operation == operation).collect()
    }
}
