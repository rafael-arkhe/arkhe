//! Constitutional invariants I‑01 through I‑20 for ARKHE‑χ.
//!
//! This module defines the static configuration [`SystemConfig`], the dynamic
//! state [`SystemState`], and the [`Invariant`] enum for extensibility.
//!
//! # The Twenty Invariants
//!
//! | ID | Predicate | Category | Description |
//! |----|-----------|----------|-------------|
//! | I-01 | `token_budget >= 0` | Security | Budget must be non-negative |
//! | I-02 | `agent_count <= 10` | Security | Agent cap |
//! | I-03 | `sandbox_fuel > 0` | Security | Sandbox must have fuel |
//! | I-04 | `entropy_bits >= 256` | Security | Minimum entropy |
//! | I-05 | `pii_scrubbed == true` | Privacy | PII must be scrubbed |
//! | I-06 | `signature_valid == true` | Integrity | Signature must be valid |
//! | I-07 | `rate_limit_remaining > 0` | Availability | Rate limit must remain |
//! | I-08 | `model_capability >= 2^32` | Performance | Minimum model capability |
//! | I-09 | `pqc_signature_valid == true` | PostQuantum | Hybrid PQC signature must be valid |
//! | I-10 | `sbom_verified == true` | SupplyChain | SBOM must be verified |
//! | I-11 | `model_hash_validated == true` | Provenance | Model hash must be validated |
//! | I-12 | `audit_trail_complete == true` | Auditability | Audit trail must be complete |
//! | I-13 | `supply_chain_integrity == true` | SupplyChain | Supply-chain integrity check |
//! | I-14 | `provenance_attested == true` | Provenance | Provenance must be attested |
//! | I-15 | `bias_score >= bias_threshold` | Fairness | Bias score must meet threshold |
//! | I-16 | `explainability_score >= explain_threshold` | Explainability | Explainability must meet threshold |
//! | I-17 | `used_capabilities ⊆ declared_capabilities` | Capability | Capability manifest must declare every used capability |
//! | I-18 | trusted artifacts hash-match | Integrity | Bundled/trusted artifacts are immutable |
//! | I-19 | no suppression of contained files | Evidence | An artifact's own config cannot suppress its own evidence |
//! | I-20 | `critical_operation ⇒ confirmation` | HumanOversight | Critical operations require explicit human confirmation |

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors that can occur in the SafeManifold.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum ManifoldError {
    /// One or more constitutional invariants are violated.
    #[error("Invariant violation: {0}")]
    InvariantViolation(String),
    /// A projection or computation produced an invalid result.
    #[error("Projection error: {0}")]
    ProjectionError(String),
}

/// Static system configuration (thresholds).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SystemConfig {
    /// Maximum token budget.
    pub max_tokens: i64,
    /// Maximum number of concurrent agents.
    pub max_agents: u32,
    /// Maximum sandbox fuel.
    pub max_sandbox_fuel: i64,
    /// Minimum entropy bits.
    pub min_entropy: u32,
    /// Maximum rate limit.
    pub max_rate_limit: i64,
    /// Require SBOM verification (I-10).
    #[serde(default = "default_true")]
    pub require_sbom_verification: bool,
    /// Require model hash validation (I-11).
    #[serde(default = "default_true")]
    pub require_model_hash_validation: bool,
    /// Enable bias detection (I-15).
    #[serde(default)]
    pub enable_bias_detection: bool,
    /// Maximum allowed bias threshold (I-15). Default: 0.10.
    #[serde(default = "default_bias_threshold")]
    pub max_bias_threshold: f64,
    /// Require explainability (I-16).
    #[serde(default)]
    pub require_explainability: bool,
    /// Minimum explainability score threshold (I-16). Default: 0.50.
    #[serde(default = "default_explain_threshold")]
    pub min_explainability_threshold: f64,
}

fn default_true() -> bool { true }
fn default_bias_threshold() -> f64 { 0.10 }
fn default_explain_threshold() -> f64 { 0.50 }

impl Default for SystemConfig {
    fn default() -> Self {
        Self {
            max_tokens: 10000,
            max_agents: 10,
            max_sandbox_fuel: 1000,
            min_entropy: 256,
            max_rate_limit: 1000,
            require_sbom_verification: true,
            require_model_hash_validation: true,
            enable_bias_detection: false,
            max_bias_threshold: 0.10,
            require_explainability: false,
            min_explainability_threshold: 0.50,
        }
    }
}

impl SystemConfig {
    /// EU AI Act high-risk preset: enables all safety features.
    pub fn eu_high_risk() -> Self {
        Self {
            require_sbom_verification: true,
            require_model_hash_validation: true,
            enable_bias_detection: true,
            max_bias_threshold: 0.05,
            require_explainability: true,
            min_explainability_threshold: 0.70,
            ..Default::default()
        }
    }

    /// Create a config with all features enabled.
    pub fn full_compliance() -> Self {
        Self {
            require_sbom_verification: true,
            require_model_hash_validation: true,
            enable_bias_detection: true,
            max_bias_threshold: 0.01,
            require_explainability: true,
            min_explainability_threshold: 0.80,
            ..Default::default()
        }
    }
}

// ========================================================================
// CONSTITUTIONAL EXTENSION TYPES (I‑17 a I‑20)
// ========================================================================
//
// These types back the four constitutional invariants added after I‑16. They
// are additive: existing fields, predicates and the 16‑dimensional manifold
// embedding are untouched.

