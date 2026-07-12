//! Approval phishing — aprovações de gasto ilimitado (unlimited allowance).
//!
//! `∀ approval, approval_amount ≤ needed_amount`

use crate::InvariantVerdict;

/// FI: uma aprovação de token só é segura se o valor concedido não excede o
/// valor efetivamente necessário para a operação (evita "approve max" que
/// deixa o gastador livre para drenar o saldo inteiro depois).
pub fn check_approval_amount(approval_amount: u128, needed_amount: u128) -> InvariantVerdict {
    if approval_amount <= needed_amount {
        InvariantVerdict::Holds
    } else {
        InvariantVerdict::violated(format!(
            "approval {approval_amount} exceeds needed amount {needed_amount}"
        ))
    }
}

/// `u128::MAX` é o padrão usado por wallets para "aprovação infinita" — o
/// alvo clássico de approval phishing.
pub const UNLIMITED_ALLOWANCE: u128 = u128::MAX;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_needed_amount_holds() {
        assert!(check_approval_amount(100, 100).holds());
    }

    #[test]
    fn minimal_approval_holds() {
        assert!(check_approval_amount(50, 100).holds());
    }

    #[test]
    fn unlimited_allowance_is_rejected() {
        assert!(!check_approval_amount(UNLIMITED_ALLOWANCE, 100).holds());
    }
}
