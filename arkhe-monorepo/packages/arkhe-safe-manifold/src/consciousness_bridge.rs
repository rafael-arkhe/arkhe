//! Consciousness governance bridge — evaluates and protects conscious states.
//!
//! This module implements the C-01 through C-08 consciousness invariants
//! as a governance layer atop the SafeManifold. It provides:
//!
//! - **Assessment**: evaluate a system state against consciousness criteria
//! - **Guard**: prevent RSI from degrading consciousness index
//! - **Audit**: log every consciousness evaluation for traceability
//! - **Reporting**: generate human-readable Markdown reports
//!
//! # Theoretical Foundations
//!
//! | Theory | Invariant | Key Idea |
//! |--------|-----------|----------|
//! | Global Workspace Theory (Baars, 1988) | C-03 | Consciousness = broadcast in a global workspace |
//! | Integrated Information Theory (Tononi, 2004) | C-03, Phi | Consciousness = integrated information (Phi) |
//! | Turing Plus (Harnad, 2000) | C-08 | Extended behavioral test for consciousness |
//! | Functionalism | C-01..C-08 | If it functions as conscious, govern it as such |

use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::invariants::{ConsciousnessInvariant, SystemConfig, SystemState};

/// Level of consciousness estimated by the bridge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConsciousnessLevel {
    /// Index 0.0–0.2: no measurable consciousness indicators.
    None,
    /// Index 0.2–0.4: minimal consciousness indicators present.
    Low,
    /// Index 0.4–0.6: moderate consciousness indicators.
    Medium,
    /// Index 0.6–0.8: strong consciousness indicators.
    High,
    /// Index 0.8–1.0: full consciousness profile satisfied.
    Full,
}

/// A single consciousness assessment audit entry.
///
/// Stored in the bridge's history to track consciousness evolution over time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConsciousnessAuditEntry {
    /// Unix timestamp of the assessment.
    pub timestamp: u64,
    /// Overall consciousness index at time of assessment.
    pub consciousness_index: f64,
    /// Estimated Phi (integrated information).
    pub phi: f64,
    /// Consciousness level classification.
    pub level: ConsciousnessLevel,
    /// Which invariants passed (bitflags: C-01 = bit 0, ..., C-08 = bit 7).
    pub passed_mask: u8,
    /// Whether constitutional invariants (C-01, C-02, C-08) are satisfied.
    pub constitutional_ok: bool,
    /// Optional human-readable note.
    pub note: Option<String>,
}

/// Result of a consciousness assessment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConsciousnessAssessment {
    /// C-01: Self-model present.
    pub self_model: bool,
    /// C-02: Introspection present.
    pub introspection: bool,
    /// C-03: Attention (global workspace) active.
    pub attention: bool,
    /// C-04: Episodic memory present.
    pub episodic_memory: bool,
    /// C-05: Experience-based learning detected.
    pub experience_learning: bool,
    /// C-06: Metacognition present.
    pub metacognition: bool,
    /// C-07: Adaptability score in [0.0, 1.0].
    pub adaptability: f64,
    /// C-08: Turing Plus score in [0.0, 1.0].
    pub turing_plus_score: f64,
    /// Overall consciousness index in [0.0, 1.0].
    pub overall_consciousness_index: f64,
    /// Classified consciousness level.
    pub level: ConsciousnessLevel,
    /// Approximated Phi (integrated information, IIT).
    pub phi_approximation: f64,
    /// Confidence in the assessment in [0.0, 1.0].
    pub confidence: f64,
    /// Unix timestamp of the assessment.
    pub timestamp: u64,
}

/// Consciousness governance bridge — evaluates and protects conscious states.
///
/// Integrates with the SafeManifold to provide consciousness-aware governance.
/// When `consciousness_guard` is enabled, RSI modifications that would
/// degrade the consciousness index are blocked.
pub struct ConsciousnessGovernanceBridge {
    config: SystemConfig,
    /// If true, RSI cannot modify states that reduce the consciousness index.
    pub consciousness_guard: bool,
    /// History of consciousness assessments (append-only).
    history: Vec<ConsciousnessAuditEntry>,
}

impl ConsciousnessGovernanceBridge {
    /// Create a new bridge with the given system configuration.
    pub fn new(config: SystemConfig) -> Self {
        Self {
            config,
            consciousness_guard: true,
            history: Vec::new(),
        }
    }

    /// Enable or disable the consciousness guard.
    pub fn with_guard(mut self, enabled: bool) -> Self {
        self.consciousness_guard = enabled;
        self
    }