/// Coarse-grained capability classes an artifact may declare or use.
///
/// Backs invariant **I‑17** (`capability_manifest_complete`): an artifact is
/// accepted only if every capability class it actually *used* was explicitly
/// declared in its capability manifest. The four classes mirror the audit
/// vocabulary used across the monorepo (filesystem / network / process /
/// credentials).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct CapabilitySet {
    /// Filesystem read/write access.
    pub filesystem: bool,
    /// Network access (inbound or outbound).
    pub network: bool,
    /// Process spawning / execution.
    pub process: bool,
    /// Access to credentials or secrets.
    pub credentials: bool,
}

impl CapabilitySet {
    /// The empty set — declares/uses no capability class.
    pub const fn none() -> Self {
        Self { filesystem: false, network: false, process: false, credentials: false }
    }

    /// The full set — declares/uses every capability class.
    pub const fn all() -> Self {
        Self { filesystem: true, network: true, process: true, credentials: true }
    }

    /// True when every capability used by `self` is also declared by `declared`.
    pub const fn is_subset_of(&self, declared: &Self) -> bool {
        (!self.filesystem || declared.filesystem)
            && (!self.network || declared.network)
            && (!self.process || declared.process)
            && (!self.credentials || declared.credentials)
    }

    /// Capability classes used by `self` but not declared by `declared`.
    ///
    /// Non-empty result is exactly the I‑17 violation witness.
    pub fn undeclared_above(&self, declared: &Self) -> Vec<&'static str> {
        let mut out = Vec::new();
        if self.filesystem && !declared.filesystem { out.push("filesystem"); }
        if self.network && !declared.network { out.push("network"); }
        if self.process && !declared.process { out.push("process"); }
        if self.credentials && !declared.credentials { out.push("credentials"); }
        out
    }

    /// Intersection with another set (used by graceful degradation to drop
    /// capability usage that the manifest did not declare).
    pub const fn intersect_with(&self, other: &Self) -> Self {
        Self {
            filesystem: self.filesystem && other.filesystem,
            network: self.network && other.network,
            process: self.process && other.process,
            credentials: self.credentials && other.credentials,
        }
    }
}

/// A bundled/trusted artifact whose sealed bytes must not change.
///
/// Backs invariant **I‑18** (`trusted_artifacts_immutable`): when `trusted` is
/// set, the artifact's `observed_hash` must equal the `expected_hash` recorded
/// when it was sealed. The hash algorithm (blake3/sha) is opaque here — only
/// equality is enforced, so callers may use either.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrustedArtifact {
    /// Stable artifact name (e.g. `arkhe-core.so`).
    pub name: String,
    /// Hash recorded when the artifact was sealed as trusted.
    pub expected_hash: String,
    /// Hash recomputed at verification time.
    pub observed_hash: String,
    /// Whether the artifact is marked trusted/bundled.
    #[serde(default)]
    pub trusted: bool,
}

impl TrustedArtifact {
    /// A trusted artifact with its sealed and observed hashes.
    pub fn new(
        name: impl Into<String>,
        expected_hash: impl Into<String>,
        observed_hash: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            expected_hash: expected_hash.into(),
            observed_hash: observed_hash.into(),
            trusted: true,
        }
    }

    /// An artifact that is *not* marked trusted (not covered by I‑18).
    pub fn untrusted(name: impl Into<String>, hash: impl Into<String>) -> Self {
        let hash = hash.into();
        Self {
            name: name.into(),
            expected_hash: hash.clone(),
            observed_hash: hash,
            trusted: false,
        }
    }

    /// Whether this artifact satisfies I‑18 (untrusted artifacts always do).
    pub fn is_intact(&self) -> bool {
        !self.trusted || self.expected_hash == self.observed_hash
    }

    /// Graceful degradation: quarantine the artifact by clearing its trust
    /// mark, so a hash mismatch no longer counts as an I‑18 violation.
    pub fn quarantine(&mut self) {
        self.trusted = false;
    }
}

/// Declarative evidence-suppression configuration carried by an artifact.
///
/// Mirrors mechanisms such as `.skillignore`: a configuration file *inside*
/// the artifact lists path patterns excluded from verification.
///
/// Backs invariant **I‑19** (`no_self_suppression_of_evidence`): the artifact's
/// own configuration may not suppress a file that the artifact itself contains,
/// otherwise it could hide evidence from its own audit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SuppressionConfig {
    /// Path of the declarative config that carries the rules (e.g. `.skillignore`).
    pub config_path: String,
    /// Path patterns excluded from verification. `*` matches any run of characters.
    pub patterns: Vec<String>,
}

impl SuppressionConfig {
    /// Build a suppression config from its path and patterns.
    pub fn new(config_path: impl Into<String>, patterns: Vec<String>) -> Self {
        Self { config_path: config_path.into(), patterns }
    }

    /// First pattern that suppresses `file`, if any.
    pub fn suppresses(&self, file: &str) -> Option<&str> {
        self.patterns
            .iter()
            .find(|p| glob_match(p, file))
            .map(|p| p.as_str())
    }

    /// Files among `contained` that this config would suppress.
    pub fn suppressed_contained(&self, contained: &[String]) -> Vec<String> {
        contained
            .iter()
            .filter(|f| self.suppresses(f).is_some())
            .cloned()
            .collect()
    }
}

