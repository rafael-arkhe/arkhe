//! Relatório de conformidade contra o OWASP Smart Contract Top 10 2026.

use crate::InvariantVerdict;
use serde::{Deserialize, Serialize};

/// Categoria do OWASP Smart Contract Top 10 2026.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OwaspCategory {
    SC01AccessControl,
    SC02BusinessLogic,
    SC03PriceOracle,
    SC04FlashLoan,
    SC05InputValidation,
    SC06UncheckedExternalCalls,
    SC07Arithmetic,
    SC08Reentrancy,
    SC09TransactionOrdering,
    SC10ProxyUpgradeability,
}

impl OwaspCategory {
    /// Identificador curto usado no catálogo YAML (`SC01`..`SC10`).
    pub fn id(&self) -> &'static str {
        match self {
            OwaspCategory::SC01AccessControl => "SC01",
            OwaspCategory::SC02BusinessLogic => "SC02",
            OwaspCategory::SC03PriceOracle => "SC03",
            OwaspCategory::SC04FlashLoan => "SC04",
            OwaspCategory::SC05InputValidation => "SC05",
            OwaspCategory::SC06UncheckedExternalCalls => "SC06",
            OwaspCategory::SC07Arithmetic => "SC07",
            OwaspCategory::SC08Reentrancy => "SC08",
            OwaspCategory::SC09TransactionOrdering => "SC09",
            OwaspCategory::SC10ProxyUpgradeability => "SC10",
        }
    }
}

/// Um achado de auditoria associado a uma categoria OWASP.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OwaspFinding {
    pub category: OwaspCategory,
    pub target: String,
    pub verdict: InvariantVerdict,
}

/// Relatório agregado: total de achados por categoria e taxa de conformidade.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OwaspReport {
    pub total_checks: usize,
    pub violations: Vec<OwaspFinding>,
}

impl OwaspReport {
    /// Constrói o relatório a partir de uma lista de achados, mantendo
    /// apenas as violações (achados que respeitam o invariante não entram
    /// no relatório de risco).
    pub fn from_findings(findings: Vec<OwaspFinding>) -> Self {
        let total_checks = findings.len();
        let violations = findings.into_iter().filter(|f| !f.verdict.holds()).collect();
        Self { total_checks, violations }
    }

    /// Taxa de conformidade (0.0 a 1.0).
    pub fn compliance_rate(&self) -> f64 {
        if self.total_checks == 0 {
            return 1.0;
        }
        1.0 - (self.violations.len() as f64 / self.total_checks as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_filters_only_violations() {
        let findings = vec![
            OwaspFinding {
                category: OwaspCategory::SC08Reentrancy,
                target: "Vault.withdraw".into(),
                verdict: InvariantVerdict::Holds,
            },
            OwaspFinding {
                category: OwaspCategory::SC01AccessControl,
                target: "Proxy.upgrade".into(),
                verdict: InvariantVerdict::violated("missing onlyAdmin"),
            },
        ];
        let report = OwaspReport::from_findings(findings);
        assert_eq!(report.total_checks, 2);
        assert_eq!(report.violations.len(), 1);
        assert_eq!(report.violations[0].category.id(), "SC01");
    }

    #[test]
    fn compliance_rate_is_computed_correctly() {
        let findings = vec![
            OwaspFinding {
                category: OwaspCategory::SC08Reentrancy,
                target: "a".into(),
                verdict: InvariantVerdict::Holds,
            },
            OwaspFinding {
                category: OwaspCategory::SC08Reentrancy,
                target: "b".into(),
                verdict: InvariantVerdict::Holds,
            },
            OwaspFinding {
                category: OwaspCategory::SC08Reentrancy,
                target: "c".into(),
                verdict: InvariantVerdict::violated("guard missing"),
            },
        ];
        let report = OwaspReport::from_findings(findings);
        assert!((report.compliance_rate() - (2.0 / 3.0)).abs() < 1e-9);
    }
}