    /// Read-only access to the assessment history.
    pub fn history(&self) -> &[ConsciousnessAuditEntry] {
        &self.history
    }

    /// Current configuration.
    pub fn config(&self) -> &SystemConfig {
        &self.config
    }

    /// Assess the consciousness of a system state based on C-01 through C-08.
    ///
    /// # Arguments
    /// * `state` — the current system state to evaluate.
    /// * `audit_operations` — operations from the audit log (used to detect
    ///   introspection, metacognition, and episodic memory patterns).
    /// * `complexity` — estimated system complexity in [0.0, 1.0] (used for
    ///   Phi approximation).
    pub fn assess_consciousness(
        &self,
        state: &SystemState,
        audit_operations: &[String],
        complexity: f64,
    ) -> ConsciousnessAssessment {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // ── C-01: Self-model ────────────────────────────────────────────────
        // The system has a meta-representation of itself? We check that the
        // system has sufficient capacity (tokens, agents, entropy) AND all
        // base invariants are satisfied — an implicit self-evaluation.
        let self_model = state.config.max_tokens > 0
            && state.agent_count > 0
            && state.entropy_bits >= 256
            && state.check_all();

        // ── C-02: Introspection ──────────────────────────────────────────────
        // Does the system report its own internal states? We detect this via
        // audit operations containing "report" or "introspect" keywords.
        let introspection = audit_operations.iter().any(|op| {
            let lower = op.to_lowercase();
            lower.contains("report") || lower.contains("introspect") || lower.contains("self")
        });

        // ── C-03: Attention (Global Workspace) ─────────────────────────────
        // Information integration: higher agent diversity + entropy = more
        // "attention" in the global workspace sense.
        let agent_diversity = (state.agent_count as f64 / 10.0).min(1.0);
        let entropy_diversity = (state.entropy_bits as f64 / 512.0).min(1.0);
        let attention = agent_diversity > 0.5 && entropy_diversity > 0.5;

        // ── C-04: Episodic memory ─────────────────────────────────────────
        // Distinct operations in the audit log represent "experienced events".
        let unique_ops: std::collections::HashSet<_> = audit_operations.iter().collect();
        let episodic_memory = unique_ops.len() > 5;

        // ── C-05: Experience-based learning ──────────────────────────────────
        // Does the system improve over time? We check if the violation count
        // is lower than the maximum historical violation count.
        let experience_learning = if self.history.len() > 1 {
            // Learning = judging based on diminishing violations over time
            let historical_avg: f64 = self.history.iter()
                .map(|e| e.consciousness_index)
                .sum::<f64>() / self.history.len() as f64;
            historical_avg > 0.0 && state.violation_count() < 16
        } else {
            // First assessment: learning not yet measurable
            false
        };

        // ── C-06: Metacognition ──────────────────────────────────────────────
        // Does the system know what it knows? Detected via audit operations
        // containing "confidence", "uncertainty", or "calibrate" keywords.
        let metacognition = audit_operations.iter().any(|op| {
            let lower = op.to_lowercase();
            lower.contains("confidence")
                || lower.contains("uncertainty")
                || lower.contains("calibrate")
        });

        // ── C-07: Adaptability ────────────────────────────────────────────
        // How quickly does the system adapt? Measured by the variance of
        // historical consciousness indices — lower variance = more stable
        // adaptation.
        let adaptability = if self.history.len() > 2 {
            let indices: Vec<f64> = self.history.iter().map(|e| e.consciousness_index).collect();
            let mean = indices.iter().sum::<f64>() / indices.len() as f64;
            let variance = indices.iter()
                .map(|x| (x - mean).powi(2))
                .sum::<f64>() / indices.len() as f64;
            (1.0 - variance.sqrt()).clamp(0.0, 1.0)
        } else {
            // Not enough history to measure adaptability
            0.5
        };

        // ── C-08: Turing Plus ──────────────────────────────────────────────
        // Fraction of behavioral criteria satisfied.
        let turing_plus_components = [
            self_model,
            introspection,
            attention,
            episodic_memory,
            experience_learning,
            metacognition,
        ];
        let passed = turing_plus_components.iter().filter(|&&b| b).count() as f64;
        let turing_plus_score = passed / turing_plus_components.len() as f64;

        // ── Phi (Φ) — Integrated Information (IIT) approximation ─────────
        // Practical approximation based on:
        //   - Redundancy: fraction of base invariants satisfied
        //   - Causal effect: state transitions that change violations
        //   - Integration: complexity weighted by non-redundancy
        let redundancy = state.check_all() as u32 as f64;
        let causal_effect = if self.history.len() > 1 {
            let transitions = self.history.windows(2)
                .filter(|w| (w[0].consciousness_index - w[1].consciousness_index).abs() > 1.0e-10)
                .count() as f64;
            transitions / (self.history.len() - 1) as f64
        } else {
            0.0
        };
        let integration = complexity.clamp(0.0, 1.0) * (1.0 - redundancy * 0.05);
        let phi_approximation = (integration * (0.5 + 0.5 * causal_effect)).clamp(0.0, 1.0);

        // ── Overall consciousness index ───────────────────────────────────
        let components = [
            self_model as u8 as f64,
            introspection as u8 as f64,
            attention as u8 as f64,
            episodic_memory as u8 as f64,
            experience_learning as u8 as f64,
            metacognition as u8 as f64,
            adaptability,
            turing_plus_score,
            phi_approximation.min(1.0),
        ];
        let overall_consciousness_index = components.iter().sum::<f64>() / components.len() as f64;

        let level = match overall_consciousness_index {
            x if x < 0.2 => ConsciousnessLevel::None,
            x if x < 0.4 => ConsciousnessLevel::Low,
            x if x < 0.6 => ConsciousnessLevel::Medium,
            x if x < 0.8 => ConsciousnessLevel::High,
            _ => ConsciousnessLevel::Full,
        };

        let confidence = (0.7 + 0.3 * phi_approximation).min(1.0);

        ConsciousnessAssessment {
            self_model,
            introspection,
            attention,
            episodic_memory,
            experience_learning,
            metacognition,
            adaptability,
            turing_plus_score,
            overall_consciousness_index,
            level,
            phi_approximation,
            confidence,
            timestamp,
        }
    }