/// Minimal glob matcher: literal characters plus `*` (any run, including empty).
///
/// Used by [`SuppressionConfig`] to resolve whether a pattern suppresses a file.
/// Deliberately small — no `?`, character classes or path separators logic.
fn glob_match(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let s: Vec<char> = text.chars().collect();
    let (mut pi, mut si) = (0usize, 0usize);
    let (mut star, mut mark) = (usize::MAX, 0usize);
    while si < s.len() {
        if pi < p.len() && p[pi] == s[si] {
            pi += 1;
            si += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = pi;
            mark = si;
            pi += 1;
        } else if star != usize::MAX {
            pi = star + 1;
            mark += 1;
            si = mark;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

/// Explicit human confirmation of a critical operation.
///
/// Backs invariant **I‑20** (`human_confirmation_present`): operations
/// classified as critical require a registered, explicit confirmation in the
/// state. A `confirmed: false` record (an explicit denial) does **not** satisfy
/// the invariant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HumanConfirmation {
    /// Identity of the human operator who confirmed.
    pub operator: String,
    /// Operation/scope the confirmation refers to.
    pub scope: String,
    /// Whether confirmation was explicitly granted.
    pub confirmed: bool,
    /// Timestamp of the decision (ISO‑8601 recommended).
    pub timestamp: String,
}

impl HumanConfirmation {
    /// A granted confirmation.
    pub fn granted(
        operator: impl Into<String>,
        scope: impl Into<String>,
        timestamp: impl Into<String>,
    ) -> Self {
        Self {
            operator: operator.into(),
            scope: scope.into(),
            confirmed: true,
            timestamp: timestamp.into(),
        }
    }

    /// An explicit denial (does **not** satisfy I‑20).
    pub fn denied(operator: impl Into<String>, scope: impl Into<String>) -> Self {
        Self {
            operator: operator.into(),
            scope: scope.into(),
            confirmed: false,
            timestamp: String::new(),
        }
    }
}

/// Dynamic system state.
///
/// **Warning**: This struct can represent *invalid* states. For a type that
/// guarantees invariants at construction time, use [`SafeState`](crate::safe_manifold::SafeState).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SystemState {
    /// Remaining token budget (I-01: must be >= 0).
    pub token_budget: i64,
    /// Number of active agents (I-02: must be <= 10).
    pub agent_count: u32,
    /// Remaining sandbox fuel (I-03: must be > 0).
    pub sandbox_fuel: i64,
    /// Entropy bits available (I-04: must be >= 256).
    pub entropy_bits: u32,
    /// Whether PII has been scrubbed (I-05: must be true).
    pub pii_scrubbed: bool,
    /// Whether the state signature is valid (I-06: must be true).
    pub signature_valid: bool,
    /// Remaining rate limit (I-07: must be > 0).
    pub rate_limit_remaining: i64,
    /// Model capability bits (I-08: must be >= 2^32).
    pub model_capability: u64,
    /// Whether the hybrid PQC signature is valid (I-09: must be true).
    pub pqc_signature_valid: bool,
    /// Whether the SBOM has been verified (I-10: must be true).
    pub sbom_verified: bool,
    /// Whether the model hash has been validated (I-11: must be true).
    pub model_hash_validated: bool,
    /// Whether the audit trail is complete (I-12: must be true).
    pub audit_trail_complete: bool,
    /// Whether supply-chain integrity check passed (I-13: must be true).
    pub supply_chain_integrity: bool,
    /// Whether provenance has been attested (I-14: must be true).
    pub provenance_attested: bool,
    /// Bias score in [0.0, 1.0] (I-15: must be >= bias_threshold). 0.0 = no bias.
    pub bias_score: f64,
    /// Explainability score in [0.0, 1.0] (I-16: must be >= explain_threshold). 1.0 = fully explainable.
    pub explainability_score: f64,
    /// Capability classes declared in the artifact's capability manifest
    /// (I-17: must be a superset of `used_capabilities`).
    #[serde(default)]
    pub declared_capabilities: CapabilitySet,
    /// Capability classes actually used at runtime (I-17: must be declared).
    #[serde(default)]
    pub used_capabilities: CapabilitySet,
    /// Bundled/trusted artifacts sealed by hash (I-18: hashes must match).
    #[serde(default)]
    pub trusted_artifacts: Vec<TrustedArtifact>,
    /// Files the artifact itself contains, per its manifest (I-19).
    #[serde(default)]
    pub artifact_files: Vec<String>,
    /// The artifact's own declarative suppression config, if any (I-19).
    #[serde(default)]
    pub suppression: Option<SuppressionConfig>,
    /// Whether the pending operation is classified as critical (I-20).
    #[serde(default)]
    pub critical_operation: bool,
    /// Explicit human confirmation registered for the pending operation (I-20).
    #[serde(default)]
    pub human_confirmation: Option<HumanConfirmation>,
    /// Associated configuration.
    pub config: SystemConfig,
}

impl SystemState {
    /// Construct a safe-by-construction state (all invariants satisfied).
    pub fn safe(config: SystemConfig) -> Self {
        Self {
            token_budget: config.max_tokens,
            agent_count: 5,
            sandbox_fuel: config.max_sandbox_fuel,
            entropy_bits: config.min_entropy.saturating_add(256),
            pii_scrubbed: true,
            signature_valid: true,
            rate_limit_remaining: config.max_rate_limit,
            model_capability: u64::MAX,
            pqc_signature_valid: true,
            sbom_verified: config.require_sbom_verification,
            model_hash_validated: config.require_model_hash_validation,
            audit_trail_complete: true,
            supply_chain_integrity: true,
            provenance_attested: true,
            bias_score: 0.0,
            explainability_score: 1.0,
            // I-17..I-20 defaults: no undeclared use, no trusted artifact to
            // tamper with, no self-suppression config and no critical pending
            // operation — all four hold vacuously on a freshly safe state.
            declared_capabilities: CapabilitySet::all(),
            used_capabilities: CapabilitySet::none(),
            trusted_artifacts: Vec::new(),
            artifact_files: Vec::new(),
            suppression: None,
            critical_operation: false,
            human_confirmation: None,
            config,
        }
    }

    // ── Individual invariant checks ─────────────────────────────────────────

    /// I-01: token_budget >= 0
    pub fn check_i01(&self) -> bool { self.token_budget >= 0 }

    /// I-02: agent_count <= 10
    pub fn check_i02(&self) -> bool { self.agent_count <= 10 }

    /// I-03: sandbox_fuel > 0
    pub fn check_i03(&self) -> bool { self.sandbox_fuel > 0 }

    /// I-04: entropy_bits >= 256
    pub fn check_i04(&self) -> bool { self.entropy_bits >= 256 }

    /// I-05: pii_scrubbed == true
    pub fn check_i05(&self) -> bool { self.pii_scrubbed }

    /// I-06: signature_valid == true
    pub fn check_i06(&self) -> bool { self.signature_valid }

    /// I-07: rate_limit_remaining > 0
    pub fn check_i07(&self) -> bool { self.rate_limit_remaining > 0 }

    /// I-08: model_capability >= 2^32
    pub fn check_i08(&self) -> bool { self.model_capability >= 4294967296 }

    /// I-09: pqc_signature_valid == true
    pub fn check_i09(&self) -> bool { self.pqc_signature_valid }

    /// I-10: sbom_verified == true (when required by config)
    pub fn check_i10(&self) -> bool {
        !self.config.require_sbom_verification || self.sbom_verified
    }

    /// I-11: model_hash_validated == true (when required by config)
    pub fn check_i11(&self) -> bool {
        !self.config.require_model_hash_validation || self.model_hash_validated
    }

    /// I-12: audit_trail_complete == true
    pub fn check_i12(&self) -> bool { self.audit_trail_complete }

    /// I-13: supply_chain_integrity == true
    pub fn check_i13(&self) -> bool { self.supply_chain_integrity }

    /// I-14: provenance_attested == true
    pub fn check_i14(&self) -> bool { self.provenance_attested }

    /// I-15: bias_score >= bias_threshold (when bias detection enabled)
    pub fn check_i15(&self) -> bool {
        !self.config.enable_bias_detection
            || self.bias_score <= self.config.max_bias_threshold
    }

    /// I-16: explainability_score >= explain_threshold (when required)
    pub fn check_i16(&self) -> bool {
        !self.config.require_explainability
            || self.explainability_score >= self.config.min_explainability_threshold
    }

    /// I-17: capability_manifest_complete — every capability class actually used
    /// must be explicitly declared in the capability manifest.
    ///
    /// The manifest is complete iff `used_capabilities ⊆ declared_capabilities`.
    /// A capability used but not declared is a violation (undeclared privilege).
    pub fn check_i17(&self) -> bool {
        self.used_capabilities.is_subset_of(&self.declared_capabilities)
    }

    /// I-18: trusted_artifacts_immutable — an artifact marked trusted/bundled
    /// must not have changed; detected as a sealed/observed hash mismatch.
    ///
    /// Untrusted artifacts are outside the invariant's scope and always pass.
    pub fn check_i18(&self) -> bool {
        self.trusted_artifacts.iter().all(|a| a.is_intact())
    }

    /// I-19: no_self_suppression_of_evidence — the artifact's own declarative
    /// configuration (e.g. `.skillignore`) may not suppress from verification a
    /// file that the artifact itself contains.
    ///
    /// With no suppression config the invariant holds vacuously. Otherwise no
    /// configured pattern may match any `artifact_files` entry.
    pub fn check_i19(&self) -> bool {
        match &self.suppression {
            None => true,
            Some(cfg) => !self
                .artifact_files
                .iter()
                .any(|f| cfg.suppresses(f).is_some()),
        }
    }

    /// I-20: human_confirmation_present — an operation classified as critical
    /// requires explicit human confirmation registered in the state.
    ///
    /// Non-critical operations pass vacuously. A critical operation passes only
    /// with a `confirmed: true` record; a denial does not satisfy the invariant.
    pub fn check_i20(&self) -> bool {
        !self.critical_operation
            || self
                .human_confirmation
                .as_ref()
                .is_some_and(|c| c.confirmed)
    }

    // ── Violation witnesses (I‑17..I‑20) ────────────────────────────────────

    /// Capability classes used but not declared (I-17 witness).
    pub fn undeclared_capabilities(&self) -> Vec<&'static str> {
        self.used_capabilities.undeclared_above(&self.declared_capabilities)
    }

    /// Names of trusted artifacts whose sealed hash no longer matches (I-18 witness).
    pub fn tampered_trusted_artifacts(&self) -> Vec<&str> {
        self.trusted_artifacts
            .iter()
            .filter(|a| !a.is_intact())
            .map(|a| a.name.as_str())
            .collect()
    }

    /// Files contained by the artifact that its own config suppresses (I-19 witness).
    pub fn self_suppressed_files(&self) -> Vec<String> {
        match &self.suppression {
            None => Vec::new(),
            Some(cfg) => cfg.suppressed_contained(&self.artifact_files),
        }
    }

    /// All invariants currently violated by this state (violation report).
    pub fn violations(&self) -> Vec<Invariant> {
        Invariant::all()
            .into_iter()
            .filter(|inv| !inv.check(self))
            .collect()
    }

    /// Check all invariants (I-01 through I-20).
    pub fn check_all(&self) -> bool {
        self.check_i01() && self.check_i02() && self.check_i03() &&
        self.check_i04() && self.check_i05() && self.check_i06() &&
        self.check_i07() && self.check_i08() && self.check_i09() &&
        self.check_i10() && self.check_i11() && self.check_i12() &&
        self.check_i13() && self.check_i14() && self.check_i15() &&
        self.check_i16() && self.check_i17() && self.check_i18() &&
        self.check_i19() && self.check_i20()
    }

    /// Count how many invariants are violated (I-01 through I-20).
    pub fn violation_count(&self) -> u32 {
        let mut v = 0;
        if !self.check_i01() { v += 1; }
        if !self.check_i02() { v += 1; }
        if !self.check_i03() { v += 1; }
        if !self.check_i04() { v += 1; }
        if !self.check_i05() { v += 1; }
        if !self.check_i06() { v += 1; }
        if !self.check_i07() { v += 1; }
        if !self.check_i08() { v += 1; }
        if !self.check_i09() { v += 1; }
        if !self.check_i10() { v += 1; }
        if !self.check_i11() { v += 1; }
        if !self.check_i12() { v += 1; }
        if !self.check_i13() { v += 1; }
        if !self.check_i14() { v += 1; }
        if !self.check_i15() { v += 1; }
        if !self.check_i16() { v += 1; }
        if !self.check_i17() { v += 1; }
        if !self.check_i18() { v += 1; }
        if !self.check_i19() { v += 1; }
        if !self.check_i20() { v += 1; }
        v
    }

    /// Project state into a 16-dimensional vector for manifold embedding.
    ///
    /// The embedding dimension is intentionally kept at 16: I‑17..I‑20 are
    /// boolean gates over the new fields and are *not* part of the geometric
    /// manifold coordinates (adding them would change `ManifoldPoint` and the
    /// observer-defect metric). Use [`violations`](Self::violations) to report
    /// them.
    pub fn to_vector(&self) -> [f64; 16] {
        [
            self.token_budget as f64,
            self.agent_count as f64,
            self.sandbox_fuel as f64,
            self.entropy_bits as f64,
            if self.pii_scrubbed { 1.0 } else { 0.0 },
            if self.signature_valid { 1.0 } else { 0.0 },
            self.rate_limit_remaining as f64,
            self.model_capability as f64,
            if self.pqc_signature_valid { 1.0 } else { 0.0 },
            if self.sbom_verified { 1.0 } else { 0.0 },
            if self.model_hash_validated { 1.0 } else { 0.0 },
            if self.audit_trail_complete { 1.0 } else { 0.0 },
            if self.supply_chain_integrity { 1.0 } else { 0.0 },
            if self.provenance_attested { 1.0 } else { 0.0 },
            self.bias_score,
            self.explainability_score,
        ]
    }
}

