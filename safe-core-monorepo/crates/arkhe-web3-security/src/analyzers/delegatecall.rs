//! Analisador: `delegatecall` para endereços fora da allowlist.
//!
//! `delegatecall` executa código externo no contexto de storage do chamador
//! — um alvo não confiável pode sobrescrever qualquer slot de storage
//! (o vetor usado no hack da Parity Multisig Wallet, 2017).

use crate::InvariantVerdict;

/// FI: todo `delegatecall` deve apontar para um endereço na allowlist de
/// implementações confiáveis (ex: apenas a implementação de proxy atual).
pub fn check_delegatecall_target(target: &str, allowed_targets: &[&str]) -> InvariantVerdict {
    if allowed_targets.contains(&target) {
        InvariantVerdict::Holds
    } else {
        InvariantVerdict::violated(format!(
            "delegatecall target '{target}' is not in the allowed implementation list"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delegatecall_to_current_implementation_holds() {
        let verdict = check_delegatecall_target("impl_v2", &["impl_v1", "impl_v2"]);
        assert!(verdict.holds());
    }

    #[test]
    fn delegatecall_to_attacker_contract_is_rejected() {
        let verdict = check_delegatecall_target("attacker_lib", &["impl_v1", "impl_v2"]);
        assert!(!verdict.holds());
    }
}
