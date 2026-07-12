//! `Registry` — ponto único de escrita do histórico do RSI.
//!
//! `Registry` não conhece o meio de armazenamento: ele depende apenas do
//! contrato `RegistryBackend`. Isso permite trocar um backend em memória por
//! um em disco/SQLite sem tocar em `Verifier`, `CheckpointStore` ou em quem
//! chama `Registry::append`.

use crate::checkpoint_store::{Checkpoint, CheckpointStore};
use crate::verifier::Verifier;
use arkhe_rsi_core::{IterationRecord, RsiError};
use std::sync::Mutex;

/// Backend de persistência do `Registry`.
pub trait RegistryBackend: Send + Sync {
    fn append(&self, record: &IterationRecord) -> Result<(), RsiError>;
    fn all(&self) -> Result<Vec<IterationRecord>, RsiError>;
}

/// Backend em memória — útil para testes e processos de vida curta.
#[derive(Default)]
pub struct InMemoryRegistryBackend {
    records: Mutex<Vec<IterationRecord>>,
}

impl InMemoryRegistryBackend {
    pub fn new() -> Self {
        Self::default()
    }
}

impl RegistryBackend for InMemoryRegistryBackend {
    fn append(&self, record: &IterationRecord) -> Result<(), RsiError> {
        self.records
            .lock()
            .expect("registry backend lock poisoned")
            .push(record.clone());
        Ok(())
    }

    fn all(&self) -> Result<Vec<IterationRecord>, RsiError> {
        Ok(self
            .records
            .lock()
            .expect("registry backend lock poisoned")
            .clone())
    }
}

/// Valida invariantes via `Verifier` e só então persiste no `RegistryBackend`.
pub struct Registry {
    backend: Box<dyn RegistryBackend>,
    verifier: Verifier,
    checkpoints: CheckpointStore,
}

impl Registry {
    pub fn new(backend: Box<dyn RegistryBackend>, max_score_drop: f64) -> Self {
        Self {
            backend,
            verifier: Verifier::new(max_score_drop),
            checkpoints: CheckpointStore::new(),
        }
    }

    /// Reabre um `Registry` a partir de um backend que já contém histórico
    /// (ex: reiniciar o processo com um backend em disco), revalidando toda
    /// a cadeia antes de aceitar novas escritas.
    pub fn restore(backend: Box<dyn RegistryBackend>, max_score_drop: f64) -> Result<Self, RsiError> {
        let history = backend.all()?;
        let mut verifier = Verifier::new(max_score_drop);
        let mut checkpoints = CheckpointStore::new();
        for record in &history {
            verifier.push(record.clone())?;
            checkpoints.record_if_validated(record);
        }
        Ok(Self {
            backend,
            verifier,
            checkpoints,
        })
    }

    /// Valida e persiste um novo registro. Se a validação falhar, nada é
    /// escrito no backend.
    ///
    /// Isto assume que `backend.append` não falha depois que a validação já
    /// passou — verdadeiro para `InMemoryRegistryBackend`. Um backend que possa
    /// falhar de fato (disco cheio, rede fora) precisaria de uma transação
    /// (ou de um log de intenção) para manter essa garantia; hoje não existe.
    pub fn append(&mut self, record: IterationRecord) -> Result<(), RsiError> {
        self.verifier.push(record.clone())?;
        self.backend.append(&record)?;
        self.checkpoints.record_if_validated(&record);
        Ok(())
    }

    pub fn history(&self) -> impl Iterator<Item = &IterationRecord> {
        self.verifier.replay()
    }

    /// Último registro que já foi `Validated` com artefato aplicado — inclui
    /// checkpoints que já tenham sido abandonados por um rollback posterior.
    /// Para saber o que está realmente ativo agora, use [`Self::active_checkpoint`].
    pub fn last_stable_checkpoint(&self) -> Option<&Checkpoint> {
        self.checkpoints.last_stable()
    }

    pub fn last_stable_checkpoint_excluding(&self, exclude: arkhe_rsi_core::Digest) -> Option<&Checkpoint> {
        self.checkpoints.last_stable_excluding(exclude)
    }

    /// O checkpoint realmente ativo agora.
    ///
    /// `CheckpointStore` é um log append-only: um rollback nunca apaga o
    /// checkpoint que ele abandona, só acrescenta um novo `IterationRecord`
    /// `RolledBack` no topo do histórico. Por isso `last_stable_checkpoint`
    /// sozinho pode devolver um checkpoint já superado. Este método olha o
    /// topo do histórico: se for um rollback, segue `rollback_id` até o
    /// checkpoint de destino; senão, é o mesmo que `last_stable_checkpoint`.
    pub fn active_checkpoint(&self) -> Option<&Checkpoint> {
        let tip = self.verifier.replay().last()?;
        if tip.checkpoint_status == arkhe_rsi_core::CheckpointStatus::RolledBack {
            self.checkpoints.find_by_record_hash(tip.rollback_id?)
        } else {
            self.checkpoints.last_stable()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arkhe_rsi_core::{Artifact, ArtifactKind, CheckpointStatus, Digest, EvaluationResult};

    fn applied_record(
        iteration: u64,
        content: &str,
        score: f64,
        previous_hash: Option<Digest>,
    ) -> IterationRecord {
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
    fn append_persists_and_creates_checkpoint() {
        let mut registry = Registry::new(Box::new(InMemoryRegistryBackend::new()), 0.5);
        let rec1 = applied_record(1, "fn main() {}", 0.9, None);
        registry.append(rec1.clone()).unwrap();

        assert_eq!(registry.history().count(), 1);
        assert_eq!(
            registry.last_stable_checkpoint().unwrap().applied_id,
            rec1.applied_id.unwrap()
        );
    }

    #[test]
    fn append_rejects_invalid_record_without_persisting() {
        let mut registry = Registry::new(Box::new(InMemoryRegistryBackend::new()), 0.1);
        let rec1 = applied_record(1, "fn main() {}", 0.95, None);
        registry.append(rec1.clone()).unwrap();

        let bad = applied_record(2, "fn main() { loop {} }", 0.2, Some(rec1.hash));
        let err = registry.append(bad).unwrap_err();
        assert!(matches!(err, RsiError::ScoreDropExceeded { .. }));
        assert_eq!(registry.history().count(), 1);
    }

    #[test]
    fn restore_revalidates_existing_backend_history() {
        let backend = InMemoryRegistryBackend::new();
        backend.append(&applied_record(1, "fn main() {}", 0.9, None)).unwrap();

        let restored = Registry::restore(Box::new(backend), 0.5).unwrap();
        assert_eq!(restored.history().count(), 1);
        assert!(restored.last_stable_checkpoint().is_some());
    }
}