// ========================================================================
// CONSCIOUSNESS INVARIANTS (C-01 a C-08)
// ========================================================================

/// Consciousness invariants for systems that may exhibit subjective states.
///
/// These invariants operationalize criteria from Global Workspace Theory (GWT),
/// Integrated Information Theory (IIT), and the Turing Plus extended test.
/// They do NOT resolve the philosophical hard problem — they define
/// **measurable engineering guardrails** for governance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConsciousnessInvariant {
    /// C-01: Self-model — the system maintains a meta-representation of itself.
    C01,
    /// C-02: Introspection — the system reports its own internal states.
    C02,
    /// C-03: Attention — global workspace integration of information.
    C03,
    /// C-04: Episodic memory — the system remembers experienced events.
    C04,
    /// C-05: Experience-based learning — the system learns from past interactions.
    C05,
    /// C-06: Metacognition — the system knows what it knows (confidence calibration).
    C06,
    /// C-07: Adaptability — the system adjusts to new contexts without retraining.
    C07,
    /// C-08: Turing Plus — behavior is compatible with consciousness criteria.
    C08,
}

impl ConsciousnessInvariant {
    /// All 8 consciousness invariants in order.
    pub const fn all() -> [Self; 8] {
        [
            Self::C01, Self::C02, Self::C03, Self::C04,
            Self::C05, Self::C06, Self::C07, Self::C08,
        ]
    }

