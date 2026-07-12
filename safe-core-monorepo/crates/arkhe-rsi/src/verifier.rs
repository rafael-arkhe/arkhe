//! Verificador de invariantes sobre o histórico de `IterationRecord`.

use arkhe_rsi_core::{CheckpointStatus, Digest, IterationRecord, RsiError};
use std::collections::HashSet;

/// Verifica a cadeia de `IterationRecord`s e aplica os invariantes do RSI:
///
/// - a cadeia de hashes nunca pode quebrar;
/// - um checkpoint `RolledBack` sempre tem um `rollback_id`;
/// - o mesmo artefato candidato nunca é registrado duas vezes;
/// - o score final nunca cai mais que `max_score_drop` em relação à iteração anterior.
///
/// Em caso de erro, `push` não altera o estado do `Verifier` — o histórico e o
/// conjunto de candidatos vistos permanecem exatamente como estavam antes da chamada.
pub struct Verifier {
    history: Vec<IterationRecord>,
    seen_candidates: HashSet<Digest>,
    max_score_drop: f64,
}

impl Verifier {
    pub fn new(max_score_drop: f64) -> Self {
        Self {
            history: Vec::new(),
            seen_candidates: HashSet::new(),
            max_score_drop,
        }
    }

    /// Adiciona um novo registro ao histórico, rejeitando-o se algum invariante falhar.
    pub fn push(&mut self, record: IterationRecord) -> Result<(), RsiError> {
        let computed = record.compute_hash();
        if record.hash != computed {
            return Err(RsiError::RecordHashMismatch {
                declared: record.hash,
                computed,
            });
        }

        if let Some(last) = self.history.last() {
            record.verify_chain(last)?;

            // Um rollback é, por definição, uma correção para uma queda de score
            // anterior — não faz sentido bloqueá-lo pelo próprio invariante que
            // ele existe para remediar.
            let is_rollback = record.checkpoint_status == CheckpointStatus::RolledBack;
            if !is_rollback
                && score_drop_exceeds(last.final_score, record.final_score, self.max_score_drop)
            {
                return Err(RsiError::ScoreDropExceeded {
                    drop: last.final_score - record.final_score,
                    threshold: self.max_score_drop,
                });
            }
        }

        if record.checkpoint_status == CheckpointStatus::RolledBack && record.rollback_id.is_none() {
            return Err(RsiError::RollbackMissingId);
        }

        if let Some(candidate_id) = record.candidate_id {
            if !self.seen_candidates.insert(candidate_id) {
                return Err(RsiError::DuplicateCandidate(candidate_id));
            }
        }

        self.history.push(record);
        Ok(())
    }

    /// Replay determinístico: percorre o histórico em ordem.
    pub fn replay(&self) -> impl Iterator<Item = &IterationRecord> {
        self.history.iter()
    }

    pub fn last(&self) -> Option<&IterationRecord> {
        self.history.last()
    }

    /// Reconstrói um `Verifier` a partir de um histórico carregado do disco
    /// (ex: um `Registry` restaurado), validando todos os registros em sequência.
    ///
    /// Diferente de `push`, que valida incrementalmente no caminho crítico de
    /// escrita, isto serve para auditoria pós-hoc de um histórico já existente
    /// (ex: em testes de propriedade que geram históricos aleatórios).
    #[cfg(test)]
    pub fn from_history(
        history: Vec<IterationRecord>,
        max_score_drop: f64,
    ) -> Result<Self, RsiError> {
        let mut verifier = Self::new(max_score_drop);
        for record in history {
            verifier.push(record)?;
        }
        Ok(verifier)
    }
}

/// `true` se a queda de score entre iterações consecutivas ultrapassa o limite permitido.
fn score_drop_exceeds(previous_score: f64, current_score: f64, max_drop: f64) -> bool {
    (previous_score - current_score) > max_drop
}

#[cfg(test)]
mod tests {
    use super::*;
    use arkhe_rsi_core::{Artifact, ArtifactKind, EvaluationResult};

