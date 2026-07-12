//! `ApprovalWorkflow` — política de aprovação por quorum.
//!
//! Separação mecanismo/política: `Verifier`/`Registry` sabem *como* validar
//! um `IterationRecord` (hash chain, score drop, rollback). `ApprovalWorkflow`
//! decide *se* um candidato pode prosseguir, com base em votos humanos — uma
//! decisão que nada tem a ver com os invariantes estruturais do registro.

use arkhe_rsi_core::{Digest, RsiError};
use std::collections::HashMap;

/// Voto de um revisor sobre um candidato.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Vote {
    Approve,
    Reject,
}

/// Um candidato é aprovado quando atinge `quorum` votos `Approve` de
/// revisores distintos. Um único `Reject` veta o candidato imediatamente,
/// independente de quantos `Approve` existam.
#[derive(Debug)]
pub struct ApprovalWorkflow {
    quorum: usize,
    votes: HashMap<Digest, HashMap<String, Vote>>,
}

impl ApprovalWorkflow {
    pub fn new(quorum: usize) -> Self {
        assert!(quorum > 0, "quorum must be at least 1");
        Self {
            quorum,
            votes: HashMap::new(),
        }
    }

    /// Registra o voto de `reviewer` para `candidate_id`. Um novo voto do
    /// mesmo revisor substitui o anterior.
    pub fn cast_vote(&mut self, candidate_id: Digest, reviewer: impl Into<String>, vote: Vote) {
        self.votes
            .entry(candidate_id)
            .or_default()
            .insert(reviewer.into(), vote);
    }

    /// `true` se algum revisor votou `Reject` para este candidato.
    pub fn is_vetoed(&self, candidate_id: &Digest) -> bool {
        self.votes
            .get(candidate_id)
            .map_or(false, |v| v.values().any(|vote| *vote == Vote::Reject))
    }

    /// `true` se o candidato atingiu o quorum de aprovações e não foi vetado.
    pub fn is_approved(&self, candidate_id: &Digest) -> bool {
        if self.is_vetoed(candidate_id) {
            return false;
        }
        self.votes.get(candidate_id).map_or(false, |v| {
            v.values().filter(|vote| **vote == Vote::Approve).count() >= self.quorum
        })
    }

    /// Decide se um candidato pode prosseguir para aplicação.
    pub fn decide(&self, candidate_id: &Digest) -> Result<(), RsiError> {
        if self.is_vetoed(candidate_id) {
            return Err(RsiError::ApprovalVetoed(*candidate_id));
        }
        if !self.is_approved(candidate_id) {
            return Err(RsiError::ApprovalPending(*candidate_id));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arkhe_rsi_core::{Artifact, ArtifactKind};

    fn candidate_id() -> Digest {
        Artifact::new(ArtifactKind::Code, "fn main() {}").id
    }

    #[test]
    fn quorum_of_approvals_is_required() {
        let id = candidate_id();
        let mut workflow = ApprovalWorkflow::new(2);
        workflow.cast_vote(id, "alice", Vote::Approve);
        assert!(workflow.decide(&id).is_err());

        workflow.cast_vote(id, "bob", Vote::Approve);
        assert!(workflow.decide(&id).is_ok());
    }

    #[test]
    fn single_veto_blocks_approval_even_with_quorum() {
        let id = candidate_id();
        let mut workflow = ApprovalWorkflow::new(1);
        workflow.cast_vote(id, "alice", Vote::Approve);
        workflow.cast_vote(id, "bob", Vote::Reject);
        assert!(matches!(workflow.decide(&id), Err(RsiError::ApprovalVetoed(_))));
    }

    #[test]
    fn reviewer_can_change_their_vote() {
        let id = candidate_id();
        let mut workflow = ApprovalWorkflow::new(1);
        workflow.cast_vote(id, "alice", Vote::Reject);
        assert!(workflow.is_vetoed(&id));

        workflow.cast_vote(id, "alice", Vote::Approve);
        assert!(!workflow.is_vetoed(&id));
        assert!(workflow.is_approved(&id));
    }

    #[test]
    fn unknown_candidate_is_pending_not_vetoed() {
        let id = candidate_id();
        let workflow = ApprovalWorkflow::new(1);
        assert!(!workflow.is_vetoed(&id));
        assert!(matches!(workflow.decide(&id), Err(RsiError::ApprovalPending(_))));
    }
}
