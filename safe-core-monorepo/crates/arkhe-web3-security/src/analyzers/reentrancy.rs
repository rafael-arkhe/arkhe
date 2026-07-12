//! Analisador estático: detecta ciclos de reentrância num grafo de chamadas.
//!
//! Diferente de `contracts::reentrancy` (guard em tempo de execução), este
//! módulo analisa um traço de chamadas *já registrado* (ex: de um simulador
//! ou de um trace on-chain) para sinalizar contratos que se chamam de volta
//! antes de retornar — o padrão de ataque explorado no GMX V1.

use crate::InvariantVerdict;

/// Um passo de chamada num traço de execução de transação.
#[derive(Debug, Clone)]
pub struct CallStep {
    pub caller: String,
    pub callee: String,
}

/// Analisa uma sequência de chamadas (ordem de entrada) e detecta se algum
/// contrato aparece como `callee` de uma chamada enquanto ainda está "aberto"
/// (isto é, apareceu como `caller` numa chamada anterior não fechada).
///
/// O traço é representado como pilha de chamadas abertas: cada `CallStep`
/// empilha `callee`; assumimos que o chamador está fechando quando reaparece
/// como `callee` de alguém mais fundo na pilha — isso é reentrância.
pub fn detect_reentrant_cycle(trace: &[CallStep]) -> InvariantVerdict {
    let mut open_stack: Vec<&str> = Vec::new();
    for step in trace {
        if open_stack.is_empty() {
            open_stack.push(step.caller.as_str());
        }
        if open_stack.contains(&step.callee.as_str()) {
            return InvariantVerdict::violated(format!(
                "reentrant call detected: '{}' re-entered while already on the call stack",
                step.callee
            ));
        }
        open_stack.push(step.callee.as_str());
    }
    InvariantVerdict::Holds
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_call_chain_holds() {
        let trace = vec![
            CallStep { caller: "user".into(), callee: "vault".into() },
            CallStep { caller: "vault".into(), callee: "token".into() },
        ];
        assert!(detect_reentrant_cycle(&trace).holds());
    }

    #[test]
    fn reentrant_call_back_into_vault_is_flagged() {
        // vault -> attacker -> vault (re-entry), mirrors executeDecreaseOrder.
        let trace = vec![
            CallStep { caller: "user".into(), callee: "vault".into() },
            CallStep { caller: "vault".into(), callee: "attacker".into() },
            CallStep { caller: "attacker".into(), callee: "vault".into() },
        ];
        assert!(!detect_reentrant_cycle(&trace).holds());
    }
}
