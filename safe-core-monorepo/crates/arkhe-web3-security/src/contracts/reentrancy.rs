//! FI-W03 / FI-W04 — Checks-Effects-Interactions e guard de reentrância (SC08:2026).
//!
//! `∀ call: ¬(reentrant ∧ mutating_state)`
//!
//! Modela o padrão `nonReentrant` da OpenZeppelin: uma trava por escopo de
//! contrato que impede que uma chamada externa reentre antes do término da
//! chamada original (o vetor usado contra o GMX V1 em `executeDecreaseOrder`).

use crate::InvariantVerdict;

/// Guard de reentrância (equivalente ao `nonReentrant` modifier).
#[derive(Debug, Default)]
pub struct ReentrancyGuard {
    locked: bool,
}

/// Erro retornado quando uma chamada reentrante é detectada.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ReentrancyError {
    #[error("reentrant call detected while guard is locked")]
    Reentrant,
}

impl ReentrancyGuard {
    pub fn new() -> Self {
        Self::default()
    }

    /// Entra na seção crítica. Falha se já estiver travada (chamada reentrante).
    pub fn enter(&mut self) -> Result<(), ReentrancyError> {
        if self.locked {
            return Err(ReentrancyError::Reentrant);
        }
        self.locked = true;
        Ok(())
    }

    /// Sai da seção crítica.
    pub fn exit(&mut self) {
        self.locked = false;
    }

    /// FI-W04: executa `f` sob a guarda; qualquer chamada reentrante que
    /// tente `enter()` de dentro de `f` é rejeitada em tempo de execução.
    pub fn execute_guarded<T>(
        &mut self,
        f: impl FnOnce() -> T,
    ) -> Result<T, ReentrancyError> {
        self.enter()?;
        let result = f();
        self.exit();
        Ok(result)
    }
}

/// Uma operação dentro de uma transação simulada: um efeito local (escrita de
/// estado) ou uma chamada externa.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    /// Escrita de estado local (effect).
    Effect,
    /// Chamada externa (interaction).
    ExternalCall,
}

/// FI-W03: verifica o padrão Checks-Effects-Interactions — nenhuma escrita de
/// estado (`Effect`) pode ocorrer depois de uma `ExternalCall` na mesma
/// transação (isso é o que abre a janela de reentrância).
pub fn check_effects_interactions_order(ops: &[Op]) -> InvariantVerdict {
    let mut seen_external_call = false;
    for (idx, op) in ops.iter().enumerate() {
        match op {
            Op::ExternalCall => seen_external_call = true,
            Op::Effect if seen_external_call => {
                return InvariantVerdict::violated(format!(
                    "state effect at index {idx} occurs after an external call"
                ));
            }
            Op::Effect => {}
        }
    }
    InvariantVerdict::Holds
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reentrant_call_is_rejected() {
        let mut guard = ReentrancyGuard::new();
        guard.enter().unwrap();
        assert_eq!(guard.enter(), Err(ReentrancyError::Reentrant));
        guard.exit();
        assert!(guard.enter().is_ok());
    }

    #[test]
    fn execute_guarded_prevents_nested_reentry() {
        let mut guard = ReentrancyGuard::new();
        let result = guard.execute_guarded(|| 42);
        assert_eq!(result, Ok(42));
        // Guard is released after execution, so a subsequent call succeeds.
        assert!(guard.enter().is_ok());
    }

    #[test]
    fn effects_before_interactions_hold() {
        let ops = [Op::Effect, Op::Effect, Op::ExternalCall];
        assert!(check_effects_interactions_order(&ops).holds());
    }

    #[test]
    fn effect_after_external_call_violates_cei() {
        // Models the GMX V1 executeDecreaseOrder pattern: external call
        // (transfer to attacker-controlled address) before state is settled.
        let ops = [Op::ExternalCall, Op::Effect];
        assert!(!check_effects_interactions_order(&ops).holds());
    }
}
