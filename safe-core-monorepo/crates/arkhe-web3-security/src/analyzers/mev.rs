//! FI-W10b — Detecção de sandwich attacks (MEV).
//!
//! `∀ block: ¬∃ (tx_before, tx_target, tx_after):
//!    tx_before.sender == tx_after.sender ∧ tx_before.swap.token == tx_target.swap.token`

use crate::InvariantVerdict;

/// Uma transação de swap dentro de um bloco.
#[derive(Debug, Clone)]
pub struct SwapTx {
    pub sender: String,
    pub token: String,
}

/// FI-W10b: varre um bloco (lista ordenada de swaps) procurando o padrão
/// sandwich — mesma conta antes e depois de uma transação de terceiro no
/// mesmo par de token.
pub fn check_no_sandwich(block: &[SwapTx]) -> InvariantVerdict {
    for i in 0..block.len() {
        for j in (i + 2)..block.len() {
            let before = &block[i];
            let after = &block[j];
            if before.sender != after.sender {
                continue;
            }
            for target in &block[i + 1..j] {
                if target.sender != before.sender && target.token == before.token {
                    return InvariantVerdict::violated(format!(
                        "sandwich detected: '{}' wraps '{}' on token '{}'",
                        before.sender, target.sender, before.token
                    ));
                }
            }
        }
    }
    InvariantVerdict::Holds
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn independent_swaps_hold() {
        let block = vec![
            SwapTx { sender: "alice".into(), token: "ETH".into() },
            SwapTx { sender: "bob".into(), token: "ETH".into() },
            SwapTx { sender: "carol".into(), token: "ETH".into() },
        ];
        assert!(check_no_sandwich(&block).holds());
    }

    #[test]
    fn sandwich_pattern_is_detected() {
        let block = vec![
            SwapTx { sender: "mev_bot".into(), token: "ETH".into() },
            SwapTx { sender: "victim".into(), token: "ETH".into() },
            SwapTx { sender: "mev_bot".into(), token: "ETH".into() },
        ];
        assert!(!check_no_sandwich(&block).holds());
    }

    #[test]
    fn same_sender_different_token_is_not_sandwich() {
        let block = vec![
            SwapTx { sender: "alice".into(), token: "ETH".into() },
            SwapTx { sender: "victim".into(), token: "USDC".into() },
            SwapTx { sender: "alice".into(), token: "ETH".into() },
        ];
        assert!(check_no_sandwich(&block).holds());
    }
}
