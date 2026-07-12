//! `RollbackManager` — reverte o estado ativo para o último checkpoint estável.
//!
//! Um rollback não é um mecanismo especial: é, ele mesmo, um `IterationRecord`
//! novo (status `RolledBack`, com `rollback_id` apontando para o checkpoint
//! revertido), validado e persistido pelo mesmo caminho de qualquer outra
//! iteração — `Registry::append` / `Verifier::push`.

use crate::registry::Registry;
use arkhe_rsi_core::{CheckpointStatus, IterationRecord, RsiError};

pub struct RollbackManager;

impl RollbackManager {
    /// Reverte para o último checkpoint estável anterior ao estado atual,
    /// anexando um novo `IterationRecord` de status `RolledBack` ao `Registry`.
    ///
    /// Usa `last_stable_checkpoint_excluding`, não `last_stable_checkpoint`:
    /// o registro no topo do histórico normalmente também é um checkpoint —
    /// ele passou pelo `Verifier` quando foi aplicado. Se ele for o motivo do
    /// rollback (problema só detectado depois, em produção), "reverter para
    /// o último checkpoint estável" sem excluí-lo seria um no-op.
    ///
    /// O score do registro de rollback é copiado do registro original do
    /// checkpoint alvo (não fica 0.0) — é o que estamos voltando a considerar
    /// "bom", não uma nova falha.
    pub fn rollback(registry: &mut Registry) -> Result<IterationRecord, RsiError> {
        let last = registry
            .history()
            .last()
            .cloned()
            .ok_or(RsiError::NoStableCheckpoint)?;

        let target = registry
            .last_stable_checkpoint_excluding(last.hash)
            .cloned()
            .ok_or(RsiError::NoStableCheckpoint)?;

        let target_record = registry
            .history()
            .find(|r| r.hash == target.record_hash)
            .cloned()
            .expect("um checkpoint sempre se origina de um registro no histórico do mesmo registry");

        let record = IterationRecord::new(
            last.iteration + 1,
            target.applied_id,
            None,
            Some(target.applied_id),
            target_record.evaluation.clone(),
            CheckpointStatus::RolledBack,
            Some(last.hash),
        )
        .with_rollback(target.record_hash);

        registry.append(record.clone())?;
        Ok(record)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::InMemoryRegistryBackend;
    use arkhe_rsi_core::{Artifact, ArtifactKind, EvaluationResult};

    fn applied_record(iteration: u64, content: &str, score: f64, previous_hash: Option<arkhe_rsi_core::Digest>) -> IterationRecord {
        let artifact = Artifact::new(ArtifactKind::Code, content);
        IterationRecord::new(
            iteration,
            artifact.id,
            None,
            Some(artifact.id),
            Some(EvaluationResult::new(score)),
            CheckpointStatus::Validated,
            previous_hash,
        )
    }

    #[test]
    fn rollback_without_any_checkpoint_fails() {
        let mut registry = Registry::new(Box::new(InMemoryRegistryBackend::new()), 0.5);
        let err = RollbackManager::rollback(&mut registry).unwrap_err();
        assert!(matches!(err, RsiError::NoStableCheckpoint));
    }

    #[test]
    fn rollback_reverts_to_last_stable_checkpoint_and_carries_its_score() {
        let mut registry = Registry::new(Box::new(InMemoryRegistryBackend::new()), 1.0);
        let good = applied_record(1, "fn main() {}", 0.95, None);
        let good_hash = good.hash;
        let good_applied_id = good.applied_id.unwrap();
        registry.append(good).unwrap();

        let bad = applied_record(2, "fn main() { loop {} }", 0.3, Some(good_hash));
        let bad_hash = bad.hash;
        registry.append(bad).unwrap();

        let rollback = RollbackManager::rollback(&mut registry).unwrap();

        assert_eq!(rollback.checkpoint_status, CheckpointStatus::RolledBack);
        assert_eq!(rollback.rollback_id, Some(good_hash));
        assert_eq!(rollback.applied_id, Some(good_applied_id));
        assert_eq!(rollback.previous_hash, Some(bad_hash));
        assert_eq!(rollback.final_score, 0.95); // score do checkpoint, não 0.0

        assert_eq!(registry.history().count(), 3);

        // last_stable_checkpoint (append-only, nunca esquece) ainda aponta
        // para o checkpoint abandonado — é "bad", não "good".
        assert_eq!(registry.last_stable_checkpoint().unwrap().record_hash, bad_hash);
        // active_checkpoint segue o rollback e reflete o que está de fato ativo.
        assert_eq!(registry.active_checkpoint().unwrap().record_hash, good_hash);
    }

    #[test]
    fn rollback_fails_when_only_checkpoint_is_the_current_tip() {
        let mut registry = Registry::new(Box::new(InMemoryRegistryBackend::new()), 1.0);
        registry.append(applied_record(1, "fn main() {}", 0.9, None)).unwrap();

        // O único checkpoint que existe É o estado atual — não há para onde reverter.
        let err = RollbackManager::rollback(&mut registry).unwrap_err();
        assert!(matches!(err, RsiError::NoStableCheckpoint));
    }
}
