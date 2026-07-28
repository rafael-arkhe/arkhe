//! The Saturon state and its pure reducer.
//!
//! State transitions happen only through [`SaturonState::reduce`], applied to
//! an [`Action`]. The reducer is synchronous, deterministic, and side-effect
//! free apart from mutating `self` and appending to the audit ledger — so the
//! same action sequence always yields the same state.

use std::collections::BTreeMap;

use crate::types::{Hypothesis, HypothesisStatus, VerificationResult};

/// The only way to mutate [`SaturonState`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Insert a newly discovered hypothesis (status `Proposed`).
    RegisterHypothesis(Hypothesis),
    /// Move a known hypothesis to `Verifying`.
    BeginVerification { id: String },
    /// Fold in a verifier's result: sets `Verified`/`Rejected` and records it.
    ApplyResult { result: VerificationResult },
    /// Mark a hypothesis `Errored` (pipeline failure before a verdict).
    MarkErrored { id: String, log: String },
}

/// Immutable-by-convention pipeline state. Clone-able for snapshots.
#[derive(Debug, Clone, Default)]
pub struct SaturonState {
    /// Hypotheses keyed by id (BTree for deterministic iteration).
    pub hypotheses: BTreeMap<String, Hypothesis>,
    /// Every result ever applied, in arrival order.
    pub results: Vec<VerificationResult>,
    /// Append-only, human-readable audit trail.
    pub ledger: Vec<String>,
}

impl SaturonState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Apply one action. This is the single mutation entry point.
    pub fn reduce(&mut self, action: Action) {
        match action {
            Action::RegisterHypothesis(h) => {
                self.ledger.push(format!("register {}", h.id));
                self.hypotheses.insert(h.id.clone(), h);
            }
            Action::BeginVerification { id } => match self.hypotheses.get_mut(&id) {
                Some(h) => {
                    h.status = HypothesisStatus::Verifying;
                    self.ledger.push(format!("verifying {id}"));
                }
                None => self
                    .ledger
                    .push(format!("begin-verification for unknown id {id}")),
            },
            Action::ApplyResult { result } => {
                match self.hypotheses.get_mut(&result.hypothesis_id) {
                    Some(h) => {
                        h.status = if result.passed {
                            HypothesisStatus::Verified
                        } else {
                            HypothesisStatus::Rejected
                        };
                        self.ledger.push(format!(
                            "result {} -> {}",
                            result.hypothesis_id,
                            h.status.as_str()
                        ));
                    }
                    None => self.ledger.push(format!(
                        "result for unknown id {}",
                        result.hypothesis_id
                    )),
                }
                self.results.push(result);
            }
            Action::MarkErrored { id, log } => match self.hypotheses.get_mut(&id) {
                Some(h) => {
                    h.status = HypothesisStatus::Errored;
                    self.ledger.push(format!("errored {id}: {log}"));
                }
                None => self
                    .ledger
                    .push(format!("error for unknown id {id}: {log}")),
            },
        }
    }

    /// Current status of a hypothesis, if known.
    pub fn status_of(&self, id: &str) -> Option<HypothesisStatus> {
        self.hypotheses.get(id).map(|h| h.status)
    }

    /// How many hypotheses are currently `Verified`.
    pub fn verified_count(&self) -> usize {
        self.hypotheses
            .values()
            .filter(|h| h.status == HypothesisStatus::Verified)
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::VerificationType;

    fn hyp(id: &str) -> Hypothesis {
        Hypothesis {
            id: id.to_string(),
            arxiv_toon_id: "TOON-X".to_string(),
            text: "claim".to_string(),
            math_syntax: "a = b".to_string(),
            variables: vec![],
            status: HypothesisStatus::Proposed,
        }
    }

    fn result(id: &str, passed: bool) -> VerificationResult {
        VerificationResult {
            hypothesis_id: id.to_string(),
            verifier: "did:test".to_string(),
            check_type: VerificationType::AlgebraicIdentity,
            passed,
            detail: "d".to_string(),
            solver_used: "SymPy".to_string(),
        }
    }

    #[test]
    fn register_then_verify_pass() {
        let mut s = SaturonState::new();
        s.reduce(Action::RegisterHypothesis(hyp("H1")));
        assert_eq!(s.status_of("H1"), Some(HypothesisStatus::Proposed));
        s.reduce(Action::BeginVerification { id: "H1".into() });
        assert_eq!(s.status_of("H1"), Some(HypothesisStatus::Verifying));
        s.reduce(Action::ApplyResult {
            result: result("H1", true),
        });
        assert_eq!(s.status_of("H1"), Some(HypothesisStatus::Verified));
        assert_eq!(s.verified_count(), 1);
        assert_eq!(s.results.len(), 1);
    }

    #[test]
    fn failing_result_rejects() {
        let mut s = SaturonState::new();
        s.reduce(Action::RegisterHypothesis(hyp("H2")));
        s.reduce(Action::ApplyResult {
            result: result("H2", false),
        });
        assert_eq!(s.status_of("H2"), Some(HypothesisStatus::Rejected));
        assert_eq!(s.verified_count(), 0);
    }

    #[test]
    fn error_marks_errored() {
        let mut s = SaturonState::new();
        s.reduce(Action::RegisterHypothesis(hyp("H3")));
        s.reduce(Action::MarkErrored {
            id: "H3".into(),
            log: "boom".into(),
        });
        assert_eq!(s.status_of("H3"), Some(HypothesisStatus::Errored));
    }

    #[test]
    fn result_for_unknown_id_is_recorded_not_paniced() {
        let mut s = SaturonState::new();
        s.reduce(Action::ApplyResult {
            result: result("ghost", true),
        });
        assert_eq!(s.status_of("ghost"), None);
        assert_eq!(s.results.len(), 1);
        assert!(s.ledger.iter().any(|l| l.contains("unknown id ghost")));
    }

    #[test]
    fn reducer_is_deterministic() {
        let actions = || {
            vec![
                Action::RegisterHypothesis(hyp("A")),
                Action::RegisterHypothesis(hyp("B")),
                Action::BeginVerification { id: "A".into() },
                Action::ApplyResult {
                    result: result("A", true),
                },
                Action::ApplyResult {
                    result: result("B", false),
                },
            ]
        };
        let mut s1 = SaturonState::new();
        let mut s2 = SaturonState::new();
        for a in actions() {
            s1.reduce(a);
        }
        for a in actions() {
            s2.reduce(a);
        }
        assert_eq!(s1.ledger, s2.ledger);
        assert_eq!(s1.verified_count(), s2.verified_count());
    }
}
