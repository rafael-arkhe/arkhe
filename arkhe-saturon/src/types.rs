//! Core domain types for ARKHE-SATURON.
//!
//! These are plain data. No math lives here — a `Hypothesis` merely *carries*
//! the equation text that an external symbolic solver will check.

/// A physical variable referenced by a hypothesis' equation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalVariable {
    /// Human-readable name, e.g. "surface area".
    pub name: String,
    /// Symbol used in the equation, e.g. "A".
    pub symbol: String,
    /// Unit string, e.g. "m^2".
    pub unit: String,
    /// Free-text description.
    pub description: String,
}

/// The kind of check a verifier performs on a hypothesis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerificationType {
    /// LHS - RHS simplifies to exactly zero.
    AlgebraicIdentity,
    /// Both sides reduce to the same physical dimension.
    DimensionalConsistency,
    /// A numeric evaluation stays within a stated bound.
    NumericBound,
}

impl VerificationType {
    pub fn as_str(self) -> &'static str {
        match self {
            VerificationType::AlgebraicIdentity => "algebraic_identity",
            VerificationType::DimensionalConsistency => "dimensional_consistency",
            VerificationType::NumericBound => "numeric_bound",
        }
    }
}

/// Lifecycle of a hypothesis inside the pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HypothesisStatus {
    /// Registered but not yet dispatched.
    Proposed,
    /// Handed to the solver; awaiting a verdict.
    Verifying,
    /// Solver confirmed the check.
    Verified,
    /// Solver refuted the check.
    Rejected,
    /// The pipeline failed before a verdict (bad script, bridge error, …).
    Errored,
}

impl HypothesisStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            HypothesisStatus::Proposed => "proposed",
            HypothesisStatus::Verifying => "verifying",
            HypothesisStatus::Verified => "verified",
            HypothesisStatus::Rejected => "rejected",
            HypothesisStatus::Errored => "errored",
        }
    }
}

/// A hypothesis extracted from an arXiv paper.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hypothesis {
    /// Stable internal id, e.g. "HYP-001".
    pub id: String,
    /// TOON id of the source paper anchor.
    pub arxiv_toon_id: String,
    /// The natural-language claim.
    pub text: String,
    /// The equation, in the solver's input syntax.
    pub math_syntax: String,
    /// Variables that appear in the equation.
    pub variables: Vec<PhysicalVariable>,
    /// Current lifecycle status.
    pub status: HypothesisStatus,
}

/// Outcome of a delegated verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationResult {
    /// Id of the hypothesis this result is about.
    pub hypothesis_id: String,
    /// DID (or other identifier) of the verifier that produced it.
    pub verifier: String,
    /// Which kind of check was run.
    pub check_type: VerificationType,
    /// Whether the check passed.
    pub passed: bool,
    /// Human-readable detail / reason (e.g. residual, failure cause).
    pub detail: String,
    /// The solver that produced the verdict, e.g. "SymPy 1.13".
    pub solver_used: String,
}
