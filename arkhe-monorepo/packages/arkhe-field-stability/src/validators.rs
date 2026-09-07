//! SemanticValidity — eixo de garantia da Fase 7 (parecer v381.0, reancorado
//! em v381.1, bloco 1005).
//!
//! A SemanticValidity mede a fração de validadores independentes que aprovam
//! uma entrada. É um **eixo de relatório ortogonal a Φ**: não compõe a fórmula
//! quadrática canônica (que permanece `Ω=stability`, `Σ=success_rate`,
//! `Λ=latency_score` em [`crate::coherence`] — medição física, provada em
//! I511–I516). A sanção semântica exige **quórum estrito**: aprovação de mais
//! de 2/3 dos validadores, com conjunto mínimo de [`MIN_VALIDATORS`].
//!
//! ## Invariantes
//!
//! * **Gap-3 — consistência dimensional:** quórum `> 2/3` exige que a fração
//!   aprovada seja *estritamente* maior que `VALIDATOR_QUORUM` — `2/3` exato
//!   não sanciona.
//! * **Loopseal-3 — audit trail:** cada validador carrega `id()` e a
//!   contagem `approved/total` é reportada explicitamente.

use serde::{Deserialize, Serialize};

use crate::ledger::CoherenceEntry;

/// Quórum estrito para aprovação semântica (2/3).
pub const VALIDATOR_QUORUM: f64 = 2.0 / 3.0;

/// Número mínimo de validadores para um conjunto sancionável.
pub const MIN_VALIDATORS: usize = 3;

/// Veredito individual de um validador sobre uma entrada.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Verdict {
    /// A entrada é semanticamente válida.
    Approve,
    /// A entrada é semanticamente rejeitada.
    Reject,
}

/// Um validador independente de SemanticValidity.
pub trait Validator {
    /// Identificador do validador (audit trail, Loopseal-3).
    fn id(&self) -> &str;
    /// Decide sobre uma entrada da cadeia de dados.
    fn verify(&self, entry: &CoherenceEntry) -> Verdict;
}

/// Agregação da SemanticValidity de um conjunto de validadores.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SemanticValidity {
    /// Número de validadores que aprovaram.
    pub approved: usize,
    /// Número total de validadores no conjunto.
    pub total: usize,
    /// Fração aprovada `approved / total` (0.0 se `total == 0`).
    pub ratio: f64,
}

impl SemanticValidity {
    /// Constrói a agregação a partir da contagem de aprovações e do total.
    #[must_use]
    pub fn new(approved: usize, total: usize) -> Self {
        let ratio = if total == 0 {
            0.0
        } else {
            approved as f64 / total as f64
        };
        Self {
            approved,
            total,
            ratio,
        }
    }

    /// Sancionada somente com conjunto mínimo e quórum **estrito**
    /// (`ratio > 2/3`). `2/3` exato não sanciona (Gap-3).
    #[must_use]
    pub fn is_satisfied(&self) -> bool {
        self.total >= MIN_VALIDATORS && self.ratio > VALIDATOR_QUORUM
    }
}

/// Erro na agregação do conjunto de validadores.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidatorSetError {
    /// Conjunto com menos que [`MIN_VALIDATORS`] validadores — não sancionável.
    TooFewValidators {
        /// Número de validadores efetivamente fornecido.
        actual: usize,
        /// Mínimo exigido ([`MIN_VALIDATORS`]).
        minimum: usize,
    },
}