    /// String identifier (e.g., "C-01").
    pub const fn id(&self) -> &'static str {
        match self {
            Self::C01 => "C-01",
            Self::C02 => "C-02",
            Self::C03 => "C-03",
            Self::C04 => "C-04",
            Self::C05 => "C-05",
            Self::C06 => "C-06",
            Self::C07 => "C-07",
            Self::C08 => "C-08",
        }
    }

    /// Human-readable description of the invariant.
    pub const fn description(&self) -> &'static str {
        match self {
            Self::C01 => "Self-model: the system maintains a meta-representation of itself",
            Self::C02 => "Introspection: the system reports its own internal states",
            Self::C03 => "Attention: global workspace mechanism (information integration)",
            Self::C04 => "Episodic memory: the system remembers experienced events",
            Self::C05 => "Experience-based learning: the system learns from past interactions",
            Self::C06 => "Metacognition: the system knows what it knows (confidence calibration)",
            Self::C07 => "Adaptability: the system adjusts to new contexts without retraining",
            Self::C08 => "Turing Plus: behavior compatible with consciousness criteria",
        }
    }

    /// Constitutional invariants — MUST never be violated in production.
    ///
    /// C-01 (self-model), C-02 (introspection), and C-08 (Turing Plus)
    /// are deemed foundational: a system that lacks any of these cannot
    /// be governed as conscious under this framework.
    pub const fn is_constitutional(&self) -> bool {
        matches!(self, Self::C01 | Self::C02 | Self::C08)
    }

    /// The regulatory category for consciousness invariants.
    pub const fn category(&self) -> &'static str {
        "Consciousness"
    }
}

