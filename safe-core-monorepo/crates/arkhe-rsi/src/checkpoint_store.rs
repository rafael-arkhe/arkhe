//! Checkpoints estáveis — alvos de rollback.

use arkhe_rsi_core::{CheckpointStatus, Digest, IterationRecord};

/// Um checkpoint estável: uma iteração cujo `IterationRecord` foi `Validated`
/// e que tem um artefato aplicado — portanto um alvo válido de rollback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checkpoint {
    pub iteration: u64,
    pub applied_id: Digest,
    pub record_hash: Digest,
}

/// Mantém a lista de checkpoints `Validated`, na ordem em que ocorreram.
#[derive(Debug, Default)]
pub struct CheckpointStore {
    checkpoints: Vec<Checkpoint>,
}

impl CheckpointStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registra `record` como checkpoint se, e somente se, ele estiver
    /// `Validated` e tiver um artefato aplicado. Registros `Pending` ou
    /// `RolledBack` são ignorados silenciosamente — não são alvos de rollback.
    pub fn record_if_validated(&mut self, record: &IterationRecord) {
        if record.checkpoint_status == CheckpointStatus::Validated {
            if let Some(applied_id) = record.applied_id {
                self.checkpoints.push(Checkpoint {
                    iteration: record.iteration,
                    applied_id,
                    record_hash: record.hash,
                });
            }
        }
    }

    /// O checkpoint estável mais recente.
    pub fn last_stable(&self) -> Option<&Checkpoint> {
        self.checkpoints.last()
    }

    /// O checkpoint estável mais recente que não seja `exclude`.
    ///
    /// Existe porque o estado que motiva um rollback normalmente também foi
    /// gravado como checkpoint — ele passou no `Verifier` no momento em que
    /// foi aplicado; só depois, fora do alcance do `Verifier` (em produção,
    /// via telemetria), é que se descobre que ele é o problema. Sem essa
    /// exclusão, "reverter para o último checkpoint estável" seria um no-op:
    /// reverteria para o próprio estado que se está tentando abandonar.
    pub fn last_stable_excluding(&self, exclude: Digest) -> Option<&Checkpoint> {
        self.checkpoints.iter().rev().find(|c| c.record_hash != exclude)
    }

    /// Busca um checkpoint pelo hash do `IterationRecord` que o originou.
    pub fn find_by_record_hash(&self, hash: Digest) -> Option<&Checkpoint> {
        self.checkpoints.iter().find(|c| c.record_hash == hash)
    }

    pub fn len(&self) -> usize {
        self.checkpoints.len()
    }

    pub fn is_empty(&self) -> bool {
        self.checkpoints.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arkhe_rsi_core::{Artifact, ArtifactKind, EvaluationResult, IterationRecord};

    #[test]
    fn only_validated_records_with_applied_artifact_become_checkpoints() {
        let mut store = CheckpointStore::new();
        let artifact = Artifact::new(ArtifactKind::Code, "fn main() {}");

        let pending = IterationRecord::new(
            1,
            artifact.id,
            None,
            None,
            None,
            CheckpointStatus::Pending,
            None,
        );
        store.record_if_validated(&pending);
        assert!(store.is_empty());

        let validated = IterationRecord::new(
            2,
            artifact.id,
            None,
            Some(artifact.id),
            Some(EvaluationResult::new(0.9)),
            CheckpointStatus::Validated,
            None,
        );
        store.record_if_validated(&validated);
        assert_eq!(store.len(), 1);
        assert_eq!(store.last_stable().unwrap().applied_id, artifact.id);
    }

    #[test]
    fn validated_record_without_applied_artifact_is_not_a_checkpoint() {
        let mut store = CheckpointStore::new();
        let artifact = Artifact::new(ArtifactKind::Code, "fn main() {}");
        let validated_but_not_applied = IterationRecord::new(
            1,
            artifact.id,
            Some(artifact.id),
            None,
            Some(EvaluationResult::new(0.9)),
            CheckpointStatus::Validated,
            None,
        );
        store.record_if_validated(&validated_but_not_applied);
        assert!(store.is_empty());
    }
}
