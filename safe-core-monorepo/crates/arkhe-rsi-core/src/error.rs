use crate::Digest;
use thiserror::Error;

/// Erros do loop RSI e do verificador de invariantes.
#[derive(Debug, Error)]
pub enum RsiError {
    #[error("hash chain broken: expected previous_hash {expected:?}, got {actual:?}")]
    HashChainBroken {
        expected: Digest,
        actual: Option<Digest>,
    },

    #[error("checkpoint marked RolledBack without a rollback_id")]
    RollbackMissingId,

    #[error("candidate artifact {0:?} was already recorded in an earlier iteration")]
    DuplicateCandidate(Digest),

    #[error("score drop of {drop:.4} exceeds max_score_drop of {threshold:.4}")]
    ScoreDropExceeded { drop: f64, threshold: f64 },

    #[error("registry backend error: {0}")]
    Backend(String),

    #[error("no stable checkpoint available for rollback")]
    NoStableCheckpoint,

    #[error("candidate {0:?} was vetoed by a reviewer")]
    ApprovalVetoed(Digest),

    #[error("candidate {0:?} has not yet reached approval quorum")]
    ApprovalPending(Digest),

    #[error("record hash does not match its own content — declared {declared:?}, computed {computed:?}")]
    RecordHashMismatch { declared: Digest, computed: Digest },
}