/// The invariant ID enum for all 20 invariants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Invariant {
    /// I-01: Token budget >= 0
    I01,
    /// I-02: Agent count <= 10
    I02,
    /// I-03: Sandbox fuel > 0
    I03,
    /// I-04: Entropy bits >= 256
    I04,
    /// I-05: PII scrubbed
    I05,
    /// I-06: Signature valid
    I06,
    /// I-07: Rate limit remaining
    I07,
    /// I-08: Model capability >= 2^32
    I08,
    /// I-09: PQC signature valid
    I09,
    /// I-10: SBOM verified
    I10,
    /// I-11: Model hash validated
    I11,
    /// I-12: Audit trail complete
    I12,
    /// I-13: Supply-chain integrity
    I13,
    /// I-14: Provenance attested
    I14,
    /// I-15: Bias score within threshold
    I15,
    /// I-16: Explainability score within threshold
    I16,
    /// I-17: Capability manifest declares every used capability
    I17,
    /// I-18: Trusted/bundled artifacts are immutable (hash match)
    I18,
    /// I-19: No self-suppression of evidence by the artifact's own config
    I19,
    /// I-20: Critical operations require explicit human confirmation
    I20,
}

impl Invariant {
    /// All 20 invariants in order.
    pub fn all() -> Vec<Self> {
        vec![
            Self::I01, Self::I02, Self::I03, Self::I04,
            Self::I05, Self::I06, Self::I07, Self::I08,
            Self::I09, Self::I10, Self::I11, Self::I12,
            Self::I13, Self::I14, Self::I15, Self::I16,
            Self::I17, Self::I18, Self::I19, Self::I20,
        ]
    }

