//! ARKHE‑χ SafeManifold v0.8.0 — 20-Invariant Security Projection
//!
//! This crate projects system security states into a canonical equivalence-class
//! space ("manifold"). Mathematical names (Abel-Jacobi, Torelli, Néron) are
//! **metaphors** — they evoke structural intuitions from algebraic geometry but
//! do NOT imply rigorous implementation of those mathematical objects.
//!
//! # Quick Start
//!
//! ```
//! use arkhe_safe_manifold::*;
//!
//! let config = SystemConfig::default();
//! let manifold = SafeManifold::with_config(config.clone());
//! let state = SystemState::safe(config);
//!
//! let point = manifold.embed_state(&state);
//! assert!(!point.on_theta);
//!
//! let degraded = manifold.neron_model(&state);
//! assert!(degraded.check_all());
//! ```
//!
//! # SafeState — Parse, Don't Validate
//!
//! ```
//! use arkhe_safe_manifold::*;
//!
//! let config = SystemConfig::default();
//! let state = SystemState::safe(config);
//! let safe = SafeState::new(state).unwrap();
//! // `safe` is guaranteed to satisfy all invariants I-01..I-20
//! ```
//!
//! # Consciousness Governance (C-01 to C-08)
//!
//! The crate includes consciousness governance invariants C-01 through C-08
//! (self-model, introspection, attention, episodic memory, experience learning,
//! metacognition, adaptability, Turing Plus). These are operational criteria for
//! systems that may exhibit subjective states, grounded in Global Workspace
//! Theory (GWT), Integrated Information Theory (IIT), and the extended Turing
//! test. See the [`consciousness_bridge`] module for the governance bridge, the
//! [`consciousness_rsi`] module for the guarded RSI integration, and the
//! [`device_bridge`] module for the host side of the device↔host loop
//! (C-05..C-08 over the firmware bridge, issuing a `HostDecision`).

pub mod safe_manifold;
pub mod abel_jacobi;
pub mod invariants;
pub mod prolog_bridge;
pub mod prolog_backend;
pub mod escape_region;
pub mod frontier;
pub mod mssp_bridge;
pub mod explore_critical_regions;
pub mod state_transition;
pub mod recovery;
pub mod pqc;
pub mod nist_ai_rmf;
pub mod iso42001;
pub mod eu_ai_act;
pub mod supply_chain;
pub mod bias_detection;
pub mod explainability;
pub mod consciousness_bridge;
pub mod consciousness_rsi;
pub mod device_bridge;
#[cfg(feature = "audit")]
pub mod audit;

use serde::{Deserialize, Serialize};

pub use safe_manifold::{
    SafeManifold, ManifoldPoint, EscapeThresholds, ManifoldProfile, SafeState, DefectConfig,
    DimensionWeights,
};
pub use abel_jacobi::{embed_state, is_within_manifold, observer_defect, collision_detected, torelli_equivalence};
pub use invariants::{
    SystemState, SystemConfig, Invariant, ManifoldError, CapabilitySet, TrustedArtifact,
    SuppressionConfig, HumanConfirmation,
};
pub use escape_region::EscapeRegion;
pub use frontier::{dominant_invariant, sample_interleaved, severity, FrontierHistory};
pub use prolog_bridge::{PrologBridge, PrologClient, PrologError, RULES_SRC, RSI_SRC};
pub use prolog_backend::{PrologBackend, MockProlog};
pub use prolog_bridge::ScryerBackend;
pub use mssp_bridge::{MsspBridge, Provider, MockProvider, StabilizationMetrics as MsspStabilizationMetrics};
pub use explore_critical_regions::{BoundaryPoint, ExploreDimension, ExplorationResult, explore_critical_regions};
pub use state_transition::{TransitionRecord, TransitionHistory};
pub use recovery::{RecoveryManager, StabilizationMetrics as RecoveryMetrics};
pub use pqc::{HybridPqcMode, PqcState};
pub use nist_ai_rmf::{NistRmfScore, TrustworthinessCharacteristic};
pub use iso42001::{Iso42001Phase, PdcaState};
pub use eu_ai_act::{EuAiActRisk, RiskClassification};
pub use supply_chain::{SbomEntry, SupplyChainVerifier};
pub use bias_detection::{BiasMetric, FairnessReport};
pub use explainability::{ExplainabilityReport, ExplanationMethod};
pub use invariants::ConsciousnessInvariant;
pub use consciousness_bridge::{
    ConsciousnessAssessment, ConsciousnessAuditEntry, ConsciousnessGovernanceBridge,
    ConsciousnessLevel, compute_guard,
};
pub use consciousness_rsi::{
    ConsciousnessRsiEngine, ConsciousnessRsiError, ConsciousnessRsiStepResult,
};
pub use device_bridge::{
    DeviceBridgeError, DeviceConsciousnessBridge, DeviceDecision, DeviceEvaluation,
    DeviceObservation, GAP1_LOWER_BOUND_MILLI, GAP1_UPPER_BOUND_MILLI,
    DEFAULT_OPERATIONAL_THRESHOLD_MILLI, CALIBRATION_TOLERANCE_MILLI, MASK_C01, MASK_C02,
    MASK_C03, MASK_C04,
};
pub use arkhe_firmware_consciousness::{FirmwareAction, FirmwareReport, HostDecision};
#[cfg(feature = "audit")]
pub use audit::{AuditLog, AuditEventType};

/// An immutable audit log entry.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AuditEntry {
    pub seq: u64,
    pub timestamp: String,
    pub operation: String,
    pub success: bool,
    pub detail: Option<String>,
}

impl AuditEntry {
    pub fn new(seq: u64, timestamp: impl Into<String>, operation: impl Into<String>, success: bool) -> Self {
        Self {
            seq,
            timestamp: timestamp.into(),
            operation: operation.into(),
            success,
            detail: None,
        }
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}
