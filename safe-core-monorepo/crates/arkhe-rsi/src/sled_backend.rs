//! Backend de disco para `RegistryBackend`, usando `sled`.

use crate::registry::RegistryBackend;
use arkhe_rsi_core::{IterationRecord, RsiError};
use std::path::Path;

/// `RegistryBackend` persistente em disco.
///
/// A chave de cada registro é `iteration` em big-endian: o sled itera as
/// chaves em ordem lexicográfica de bytes, e big-endian é a única codificação
/// de inteiro que faz essa ordem de bytes coincidir com a ordem numérica —
/// com little-endian, `all()` devolveria os registros fora de ordem e
/// `Registry::restore` rejeitaria a cadeia por hash quebrado.
pub struct SledRegistryBackend {
    tree: sled::Tree,
}

impl SledRegistryBackend {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, RsiError> {
        let db = sled::open(path).map_err(|e| RsiError::Backend(e.to_string()))?;
        let tree = db
            .open_tree("iteration_records")
            .map_err(|e| RsiError::Backend(e.to_string()))?;
        Ok(Self { tree })
    }
}

impl RegistryBackend for SledRegistryBackend {
    fn append(&self, record: &IterationRecord) -> Result<(), RsiError> {
        let key = record.iteration.to_be_bytes();
        let value = serde_json::to_vec(record).map_err(|e| RsiError::Backend(e.to_string()))?;
        self.tree
            .insert(key, value)
            .map_err(|e| RsiError::Backend(e.to_string()))?;
        self.tree
            .flush()
            .map_err(|e| RsiError::Backend(e.to_string()))?;
        Ok(())
    }

    fn all(&self) -> Result<Vec<IterationRecord>, RsiError> {
        self.tree
            .iter()
            .values()
            .map(|res| {
                let bytes = res.map_err(|e| RsiError::Backend(e.to_string()))?;
                serde_json::from_slice(&bytes).map_err(|e| RsiError::Backend(e.to_string()))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::Registry;
    use arkhe_rsi_core::{Artifact, ArtifactKind, CheckpointStatus, EvaluationResult};

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
    fn history_survives_reopening_the_same_path() {
        let dir = tempfile::tempdir().unwrap();

        {
            let backend = SledRegistryBackend::open(dir.path()).unwrap();
            let mut registry = Registry::new(Box::new(backend), 0.5);
            let rec1 = applied_record(1, "fn main() {}", 0.9, None);
            let hash1 = rec1.hash;
            registry.append(rec1).unwrap();

            let rec2 = applied_record(2, "fn main() { println!(\"hi\"); }", 0.85, Some(hash1));
            registry.append(rec2).unwrap();
        } // registry (e o sled::Db por trás dela) sai de escopo e fecha aqui

        let reopened = SledRegistryBackend::open(dir.path()).unwrap();
        let restored = Registry::restore(Box::new(reopened), 0.5).unwrap();

        assert_eq!(restored.history().count(), 2);
        assert_eq!(
            restored.last_stable_checkpoint().unwrap().record_hash,
            restored.history().last().unwrap().hash
        );
    }

    #[test]
    fn records_are_replayed_in_iteration_order() {
        let dir = tempfile::tempdir().unwrap();
        let backend = SledRegistryBackend::open(dir.path()).unwrap();

        let rec1 = applied_record(1, "a", 0.9, None);
        let hash1 = rec1.hash;
        backend.append(&rec1).unwrap();
        let rec2 = applied_record(2, "b", 0.9, Some(hash1));
        backend.append(&rec2).unwrap();

        let all = backend.all().unwrap();
        assert_eq!(all[0].iteration, 1);
        assert_eq!(all[1].iteration, 2);
    }
}
