//! Price Oracle Manipulation (SC03:2026).
//!
//! `price_deviation ≤ threshold`

use crate::InvariantVerdict;

/// FI: compara um preço spot (potencialmente manipulável dentro de uma única
/// transação, ex: via flash loan num pool de baixa liquidez) contra um preço
/// de referência (TWAP, oráculo agregado) e rejeita se o desvio relativo
/// excede o limiar configurado (em basis points).
pub fn check_price_deviation(spot_price: u128, reference_price: u128, max_deviation_bps: u32) -> InvariantVerdict {
    if reference_price == 0 {
        return InvariantVerdict::violated("reference price is zero");
    }
    let diff = spot_price.abs_diff(reference_price);
    let deviation_bps = diff.saturating_mul(10_000) / reference_price;
    if deviation_bps <= max_deviation_bps as u128 {
        InvariantVerdict::Holds
    } else {
        InvariantVerdict::violated(format!(
            "price deviation {deviation_bps}bps exceeds max {max_deviation_bps}bps \
             (spot={spot_price}, reference={reference_price})"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_deviation_holds() {
        // 1% deviation, 500bps threshold (5%).
        assert!(check_price_deviation(1_010, 1_000, 500).holds());
    }

    #[test]
    fn manipulated_spot_price_is_rejected() {
        // Spot price doubled relative to TWAP — classic single-block manipulation.
        assert!(!check_price_deviation(2_000, 1_000, 500).holds());
    }
}