/// Agrega as decisões de um conjunto de validadores sobre uma entrada.
///
/// Falha com [`ValidatorSetError::TooFewValidators`] se
/// `validators.len() < MIN_VALIDATORS`. Cada veredito `Approve` soma uma
/// aprovação; a [`SemanticValidity`] resultante carrega `approved`, `total` e
/// `ratio` para o relatório (eixo ortogonal, não componente de Φ).
pub fn aggregate_validity(
    validators: &[&dyn Validator],
    entry: &CoherenceEntry,
) -> Result<SemanticValidity, ValidatorSetError> {
    if validators.len() < MIN_VALIDATORS {
        return Err(ValidatorSetError::TooFewValidators {
            actual: validators.len(),
            minimum: MIN_VALIDATORS,
        });
    }
    let approved = validators
        .iter()
        .filter(|v| v.verify(entry) == Verdict::Approve)
        .count();
    Ok(SemanticValidity::new(approved, validators.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::ledger::{CoherenceEntry, GENESIS};

    fn entry(phi: f64, integrity_ok: bool) -> CoherenceEntry {
        let mut e = CoherenceEntry::from_parts(0, 1, phi, 0.9, 0.95, 0.8, GENESIS);
        if !integrity_ok {
            e.hash = "TAMPERED".to_string();
        }
        e
    }

    struct Approver;
    impl Validator for Approver {
        fn id(&self) -> &str {
            "approver"
        }
        fn verify(&self, _entry: &CoherenceEntry) -> Verdict {
            Verdict::Approve
        }
    }

    struct PhiGate {
        min_phi: f64,
    }
    impl Validator for PhiGate {
        fn id(&self) -> &str {
            "phi_gate"
        }
        fn verify(&self, entry: &CoherenceEntry) -> Verdict {
            if entry.phi >= self.min_phi {
                Verdict::Approve
            } else {
                Verdict::Reject
            }
        }
    }

    struct IntegrityChecker;
    impl Validator for IntegrityChecker {
        fn id(&self) -> &str {
            "integrity_checker"
        }
        fn verify(&self, entry: &CoherenceEntry) -> Verdict {
            if entry.hash == entry.compute_hash() {
                Verdict::Approve
            } else {
                Verdict::Reject
            }
        }
    }

    fn entry_ok() -> CoherenceEntry {
        entry(0.96, true)
    }

    #[test]
    fn three_validators_all_approve_satisfies() {
        let e = entry_ok();
        let set: [&dyn Validator; 3] = [&Approver, &PhiGate { min_phi: 0.9 }, &IntegrityChecker];
        let sv = aggregate_validity(&set, &e).unwrap();
        assert_eq!(sv.approved, 3);
        assert_eq!(sv.total, 3);
        assert!((sv.ratio - 1.0).abs() < 1e-12);
        assert!(sv.is_satisfied());
    }

    #[test]
    fn two_of_three_is_not_quorum_strict() {
        // 2/3 exato NÃO é > 2/3 (Gap-3: quórum estrito).
        let e = entry_ok();
        let set: [&dyn Validator; 3] = [&Approver, &Approver, &PhiGate { min_phi: 0.99 }];
        let sv = aggregate_validity(&set, &e).unwrap();
        assert_eq!(sv.approved, 2);
        assert!((sv.ratio - 2.0 / 3.0).abs() < 1e-12);
        assert!(!sv.is_satisfied(), "2/3 exato não deve sancionar");
    }

    #[test]
    fn content_tamper_is_rejected_by_integrity_checker() {
        let e = entry(0.96, false);
        let set: [&dyn Validator; 3] = [&Approver, &Approver, &IntegrityChecker];
        let sv = aggregate_validity(&set, &e).unwrap();
        assert_eq!(sv.approved, 2);
        assert!(!sv.is_satisfied());
    }

    #[test]
    fn nine_members_seven_approve_satisfies() {
        let e = entry_ok();
        let set: Vec<&dyn Validator> = std::iter::repeat_n(&Approver as &dyn Validator, 7)
            .chain(std::iter::repeat_n(&PhiGate { min_phi: 0.99 } as &dyn Validator, 2))
            .collect();
        let sv = aggregate_validity(&set, &e).unwrap();
        assert_eq!(sv.approved, 7);
        assert!((sv.ratio - 7.0 / 9.0).abs() < 1e-12);
        assert!(sv.is_satisfied(), "7/9 > 2/3 deve sancionar");
    }

    #[test]
    fn six_of_nine_is_boundary_not_satisfied() {
        let e = entry_ok();
        let set: Vec<&dyn Validator> = std::iter::repeat_n(&Approver as &dyn Validator, 6)
            .chain(std::iter::repeat_n(&PhiGate { min_phi: 0.99 } as &dyn Validator, 3))
            .collect();
        let sv = aggregate_validity(&set, &e).unwrap();
        assert_eq!(sv.approved, 6);
        assert!((sv.ratio - 6.0 / 9.0).abs() < 1e-12);
        assert!(!sv.is_satisfied(), "6/9 = 2/3 exato não sanciona");
    }

    #[test]
    fn too_few_validators_errors() {
        let e = entry_ok();
        let set: [&dyn Validator; 2] = [&Approver, &Approver];
        assert_eq!(
            aggregate_validity(&set, &e),
            Err(ValidatorSetError::TooFewValidators {
                actual: 2,
                minimum: MIN_VALIDATORS
            })
        );
    }

    #[test]
    fn empty_set_errors_and_ratio_guarded() {
        let e = entry_ok();
        let set: [&dyn Validator; 0] = [];
        assert_eq!(
            aggregate_validity(&set, &e),
            Err(ValidatorSetError::TooFewValidators {
                actual: 0,
                minimum: MIN_VALIDATORS
            })
        );
        let sv = SemanticValidity::new(0, 0);
        assert_eq!(sv.ratio, 0.0);
        assert!(!sv.is_satisfied());
    }

    #[test]
    fn all_reject_is_unsatisfied() {
        let e = entry(0.1, false);
        let set: [&dyn Validator; 3] = [&PhiGate { min_phi: 0.9 }; 3];
        let sv = aggregate_validity(&set, &e).unwrap();
        assert_eq!(sv.approved, 0);
        assert_eq!(sv.ratio, 0.0);
        assert!(!sv.is_satisfied());
    }

    #[test]
    fn property_ratio_monotone_in_approved_count() {
        // Propriedade (7.4): para total fixo, `ratio` cresce monotonicamente com
        // o número de aprovações e `is_satisfied` só dispara acima de 2/3.
        let total = 9;
        let mut prev_ratio = -1.0f64;
        for approved in 0..=total {
            let sv = SemanticValidity::new(approved, total);
            assert!(sv.ratio >= prev_ratio, "ratio deve ser não-decrescente");
            prev_ratio = sv.ratio;
            let strict_above_quorum = (approved as f64 / total as f64) > VALIDATOR_QUORUM;
            assert_eq!(sv.is_satisfied(), strict_above_quorum, "approved={approved}");
        }
    }

    #[test]
    fn property_quorum_boundary_extended_sets() {
        // Propriedade (7.4): para várias dimensões de conjunto, `is_satisfied`
        // é verdadeiro sse `approved/total > 2/3` e `total >= MIN_VALIDATORS`.
        let totals = [MIN_VALIDATORS, 4, 6, 100];
        for total in totals {
            for approved in 0..=total {
                let sv = SemanticValidity::new(approved, total);
                let expected = (approved as f64 / total as f64) > VALIDATOR_QUORUM;
                assert_eq!(sv.is_satisfied(), expected, "approved={approved}/{total}");
            }
        }
    }
}