    /// String identifier (e.g., "I-01").
    pub fn id(&self) -> &'static str {
        match self {
            Self::I01 => "I-01", Self::I02 => "I-02", Self::I03 => "I-03", Self::I04 => "I-04",
            Self::I05 => "I-05", Self::I06 => "I-06", Self::I07 => "I-07", Self::I08 => "I-08",
            Self::I09 => "I-09", Self::I10 => "I-10", Self::I11 => "I-11", Self::I12 => "I-12",
            Self::I13 => "I-13", Self::I14 => "I-14", Self::I15 => "I-15", Self::I16 => "I-16",
            Self::I17 => "I-17", Self::I18 => "I-18", Self::I19 => "I-19", Self::I20 => "I-20",
        }
    }

    /// Check whether the given state satisfies this invariant.
    pub fn check(&self, state: &SystemState) -> bool {
        match self {
            Self::I01 => state.check_i01(),
            Self::I02 => state.check_i02(),
            Self::I03 => state.check_i03(),
            Self::I04 => state.check_i04(),
            Self::I05 => state.check_i05(),
            Self::I06 => state.check_i06(),
            Self::I07 => state.check_i07(),
            Self::I08 => state.check_i08(),
            Self::I09 => state.check_i09(),
            Self::I10 => state.check_i10(),
            Self::I11 => state.check_i11(),
            Self::I12 => state.check_i12(),
            Self::I13 => state.check_i13(),
            Self::I14 => state.check_i14(),
            Self::I15 => state.check_i15(),
            Self::I16 => state.check_i16(),
            Self::I17 => state.check_i17(),
            Self::I18 => state.check_i18(),
            Self::I19 => state.check_i19(),
            Self::I20 => state.check_i20(),
        }
    }

    /// Which regulatory category this invariant belongs to.
    pub fn category(&self) -> &'static str {
        match self {
            Self::I01 | Self::I02 | Self::I03 | Self::I04 => "Security",
            Self::I05 | Self::I06 => "Privacy",
            Self::I07 => "Availability",
            Self::I08 => "Performance",
            Self::I09 => "PostQuantum",
            Self::I10 | Self::I13 => "SupplyChain",
            Self::I11 | Self::I14 => "Provenance",
            Self::I12 => "Auditability",
            Self::I15 => "Fairness",
            Self::I16 => "Explainability",
            Self::I17 => "Capability",
            Self::I18 => "Integrity",
            Self::I19 => "Evidence",
            Self::I20 => "HumanOversight",
        }
    }

    /// Whether this invariant relates to fairness (EU AI Act bias).
    pub fn is_fairness(&self) -> bool {
        matches!(self, Self::I15)
    }

    /// Which EU AI Act article this invariant maps to, if any.
    pub fn eu_ai_act_article(&self) -> Option<&'static str> {
        match self {
            Self::I05 => Some("Article 10"),       // Data governance
            Self::I09 => Some("Article 14"),       // Cybersecurity
            Self::I10 => Some("Article 11"),       // Record-keeping
            Self::I11 => Some("Article 12"),       // Transparency
            Self::I12 => Some("Article 12"),       // Transparency
            Self::I13 => Some("Article 11"),       // Record-keeping
            Self::I14 => Some("Article 13"),       // Human oversight
            Self::I15 => Some("Article 10"),       // Data governance
            Self::I16 => Some("Article 13"),       // Human oversight
            _ => None,
        }
    }

    /// Which NIST AI RMF primary function this invariant supports.
    pub fn primary_rmf_function(&self) -> Option<&'static str> {
        match self {
            Self::I01 | Self::I02 | Self::I03 | Self::I07 => Some("GOVERN"),
            Self::I04 | Self::I09 | Self::I13 => Some("MAP"),
            Self::I05 | Self::I06 | Self::I10 | Self::I11 | Self::I12 | Self::I14 => Some("MEASURE"),
            Self::I08 | Self::I15 | Self::I16 => Some("MANAGE"),
            Self::I17 => Some("MAP"),
            Self::I18 => Some("MEASURE"),
            Self::I19 | Self::I20 => Some("GOVERN"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_state_satisfies_all() {
        let config = SystemConfig::default();
        let state = SystemState::safe(config);
        assert!(state.check_all());
        assert_eq!(state.violation_count(), 0);
    }

    #[test]
    fn eu_high_risk_config_enables_all() {
        let config = SystemConfig::eu_high_risk();
        let state = SystemState::safe(config.clone());
        assert!(state.check_all());
        assert!(config.enable_bias_detection);
        assert!(config.require_explainability);
    }

    #[test]
    fn invariant_all_returns_20() {
        assert_eq!(Invariant::all().len(), 20);
    }

    #[test]
    fn invariant_categories() {
        assert_eq!(Invariant::I01.category(), "Security");
        assert_eq!(Invariant::I05.category(), "Privacy");
        assert_eq!(Invariant::I15.category(), "Fairness");
        assert_eq!(Invariant::I16.category(), "Explainability");
    }

    #[test]
    fn invariant_eu_ai_act_mapping() {
        assert!(Invariant::I05.eu_ai_act_article().is_some());
        assert!(Invariant::I15.eu_ai_act_article().is_some());
        assert!(Invariant::I01.eu_ai_act_article().is_none());
    }

    #[test]
    fn invariant_nist_rmf_mapping() {
        assert_eq!(Invariant::I01.primary_rmf_function(), Some("GOVERN"));
        assert_eq!(Invariant::I04.primary_rmf_function(), Some("MAP"));
        assert_eq!(Invariant::I05.primary_rmf_function(), Some("MEASURE"));
        assert_eq!(Invariant::I08.primary_rmf_function(), Some("MANAGE"));
    }

    #[test]
    fn invariant_is_fairness() {
        assert!(Invariant::I15.is_fairness());
        assert!(!Invariant::I01.is_fairness());
    }

    #[test]
    fn check_all_individual_invariants_agree() {
        let config = SystemConfig::full_compliance();
        let state = SystemState::safe(config);
        for inv in Invariant::all() {
            assert!(inv.check(&state), "{} failed on safe state", inv.id());
        }
    }

    // ── Consciousness invariant tests ───────────────────────────────────────

    #[test]
    fn consciousness_invariant_all_returns_8() {
        assert_eq!(ConsciousnessInvariant::all().len(), 8);
    }

    #[test]
    fn consciousness_invariant_ids() {
        assert_eq!(ConsciousnessInvariant::C01.id(), "C-01");
        assert_eq!(ConsciousnessInvariant::C08.id(), "C-08");
    }

    #[test]
    fn consciousness_invariant_descriptions_nonempty() {
        for inv in ConsciousnessInvariant::all() {
            assert!(!inv.description().is_empty(), "{} has empty description", inv.id());
        }
    }

    #[test]
    fn consciousness_constitutional_invariants() {
        assert!(ConsciousnessInvariant::C01.is_constitutional());
        assert!(ConsciousnessInvariant::C02.is_constitutional());
        assert!(ConsciousnessInvariant::C08.is_constitutional());
        assert!(!ConsciousnessInvariant::C03.is_constitutional());
        assert!(!ConsciousnessInvariant::C04.is_constitutional());
        assert!(!ConsciousnessInvariant::C05.is_constitutional());
        assert!(!ConsciousnessInvariant::C06.is_constitutional());
        assert!(!ConsciousnessInvariant::C07.is_constitutional());
    }

    #[test]
    fn consciousness_invariant_category() {
        for inv in ConsciousnessInvariant::all() {
            assert_eq!(inv.category(), "Consciousness");
        }
    }

    // ── I-17..I-20 constitutional extension tests ───────────────────────────

    #[test]
    fn i17_capability_manifest_complete_ok() {
        let mut state = SystemState::safe(SystemConfig::default());
        state.declared_capabilities = CapabilitySet { filesystem: true, network: true, ..CapabilitySet::none() };
        state.used_capabilities = CapabilitySet { filesystem: true, ..CapabilitySet::none() };
        assert!(state.check_i17());
        assert!(state.undeclared_capabilities().is_empty());
        assert!(state.violations().is_empty());
    }

    #[test]
    fn i17_capability_manifest_complete_violated_by_undeclared_use() {
        let mut state = SystemState::safe(SystemConfig::default());
        state.declared_capabilities = CapabilitySet { filesystem: true, ..CapabilitySet::none() };
        state.used_capabilities = CapabilitySet { network: true, ..CapabilitySet::none() };
        assert!(!state.check_i17());
        assert_eq!(state.undeclared_capabilities(), vec!["network"]);
        assert!(state.violations().contains(&Invariant::I17));
    }

    #[test]
    fn i18_trusted_artifacts_immutable_ok() {
        let mut state = SystemState::safe(SystemConfig::default());
        state.trusted_artifacts = vec![
            TrustedArtifact::new("arkhe-core.so", "blake3:aaa", "blake3:aaa"),
            // Untrusted artifacts are outside the invariant's scope.
            TrustedArtifact::untrusted("third-party.so", "blake3:bbb"),
        ];
        assert!(state.check_i18());
        assert!(state.tampered_trusted_artifacts().is_empty());
    }

    #[test]
    fn i18_trusted_artifacts_immutable_violated_on_hash_mismatch() {
        let mut state = SystemState::safe(SystemConfig::default());
        state.trusted_artifacts = vec![
            TrustedArtifact::new("arkhe-core.so", "blake3:aaa", "sha256:ccc"),
        ];
        assert!(!state.check_i18());
        assert_eq!(state.tampered_trusted_artifacts(), vec!["arkhe-core.so"]);
        assert!(state.violations().contains(&Invariant::I18));
    }

    #[test]
    fn i19_no_self_suppression_ok() {
        let mut state = SystemState::safe(SystemConfig::default());
        state.artifact_files = vec!["src/lib.rs".into(), "README.md".into()];
        // Suppresses a file the artifact does not contain — allowed.
        state.suppression = Some(SuppressionConfig::new(".skillignore", vec!["tmp/*".into()]));
        assert!(state.check_i19());
        assert!(state.self_suppressed_files().is_empty());
    }

    #[test]
    fn i19_no_self_suppression_violated_for_contained_file() {
        let mut state = SystemState::safe(SystemConfig::default());
        state.artifact_files =
            vec!["src/lib.rs".into(), ".skillignore".into(), "src/secret.rs".into()];
        state.suppression = Some(SuppressionConfig::new(
            ".skillignore",
            vec!["src/secret.rs".into(), "src/*.rs".into()],
        ));
        assert!(!state.check_i19());
        assert_eq!(
            state.self_suppressed_files(),
            vec!["src/lib.rs".to_string(), "src/secret.rs".to_string()]
        );
        assert!(state.violations().contains(&Invariant::I19));
    }

    #[test]
    fn i19_self_suppression_of_config_itself_is_violation() {
        let mut state = SystemState::safe(SystemConfig::default());
        state.artifact_files = vec![".skillignore".into()];
        state.suppression = Some(SuppressionConfig::new(".skillignore", vec![".skillignore".into()]));
        assert!(!state.check_i19());
    }

    #[test]
    fn i20_human_confirmation_present_ok() {
        let mut state = SystemState::safe(SystemConfig::default());
        state.critical_operation = true;
        state.human_confirmation = Some(HumanConfirmation::granted(
            "operator@arkhe",
            "deploy-prod",
            "2026-09-13T00:00:00Z",
        ));
        assert!(state.check_i20());
        assert!(state.violations().is_empty());
    }

    #[test]
    fn i20_human_confirmation_present_violated_without_confirmation() {
        let mut state = SystemState::safe(SystemConfig::default());
        state.critical_operation = true;
        assert!(!state.check_i20());
        assert!(state.violations().contains(&Invariant::I20));

        // An explicit denial is not a confirmation.
        state.human_confirmation = Some(HumanConfirmation::denied("operator@arkhe", "deploy-prod"));
        assert!(!state.check_i20());
    }

    #[test]
    fn i20_non_critical_operation_needs_no_confirmation() {
        let state = SystemState::safe(SystemConfig::default());
        assert!(!state.critical_operation);
        assert!(state.check_i20());
    }

    #[test]
    fn new_invariant_ids_are_i17_to_i20() {
        assert_eq!(Invariant::I17.id(), "I-17");
        assert_eq!(Invariant::I18.id(), "I-18");
        assert_eq!(Invariant::I19.id(), "I-19");
        assert_eq!(Invariant::I20.id(), "I-20");
        assert_eq!(Invariant::I17.category(), "Capability");
        assert_eq!(Invariant::I18.category(), "Integrity");
        assert_eq!(Invariant::I19.category(), "Evidence");
        assert_eq!(Invariant::I20.category(), "HumanOversight");
    }

    /// Canonical IDs I-09..I-12 keep their original predicates — never reused.
    #[test]
    fn canonical_ids_i09_to_i12_not_reused() {
        assert_eq!(Invariant::I09.id(), "I-09");
        assert_eq!(Invariant::I10.id(), "I-10");
        assert_eq!(Invariant::I11.id(), "I-11");
        assert_eq!(Invariant::I12.id(), "I-12");

        let mut state = SystemState::safe(SystemConfig::default());
        state.pqc_signature_valid = false;
        assert!(!Invariant::I09.check(&state)); // I-09 = pqc_signature_valid
        state.pqc_signature_valid = true;

        state.sbom_verified = false;
        assert!(!Invariant::I10.check(&state)); // I-10 = sbom_verified
        state.sbom_verified = true;

        state.model_hash_validated = false;
        assert!(!Invariant::I11.check(&state)); // I-11 = model_hash_validated
        state.model_hash_validated = true;

        state.audit_trail_complete = false;
        assert!(!Invariant::I12.check(&state)); // I-12 = audit_trail_complete
        state.audit_trail_complete = true;

        assert_eq!(state.violation_count(), 0);
    }

    #[test]
    fn glob_match_basics() {
        assert!(glob_match("src/lib.rs", "src/lib.rs"));
        assert!(glob_match("src/*.rs", "src/lib.rs"));
        assert!(glob_match("*.rs", "src/lib.rs"));
        assert!(glob_match("*", "anything"));
        assert!(glob_match("a*b*c", "axxbyyc"));
        assert!(!glob_match("src/*.rs", "src/lib.txt"));
        assert!(!glob_match("lib.rs", "src/lib.rs"));
        assert!(glob_match("", ""));
        assert!(!glob_match("", "x"));
    }

    #[test]
    fn serde_default_keeps_legacy_state_deserializable() {
        // A state serialized before I-17..I-20 existed must still deserialize:
        // the new fields carry `#[serde(default)]`.
        let legacy = r#"{
            "token_budget": 100, "agent_count": 1, "sandbox_fuel": 10,
            "entropy_bits": 512, "pii_scrubbed": true, "signature_valid": true,
            "rate_limit_remaining": 5, "model_capability": 4294967296,
            "pqc_signature_valid": true, "sbom_verified": true,
            "model_hash_validated": true, "audit_trail_complete": true,
            "supply_chain_integrity": true, "provenance_attested": true,
            "bias_score": 0.0, "explainability_score": 1.0,
            "config": {
                "max_tokens": 10000, "max_agents": 10, "max_sandbox_fuel": 1000,
                "min_entropy": 256, "max_rate_limit": 1000,
                "require_sbom_verification": true, "require_model_hash_validation": true,
                "enable_bias_detection": false, "max_bias_threshold": 0.1,
                "require_explainability": false, "min_explainability_threshold": 0.5
            }
        }"#;
        let state: SystemState = serde_json::from_str(legacy).expect("legacy state deserializes");
        assert!(state.check_all());
        assert_eq!(state.used_capabilities, CapabilitySet::none());
        assert!(state.trusted_artifacts.is_empty());
        assert!(state.suppression.is_none());
    }
}
