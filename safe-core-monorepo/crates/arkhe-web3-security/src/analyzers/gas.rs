//! Denial of Service via consumo de gas (unbounded loops, gas griefing).

use crate::InvariantVerdict;

/// FI: uma operação estimada não pode exceder o gas limit do bloco, e deve
/// deixar uma margem de segurança (`reserve_bps`) para outras transações.
pub fn check_gas_within_block_limit(estimated_gas: u64, block_gas_limit: u64, reserve_bps: u32) -> InvariantVerdict {
    let usable_limit = block_gas_limit - (block_gas_limit as u128 * reserve_bps as u128 / 10_000) as u64;
    if estimated_gas <= usable_limit {
        InvariantVerdict::Holds
    } else {
        InvariantVerdict::violated(format!(
            "estimated gas {estimated_gas} exceeds usable block limit {usable_limit}"
        ))
    }
}

/// FI: detecta loops cujo número de iterações depende de uma coleção
/// controlada por um usuário sem limite superior — o vetor clássico de
/// unbounded-loop DoS (ex: array de "todos os investidores" que só cresce).
pub fn check_bounded_iteration(collection_len: usize, max_allowed: usize) -> InvariantVerdict {
    if collection_len <= max_allowed {
        InvariantVerdict::Holds
    } else {
        InvariantVerdict::violated(format!(
            "iteration over {collection_len} elements exceeds bounded max {max_allowed}"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gas_within_limit_holds() {
        assert!(check_gas_within_block_limit(20_000_000, 30_000_000, 1_000).holds());
    }

    #[test]
    fn gas_exceeding_reserve_is_rejected() {
        assert!(!check_gas_within_block_limit(29_900_000, 30_000_000, 1_000).holds());
    }

    #[test]
    fn bounded_iteration_holds() {
        assert!(check_bounded_iteration(50, 100).holds());
    }

    #[test]
    fn unbounded_growth_is_rejected() {
        assert!(!check_bounded_iteration(10_000, 100).holds());
    }
}
