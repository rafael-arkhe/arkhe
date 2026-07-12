use crate::{CheckpointStatus, Digest, EvaluationResult, RsiError};
use arkhe_core::hash::blake3_hash;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

/// Registro imutável de uma iteração do loop RSI.
///
/// Auto-suficiente para replay: dado o histórico completo de `IterationRecord`s,
/// é possível reconstruir a sequência de artefatos aplicados sem outra fonte de dados.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IterationRecord {
    pub iteration: u64,
    pub baseline_id: Digest,
    pub candidate_id: Option<Digest>,
    pub applied_id: Option<Digest>,
    pub evaluation: Option<EvaluationResult>,
    pub final_score: f64,
    /// Reflete apenas se um artefato foi de fato aplicado.
    /// Não é gateado por score — essa decisão é do `Verifier`, não do registro.
    pub was_applied: bool,
    pub checkpoint_status: CheckpointStatus,
    pub previous_hash: Option<Digest>,
    pub hash: Digest,
    pub timestamp_secs: u64,
    pub rollback_id: Option<Digest>,
}

impl IterationRecord {
    /// `candidate_id`/`applied_id` são identificadores de conteúdo (`Artifact::id`),
    /// não o `Artifact` inteiro — o registro nunca precisou do conteúdo, só do id,
    /// o que também é o que torna um rollback possível sem reconstruir o artefato
    /// original (só o `Digest` do checkpoint é necessário).
    pub fn new(
        iteration: u64,
        baseline_id: Digest,
        candidate_id: Option<Digest>,
        applied_id: Option<Digest>,
        evaluation: Option<EvaluationResult>,
        checkpoint_status: CheckpointStatus,
        previous_hash: Option<Digest>,
    ) -> Self {
        let final_score = evaluation.as_ref().map(|e| e.score).unwrap_or(0.0);
        let was_applied = applied_id.is_some();

        let mut record = Self {
            iteration,
            baseline_id,
            candidate_id,
            applied_id,
            evaluation,
            final_score,
            was_applied,
            checkpoint_status,
            previous_hash,
            hash: Digest::default(),
            timestamp_secs: now_secs(),
            rollback_id: None,
        };
        record.hash = record.compute_hash();
        record
    }

    /// Hash BLAKE3 dos campos que definem a identidade do registro.
    pub fn compute_hash(&self) -> Digest {
        let mut buf = Vec::new();
        buf.extend_from_slice(&self.iteration.to_le_bytes());
        buf.extend_from_slice(&self.baseline_id);
        if let Some(id) = &self.candidate_id {
            buf.extend_from_slice(id);
        }
        if let Some(id) = &self.applied_id {
            buf.extend_from_slice(id);
        }
        buf.extend_from_slice(&self.final_score.to_le_bytes());
        buf.push(self.was_applied as u8);
        buf.push(self.checkpoint_status as u8);
        if let Some(prev) = &self.previous_hash {
            buf.extend_from_slice(prev);
        }
        buf.extend_from_slice(&self.timestamp_secs.to_le_bytes());
        if let Some(rid) = &self.rollback_id {
            buf.extend_from_slice(rid);
        }
        blake3_hash(&buf)
    }

    /// Marca este registro como resultado de um rollback, associando-o ao
    /// registro do checkpoint revertido e recomputando o hash para refletir
    /// essa mudança — `rollback_id` participa do hash, então setá-lo depois
    /// de `new()` exige recomputar, ou o registro fica com um hash que não
    /// corresponde ao seu próprio conteúdo.
    pub fn with_rollback(mut self, rollback_id: Digest) -> Self {
        self.rollback_id = Some(rollback_id);
        self.hash = self.compute_hash();
        self
    }

    /// Verifica se este registro encadeia corretamente a partir de `previous`.
    pub fn verify_chain(&self, previous: &Self) -> Result<(), RsiError> {
        match self.previous_hash {
            Some(h) if h == previous.hash => Ok(()),
            actual => Err(RsiError::HashChainBroken {
                expected: previous.hash,
                actual,
            }),
        }
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before unix epoch")
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Artifact, ArtifactKind};

    #[test]
    fn was_applied_reflects_application_not_score() {
        let artifact = Artifact::new(ArtifactKind::Code, "fn main() {}");
        let low_score_but_applied = IterationRecord::new(
            1,
            artifact.id,
            None,
            Some(artifact.id),
            Some(EvaluationResult::new(0.1)),
            CheckpointStatus::Validated,
            None,
        );
        assert!(low_score_but_applied.was_applied);
    }

    #[test]
    fn chain_verification_detects_break() {
        let a1 = Artifact::new(ArtifactKind::Code, "fn main() {}");
        let a2 = Artifact::new(ArtifactKind::Code, "fn main() { println!(\"hi\"); }");

        let rec1 = IterationRecord::new(
            1,
            a1.id,
            None,
            Some(a1.id),
            Some(EvaluationResult::new(0.9)),
            CheckpointStatus::Validated,
            None,
        );
        let rec2 = IterationRecord::new(
            2,
            a2.id,
            None,
            Some(a2.id),
            Some(EvaluationResult::new(0.85)),
            CheckpointStatus::Validated,
            Some(rec1.hash),
        );

        assert!(rec2.verify_chain(&rec1).is_ok());
        assert!(rec1.verify_chain(&rec2).is_err());
    }
}
