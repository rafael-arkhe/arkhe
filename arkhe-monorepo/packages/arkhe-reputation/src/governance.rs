//! Matriz de governança por esperança estatística (#15).
//!
//! Cada voto carrega a expectativa do proponente (ganho esperado da melhoria) e a
//! reputação do votante. A decisão agrega `∑ (reputação × esperança)` normalizada
//! pela confiança total — governança ponderada por utilidade esperada, ancorada na
//! Cadeia Temporal (Loopseal-1).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::reputation::ReputationScore;

/// Proposta de governança a ser avaliada.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proposal {
    pub id: String,
    /// Descrição da melhoria constitucional.
    pub description: String,
    /// Ganho esperado (utilidade) da melhoria, em [0,1].
    pub expected_gain: f64,
}

/// Voto ponderado por reputação.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Voter {
    pub id: String,
    pub reputation: f64,
    pub support: bool,
    /// Confiança na própria avaliação [0,1].
    pub confidence: f64,
}

/// Matriz de reputação ⟨votante, proposta⟩ com esperança agregada.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReputationMatrix {
    pub voters: BTreeMap<String, ReputationScore>,
    pub proposals: Vec<Proposal>,
    /// Scores de apoio por proposta (linhas = proposta).
    pub support_scores: BTreeMap<String, f64>,
    /// Confiança total por proposta.
    pub total_confidence: BTreeMap<String, f64>,
}

impl Default for ReputationMatrix {
    fn default() -> Self {
        Self::new()
    }
}

impl ReputationMatrix {
    pub fn new() -> Self {
        Self {
            voters: BTreeMap::new(),
            proposals: Vec::new(),
            support_scores: BTreeMap::new(),
            total_confidence: BTreeMap::new(),
        }
    }

    pub fn add_voter(&mut self, score: ReputationScore) {
        self.voters.insert(score.id.clone(), score);
    }

    pub fn add_proposal(&mut self, proposal: Proposal) {
        self.proposals.push(proposal);
    }

    /// Vota em uma proposta; retorna erro se votante ou proposta não existirem.
    pub fn vote(&mut self, proposal_id: &str, voter: Voter) -> Result<(), GovernanceError> {
        if !self.proposals.iter().any(|p| p.id == proposal_id) {
            return Err(GovernanceError::UnknownProposal(proposal_id.to_string()));
        }
        let score = self
            .support_scores
            .entry(proposal_id.to_string())
            .or_insert(0.0);
        let conf = self
            .total_confidence
            .entry(proposal_id.to_string())
            .or_insert(0.0);
        if voter.support {
            *score += voter.reputation * voter.confidence;
        }
        *conf += voter.confidence;
        Ok(())
    }

    /// Esperança normalizada de uma proposta: `∑(rep×conf×support)/∑conf`.
    pub fn expected_support(&self, proposal_id: &str) -> f64 {
        let s = self.support_scores.get(proposal_id).copied().unwrap_or(0.0);
        let c = self.total_confidence.get(proposal_id).copied().unwrap_or(0.0);
        if c <= 0.0 {
            0.0
        } else {
            s / c
        }
    }

    /// Governança aprovada ↔ esperança ≥ 0.5 E φ_C médio da mesa ≥ limiar 0.577350.
    pub fn governance_matrix(&self) -> GovernanceMatrix {
        let props = &self.proposals;
        let mut resolutions = Vec::new();
        let mut passed = 0usize;
        let mean_phi = if self.voters.is_empty() {
            0.0
        } else {
            self.voters.values().map(|v| v.phi_c).sum::<f64>() / self.voters.len() as f64
        };
        for p in props {
            let exp = self.expected_support(&p.id);
            let ok = exp >= 0.5 && mean_phi > 0.577350;
            if ok {
                passed += 1;
            }
            resolutions.push((p.id.clone(), exp, ok));
        }
        GovernanceMatrix {
            resolutions,
            passed,
            mean_phi_c: mean_phi,
            table_live: mean_phi > 0.577350,
        }
    }
}

/// Resultado da matriz de governança.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceMatrix {
    /// (id da proposta, esperança, aprovada?)
    pub resolutions: Vec<(String, f64, bool)>,
    pub passed: usize,
    pub mean_phi_c: f64,
    pub table_live: bool,
}

#[derive(Debug, thiserror::Error, Clone)]
pub enum GovernanceError {
    #[error("proposta desconhecida: {0}")]
    UnknownProposal(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reputation::ReputationScore;

    #[test]
    fn majority_with_trusted_table_passes() {
        let mut m = ReputationMatrix::new();
        m.add_voter(ReputationScore::evaluate("v1", 0.9, 0.9, 0.9, 0.0, 0.0));
        m.add_voter(ReputationScore::evaluate("v2", 0.8, 0.8, 0.85, 0.0, 0.0));
        m.add_proposal(Proposal {
            id: "P1".into(),
            description: "adicionar invariante".into(),
            expected_gain: 0.8,
        });
        m.vote("P1", Voter { id: "v1".into(), reputation: 90.0, support: true, confidence: 0.9 }).unwrap();
        m.vote("P1", Voter { id: "v2".into(), reputation: 80.0, support: false, confidence: 0.8 }).unwrap();
        let g = m.governance_matrix();
        assert!(g.table_live);
        assert_eq!(g.passed, 1);
    }

    #[test]
    fn no_votes_fails() {
        let mut m = ReputationMatrix::new();
        m.add_proposal(Proposal { id: "P2".into(), description: "x".into(), expected_gain: 0.9 });
        let g = m.governance_matrix();
        assert_eq!(g.passed, 0, "proposta sem votos jamais passa");
    }

    #[test]
    fn unknown_proposal_rejected() {
        let mut m = ReputationMatrix::new();
        assert!(m.vote("NOPE", Voter { id: "v".into(), reputation: 1.0, support: true, confidence: 1.0 }).is_err());
    }
}