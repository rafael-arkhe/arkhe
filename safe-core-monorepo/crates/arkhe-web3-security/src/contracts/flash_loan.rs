//! Flash-loan-facilitated attacks (SC04:2026).
//!
//! `state_after(flash_loan) ⇒ valid_state`
//!
//! A flash loan itself is not the vulnerability — it is a large, uncollateralized
//! amplifier for a pre-existing invariant break elsewhere in the protocol. This
//! module checks that the invariant the protocol depends on (e.g. total
//! collateral ≥ total debt) still holds once the loan is repaid within the
//! same transaction.

use crate::InvariantVerdict;

/// Estado do protocolo relevante para um invariante de solvência.
#[derive(Debug, Clone, Copy)]
pub struct PoolState {
    pub total_collateral: u128,
    pub total_debt: u128,
}

impl PoolState {
    /// Invariante de solvência do pool: colateral nunca pode ficar abaixo da dívida.
    pub fn is_solvent(&self) -> bool {
        self.total_collateral >= self.total_debt
    }
}

/// Simula um empréstimo relâmpago: `amount` é emprestado e deve ser
/// devolvido (com `fee`) antes do fim da transação. Retorna o estado
/// resultante e verifica que o invariante de solvência se mantém.
///
/// `state_after(flash_loan) ⇒ valid_state` — a checagem ocorre *depois* do
/// reembolso, simulando a atomicidade de uma transação EVM.
pub fn check_flash_loan_settlement(
    before: PoolState,
    amount: u128,
    fee: u128,
    repaid: u128,
) -> InvariantVerdict {
    if repaid < amount + fee {
        return InvariantVerdict::violated(format!(
            "flash loan under-repaid: repaid={repaid} < amount+fee={}",
            amount + fee
        ));
    }

    let after = PoolState {
        total_collateral: before.total_collateral + fee,
        total_debt: before.total_debt,
    };

    if !after.is_solvent() {
        return InvariantVerdict::violated(
            "pool insolvent after flash loan settlement".to_string(),
        );
    }

    InvariantVerdict::Holds
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fully_repaid_loan_keeps_pool_solvent() {
        let before = PoolState { total_collateral: 1_000, total_debt: 500 };
        let verdict = check_flash_loan_settlement(before, 10_000, 10, 10_010);
        assert!(verdict.holds());
    }

    #[test]
    fn under_repaid_loan_is_rejected() {
        let before = PoolState { total_collateral: 1_000, total_debt: 500 };
        // Attacker repays less than owed — the classic flash-loan drain shape.
        let verdict = check_flash_loan_settlement(before, 10_000, 10, 9_000);
        assert!(!verdict.holds());
    }
}