    fn record(iteration: u64, content: &str, score: f64, previous_hash: Option<Digest>) -> IterationRecord {
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
    fn rejects_tampered_hash() {
        let mut rec = record(1, "fn main() {}", 0.9, None);
        rec.hash = Digest::default(); // simula corrupção/adulteração

        let mut verifier = Verifier::new(0.5);
        let err = verifier.push(rec).unwrap_err();
        assert!(matches!(err, RsiError::RecordHashMismatch { .. }));
    }

    #[test]
    fn rollback_is_exempt_from_score_drop_check() {
        let rec1 = record(1, "fn main() {}", 0.95, None);
        let rollback = IterationRecord::new(
            2,
            rec1.applied_id.unwrap(),
            None,
            None,
            None, // sem evaluation -> final_score cai para 0.0
            CheckpointStatus::RolledBack,
            Some(rec1.hash),
        )
        .with_rollback(rec1.hash);

        let mut verifier = Verifier::new(0.1); // threshold apertado
        verifier.push(rec1).unwrap();
        verifier.push(rollback).unwrap();
    }

    #[test]
    fn accepts_good_chain() {
        let rec1 = record(1, "fn main() {}", 0.9, None);
        let rec2 = record(2, "fn main() { println!(\"hi\"); }", 0.85, Some(rec1.hash));

        let mut verifier = Verifier::new(0.5);
        verifier.push(rec1).unwrap();
        verifier.push(rec2).unwrap();
        assert_eq!(verifier.replay().count(), 2);
    }

    #[test]
    fn rejects_broken_chain() {
        let rec1 = record(1, "fn main() {}", 0.9, None);
        let rec2 = record(2, "fn main() { println!(\"hi\"); }", 0.85, Some(rec1.hash));
        let rec3 = record(3, "fn main() { loop {} }", 0.95, Some(Digest::default()));

        let mut verifier = Verifier::new(0.5);
        verifier.push(rec1).unwrap();
        verifier.push(rec2).unwrap();
        let err = verifier.push(rec3).unwrap_err();
        assert!(matches!(err, RsiError::HashChainBroken { .. }));
    }

    #[test]
    fn rejects_excessive_score_drop() {
        let rec1 = record(1, "fn main() {}", 0.95, None);
        let rec2 = record(2, "fn main() { loop {} }", 0.3, Some(rec1.hash));

        let mut verifier = Verifier::new(0.1);
        verifier.push(rec1).unwrap();
        let err = verifier.push(rec2).unwrap_err();
        assert!(matches!(err, RsiError::ScoreDropExceeded { .. }));
    }

    #[test]
    fn rejects_duplicate_candidate() {
        let artifact = Artifact::new(ArtifactKind::Code, "fn main() {}");
        let rec1 = IterationRecord::new(
            1,
            artifact.id,
            Some(artifact.id),
            None,
            Some(EvaluationResult::new(0.9)),
            CheckpointStatus::Validated,
            None,
        );
        let rec2 = IterationRecord::new(
            2,
            artifact.id,
            Some(artifact.id),
            None,
            Some(EvaluationResult::new(0.9)),
            CheckpointStatus::Validated,
            Some(rec1.hash),
        );

        let mut verifier = Verifier::new(1.0);
        verifier.push(rec1).unwrap();
        let err = verifier.push(rec2).unwrap_err();
        assert!(matches!(err, RsiError::DuplicateCandidate(_)));
    }

    #[test]
    fn rejects_rollback_without_id() {
        let artifact = Artifact::new(ArtifactKind::Code, "fn main() {}");
        let rec = IterationRecord::new(
            1,
            artifact.id,
            None,
            Some(artifact.id),
            Some(EvaluationResult::new(0.9)),
            CheckpointStatus::RolledBack,
            None,
        );

        let mut verifier = Verifier::new(1.0);
        let err = verifier.push(rec).unwrap_err();
        assert!(matches!(err, RsiError::RollbackMissingId));
    }

    #[test]
    fn from_history_replays_and_validates() {
        let rec1 = record(1, "fn main() {}", 0.9, None);
        let rec2 = record(2, "fn main() { println!(\"hi\"); }", 0.85, Some(rec1.hash));

        let verifier = Verifier::from_history(vec![rec1, rec2], 0.5).unwrap();
        assert_eq!(verifier.replay().count(), 2);
    }
}
