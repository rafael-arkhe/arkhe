//! Tipos fundacionais do loop RSI: artefatos, avaliações e o registro de iteração.

#![deny(unsafe_code)]

mod artifact;
mod checkpoint;
mod error;
mod evaluation;
mod iteration_record;
mod validation;

pub use artifact::{Artifact, ArtifactKind};
pub use checkpoint::CheckpointStatus;
pub use error::RsiError;
pub use evaluation::EvaluationResult;
pub use iteration_record::IterationRecord;
pub use validation::ValidationReport;

/// Identificador de conteúdo — reaproveita o hash BLAKE3 já usado em `arkhe-core`.
pub type Digest = arkhe_core::ArkheHash;
