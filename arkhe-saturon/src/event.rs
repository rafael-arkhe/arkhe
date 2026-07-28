//! Events flowing into the orchestrator, and commands flowing out.
//!
//! The orchestrator is a consumer of [`SaturonEvent`] and a producer of
//! [`SaturonCommand`]. Nothing here performs work — these are messages.

use crate::types::{PhysicalVariable, VerificationResult};

/// Events the orchestrator reacts to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaturonEvent {
    /// An extractor turned a paper into a candidate hypothesis + equation.
    ArxivDiscovery {
        hypothesis_id: String,
        arxiv_toon_id: String,
        hypothesis_text: String,
        math_syntax: String,
        variables: Vec<PhysicalVariable>,
    },
    /// A verifier delivered a result out-of-band (not via the inline bridge).
    VerificationResultReceived { result: VerificationResult },
    /// Script generation failed before anything could be dispatched.
    ScriptGenerationError {
        hypothesis_id: String,
        error_log: String,
    },
}

/// Commands the orchestrator emits for a UI / downstream consumer to act on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaturonCommand {
    /// A hypothesis passed verification.
    HypothesisVerified { hypothesis_id: String },
    /// A hypothesis was refuted.
    HypothesisRejected {
        hypothesis_id: String,
        reason: String,
    },
    /// Surface an error to the operator.
    ShowError {
        hypothesis_id: String,
        message: String,
    },
}