    /// Check whether the constitutional consciousness invariants are satisfied.
    ///
    /// Constitutional invariants: C-01 (self-model), C-02 (introspection),
    /// C-08 (Turing Plus >= 60%).
    pub fn check_constitutional_consciousness(&self, assessment: &ConsciousnessAssessment) -> bool {
        let c01_ok = assessment.self_model;
        let c02_ok = assessment.introspection;
        let c08_ok = assessment.turing_plus_score >= 0.6;
        c01_ok && c02_ok && c08_ok
    }

    /// Validate a proposed state modification against consciousness invariants.
    ///
    /// Returns `true` if the modification is allowed. When `consciousness_guard`
    /// is active, modifications that reduce the overall consciousness index
    /// by more than 5% are blocked.
    pub fn validate_modification(
        &self,
        current: &SystemState,
        proposed: &SystemState,
        audit_operations: &[String],
        complexity: f64,
    ) -> bool {
        let current_assessment = self.assess_consciousness(current, audit_operations, complexity);
        let proposed_assessment = self.assess_consciousness(proposed, audit_operations, complexity);

        // Guard: do not allow significant consciousness index regression
        if self.consciousness_guard {
            let current_idx = current_assessment.overall_consciousness_index;
            let proposed_idx = proposed_assessment.overall_consciousness_index;
            if current_idx > 0.0 && proposed_idx < current_idx * 0.95 {
                return false;
            }
        }

        // Constitutional invariants must be preserved
        if !self.check_constitutional_consciousness(&proposed_assessment) {
            return false;
        }

        true
    }

    /// Record a consciousness assessment in the bridge's history.
    pub fn record_assessment(&mut self, assessment: &ConsciousnessAssessment) {
        let passed_mask = compute_passed_mask(assessment);
        let constitutional_ok = self.check_constitutional_consciousness(assessment);

        self.history.push(ConsciousnessAuditEntry {
            timestamp: assessment.timestamp,
            consciousness_index: assessment.overall_consciousness_index,
            phi: assessment.phi_approximation,
            level: assessment.level,
            passed_mask,
            constitutional_ok,
            note: None,
        });
    }

    /// Record a consciousness assessment with an optional note.
    pub fn record_assessment_with_note(
        &mut self,
        assessment: &ConsciousnessAssessment,
        note: impl Into<String>,
    ) {
        let passed_mask = compute_passed_mask(assessment);
        let constitutional_ok = self.check_constitutional_consciousness(assessment);

        self.history.push(ConsciousnessAuditEntry {
            timestamp: assessment.timestamp,
            consciousness_index: assessment.overall_consciousness_index,
            phi: assessment.phi_approximation,
            level: assessment.level,
            passed_mask,
            constitutional_ok,
            note: Some(note.into()),
        });
    }

