//! Consenso: 51% attack e Sybil attack.
//!
//! `honest_validators ≥ ⅔ * total_validators`

use crate::InvariantVerdict;

/// FI: verifica se a fração de validadores honestos mantém segurança BFT (≥ 2/3).
pub fn check_honest_majority(honest_validators: u64, total_validators: u64) -> InvariantVerdict {
    if total_validators == 0 {
        return InvariantVerdict::violated("no validators registered");
    }
    // Compare as honest * 3 >= total * 2 to avoid floating point.
    if honest_validators.saturating_mul(3) >= total_validators.saturating_mul(2) {
        InvariantVerdict::Holds
    } else {
        InvariantVerdict::violated(format!(
            "honest validators {honest_validators}/{total_validators} below 2/3 BFT threshold"
        ))
    }
}

/// Sybil attack: exige que cada identidade de validador seja distinta e verificada.
pub fn check_no_sybil(verified_identities: &[String]) -> InvariantVerdict {
    let mut seen = std::collections::HashSet::new();
    for id in verified_identities {
        if !seen.insert(id) {
            return InvariantVerdict::violated(format!("duplicate validator identity: {id}"));
        }
    }
    InvariantVerdict::Holds
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_thirds_honest_holds() {
        assert!(check_honest_majority(67, 100).holds());
    }

    #[test]
    fn below_two_thirds_violates() {
        assert!(!check_honest_majority(60, 100).holds());
    }

    #[test]
    fn duplicate_identity_is_sybil() {
        let ids = vec!["node-a".to_string(), "node-b".to_string(), "node-a".to_string()];
        assert!(!check_no_sybil(&ids).holds());
    }

    #[test]
    fn distinct_identities_hold() {
        let ids = vec!["node-a".to_string(), "node-b".to_string()];
        assert!(check_no_sybil(&ids).holds());
    }
}