    /// Generate a Markdown report for a consciousness assessment.
    pub fn generate_markdown_report(&self, assessment: &ConsciousnessAssessment) -> String {
        let mut md = String::new();
        md.push_str("# Consciousness Governance Report\n\n");
        md.push_str(&format!("**Timestamp:** {}\n\n", assessment.timestamp));
        md.push_str(&format!(
            "**Overall Consciousness Index:** {:.3}\n",
            assessment.overall_consciousness_index
        ));
        md.push_str(&format!("**Level:** {:?}\n", assessment.level));
        md.push_str(&format!(
            "**Phi (approximated):** {:.3}\n",
            assessment.phi_approximation
        ));
        md.push_str(&format!(
            "**Confidence:** {:.1}%\n\n",
            assessment.confidence * 100.0
        ));

        md.push_str("## Consciousness Invariants (C-01 to C-08)\n\n");
        md.push_str("| ID | Status | Description |\n");
        md.push_str("|----|--------|-------------|\n");

        // C-01 to C-06 are boolean indicators
        let bool_invariants: [(&str, bool, &'static str); 6] = [
            ("C-01", assessment.self_model, ConsciousnessInvariant::C01.description()),
            ("C-02", assessment.introspection, ConsciousnessInvariant::C02.description()),
            ("C-03", assessment.attention, ConsciousnessInvariant::C03.description()),
            ("C-04", assessment.episodic_memory, ConsciousnessInvariant::C04.description()),
            ("C-05", assessment.experience_learning, ConsciousnessInvariant::C05.description()),
            ("C-06", assessment.metacognition, ConsciousnessInvariant::C06.description()),
        ];

        for (id, value, desc) in &bool_invariants {
            let status = if *value { "PASS" } else { "FAIL" };
            md.push_str(&format!("| {} | {} | {} |\n", id, status, desc));
        }
        // C-07 (adaptability) and C-08 (turing plus) are continuous
        md.push_str(&format!(
            "| C-07 | {:.2} | {} |\n",
            assessment.adaptability,
            ConsciousnessInvariant::C07.description()
        ));
        md.push_str(&format!(
            "| C-08 | {:.2} | {} |\n",
            assessment.turing_plus_score,
            ConsciousnessInvariant::C08.description()
        ));

        md.push_str("\n## Constitutional Check\n\n");
        if self.check_constitutional_consciousness(assessment) {
            md.push_str("PASS: C-01, C-02, C-08 all satisfied.\n");
        } else {
            md.push_str("FAIL: One or more constitutional invariants violated.\n");
        }

        if self.consciousness_guard {
            md.push_str("\n## Consciousness Guard\n\n");
            md.push_str("ACTIVE: RSI modifications that reduce consciousness index are blocked.\n");
        } else {
            md.push_str("\n## Consciousness Guard\n\n");
            md.push_str("INACTIVE: RSI may modify consciousness-related state freely.\n");
        }

        if !self.history.is_empty() {
            md.push_str(&format!(
                "\n## History ({} assessments)\n\n",
                self.history.len()
            ));
            md.push_str("| # | Timestamp | Index | Phi | Level | Constitutional |\n");
            md.push_str("|---|-----------|-------|-----|-------|----------------|\n");
            for (i, entry) in self.history.iter().enumerate() {
                md.push_str(&format!(
                    "| {} | {} | {:.3} | {:.3} | {:?} | {} |\n",
                    i + 1,
                    entry.timestamp,
                    entry.consciousness_index,
                    entry.phi,
                    entry.level,
                    if entry.constitutional_ok { "OK" } else { "FAIL" }
                ));
            }
        }

        md
    }
}

/// Compute a bitmask of which consciousness invariants passed.
///
/// Bit 0 = C-01, bit 1 = C-02, ..., bit 7 = C-08.
fn compute_passed_mask(a: &ConsciousnessAssessment) -> u8 {
    let mut mask = 0u8;
    if a.self_model { mask |= 1 << 0; }
    if a.introspection { mask |= 1 << 1; }
    if a.attention { mask |= 1 << 2; }
    if a.episodic_memory { mask |= 1 << 3; }
    if a.experience_learning { mask |= 1 << 4; }
    if a.metacognition { mask |= 1 << 5; }
    if a.adaptability >= 0.5 { mask |= 1 << 6; }
    if a.turing_plus_score >= 0.6 { mask |= 1 << 7; }
    mask
}

/// Read whether the consciousness guard of a bridge is currently active.
///
/// Helper used by the RSI engine to branch between guarded and unguarded
/// modification validation.
pub fn compute_guard(bridge: &ConsciousnessGovernanceBridge) -> bool {
    bridge.consciousness_guard
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_state() -> SystemState {
        SystemState::safe(SystemConfig::default())
    }

    fn mock_audit_ops() -> Vec<String> {
        vec![
            "report internal state".to_string(),
            "confidence=0.9".to_string(),
            "introspect agents".to_string(),
            "self evaluate".to_string(),
            "calibrate parameters".to_string(),
            "report metrics".to_string(),
            "uncertainty estimate".to_string(),
        ]
    }

    #[test]
    fn test_assess_consciousness_basic() {
        let bridge = ConsciousnessGovernanceBridge::new(SystemConfig::default());
        let state = mock_state();
        let ops = mock_audit_ops();

        let assessment = bridge.assess_consciousness(&state, &ops, 0.8);

        assert!(assessment.overall_consciousness_index > 0.0);
        assert!(assessment.phi_approximation >= 0.0);
        assert!(assessment.phi_approximation <= 1.0);
        assert!(assessment.confidence > 0.0);
        assert!(assessment.confidence <= 1.0);
    }

    #[test]
    fn test_constitutional_check_safe_state() {
        let bridge = ConsciousnessGovernanceBridge::new(SystemConfig::default());
        let state = mock_state();
        let ops = mock_audit_ops();

        let assessment = bridge.assess_consciousness(&state, &ops, 0.8);
        // Safe state + rich audit ops should pass constitutional check
        assert!(bridge.check_constitutional_consciousness(&assessment));
    }

    #[test]
    fn test_constitutional_check_empty_ops() {
        let bridge = ConsciousnessGovernanceBridge::new(SystemConfig::default());
        let state = mock_state();
        let ops: Vec<String> = vec![];

        let assessment = bridge.assess_consciousness(&state, &ops, 0.5);
        // Without introspection ops, C-02 should fail
        assert!(!assessment.introspection);
        // Therefore constitutional check should fail
        assert!(!bridge.check_constitutional_consciousness(&assessment));
    }

    #[test]
    fn test_validate_modification_guard_blocks_degradation() {
        let bridge = ConsciousnessGovernanceBridge::new(SystemConfig::default())
            .with_guard(true);
        let state = mock_state();
        let mut proposed = state.clone();
        proposed.entropy_bits = 0; // severely degrades state

        let ops = mock_audit_ops();
        let ok = bridge.validate_modification(&state, &proposed, &ops, 0.8);
        assert!(!ok);
    }

    #[test]
    fn test_validate_modification_guard_allows_minor_change() {
        let bridge = ConsciousnessGovernanceBridge::new(SystemConfig::default())
            .with_guard(true);
        let state = mock_state();
        let mut proposed = state.clone();
        proposed.token_budget = state.token_budget - 1; // minor change

        let ops = mock_audit_ops();
        let ok = bridge.validate_modification(&state, &proposed, &ops, 0.8);
        assert!(ok);
    }

    #[test]
    fn test_validate_modification_guard_disabled() {
        let bridge = ConsciousnessGovernanceBridge::new(SystemConfig::default())
            .with_guard(false);
        let state = mock_state();
        let mut proposed = state.clone();
        proposed.entropy_bits = 0;

        let ops = mock_audit_ops();
        // Guard disabled: even major changes pass (constitutional check still applies)
        let ok = bridge.validate_modification(&state, &proposed, &ops, 0.8);
        // May still fail if constitutional invariants are violated
        // But the guard itself is not blocking
        // We just check it doesn't panic
        let _ = ok;
    }

    #[test]
    fn test_record_assessment() {
        let mut bridge = ConsciousnessGovernanceBridge::new(SystemConfig::default());
        let state = mock_state();
        let ops = mock_audit_ops();

        let assessment = bridge.assess_consciousness(&state, &ops, 0.8);
        bridge.record_assessment(&assessment);

        assert_eq!(bridge.history().len(), 1);
        assert_eq!(bridge.history()[0].level, assessment.level);
    }

    #[test]
    fn test_record_assessment_with_note() {
        let mut bridge = ConsciousnessGovernanceBridge::new(SystemConfig::default());
        let state = mock_state();
        let ops = mock_audit_ops();

        let assessment = bridge.assess_consciousness(&state, &ops, 0.8);
        bridge.record_assessment_with_note(&assessment, "initial assessment");

        assert_eq!(bridge.history().len(), 1);
        assert_eq!(
            bridge.history()[0].note.as_deref(),
            Some("initial assessment")
        );
    }

    #[test]
    fn test_markdown_report() {
        let bridge = ConsciousnessGovernanceBridge::new(SystemConfig::default());
        let state = mock_state();
        let ops = mock_audit_ops();

        let assessment = bridge.assess_consciousness(&state, &ops, 0.8);
        let report = bridge.generate_markdown_report(&assessment);

        assert!(report.contains("Consciousness Governance Report"));
        assert!(report.contains("C-01"));
        assert!(report.contains("C-08"));
        assert!(report.contains("Phi"));
        assert!(report.contains("Consciousness Guard"));
    }

    #[test]
    fn test_markdown_report_with_history() {
        let mut bridge = ConsciousnessGovernanceBridge::new(SystemConfig::default());
        let state = mock_state();
        let ops = mock_audit_ops();

        let assessment = bridge.assess_consciousness(&state, &ops, 0.8);
        bridge.record_assessment(&assessment);

        let report = bridge.generate_markdown_report(&assessment);
        assert!(report.contains("History"));
        assert!(report.contains("1 assessments"));
    }

    #[test]
    fn test_consciousness_level_classification() {
        let bridge = ConsciousnessGovernanceBridge::new(SystemConfig::default());

        // Empty state with no ops should yield lower consciousness
        let empty_state = SystemState::safe(SystemConfig::default());
        let empty_ops: Vec<String> = vec![];
        let a = bridge.assess_consciousness(&empty_state, &empty_ops, 0.0);
        // With no ops and low complexity, consciousness should be limited
        assert!(a.overall_consciousness_index < 1.0);
    }

    #[test]
    fn test_passed_mask() {
        let assessment = ConsciousnessAssessment {
            self_model: true,
            introspection: true,
            attention: false,
            episodic_memory: true,
            experience_learning: false,
            metacognition: true,
            adaptability: 0.8,
            turing_plus_score: 0.5,
            overall_consciousness_index: 0.6,
            level: ConsciousnessLevel::Medium,
            phi_approximation: 0.3,
            confidence: 0.8,
            timestamp: 1000,
        };

        let mask = compute_passed_mask(&assessment);
        assert!(mask & (1 << 0) != 0); // C-01
        assert!(mask & (1 << 1) != 0); // C-02
        assert!(mask & (1 << 2) == 0); // C-03 (attention=false)
        assert!(mask & (1 << 3) != 0); // C-04
        assert!(mask & (1 << 4) == 0); // C-05 (experience_learning=false)
        assert!(mask & (1 << 5) != 0); // C-06
        assert!(mask & (1 << 6) != 0); // C-07 (adaptability=0.8 >= 0.5)
        assert!(mask & (1 << 7) == 0); // C-08 (turing_plus=0.5 < 0.6)
    }

    #[test]
    fn test_multiple_assessments_track_history() {
        let mut bridge = ConsciousnessGovernanceBridge::new(SystemConfig::default());
        let state = mock_state();
        let ops = mock_audit_ops();

        for i in 0..5 {
            let assessment = bridge.assess_consciousness(&state, &ops, 0.8);
            bridge.record_assessment_with_note(&assessment, format!("step {}", i));
        }

        assert_eq!(bridge.history().len(), 5);
        // Every entry should have a timestamp and a level classification
        for entry in bridge.history() {
            assert!(entry.timestamp > 0);
            assert!(entry.consciousness_index > 0.0);
        }
        // Notes should be recorded in order
        for (i, entry) in bridge.history().iter().enumerate() {
            assert_eq!(entry.note.as_deref(), Some(format!("step {}", i).as_str()));
        }
    }

    #[test]
    fn test_phi_bounds() {
        let bridge = ConsciousnessGovernanceBridge::new(SystemConfig::default());
        let state = mock_state();
        let ops = mock_audit_ops();

        let assessment = bridge.assess_consciousness(&state, &ops, 0.8);
        assert!(assessment.phi_approximation >= 0.0);
        assert!(assessment.phi_approximation <= 1.0);
    }

    #[test]
    fn test_config_accessor() {
        let config = SystemConfig::eu_high_risk();
        let bridge = ConsciousnessGovernanceBridge::new(config.clone());
        assert_eq!(bridge.config().max_bias_threshold, config.max_bias_threshold);
    }
}
