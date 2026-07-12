//! Analisador: detecta o formato "borrow → manipulate → repay" de um flash loan
//! dentro de um único traço de transação e sinaliza se o valor devolvido é
//! insuficiente antes do fim do escopo atômico.

use crate::InvariantVerdict;

/// Um evento dentro do traço de uma transação.
#[derive(Debug, Clone)]
pub enum TraceEvent {
    Borrow { amount: u128 },
    Repay { amount: u128 },
}

/// FI: soma todos os `Borrow` e `Repay` no traço; a transação só é segura se
/// o total devolvido cobre o total emprestado antes do fim do traço (o traço
/// representa uma transação atômica — se não fechar aqui, a EVM reverte).
pub fn check_flash_loan_trace(trace: &[TraceEvent]) -> InvariantVerdict {
    let mut borrowed: u128 = 0;
    let mut repaid: u128 = 0;
    for event in trace {
        match event {
            TraceEvent::Borrow { amount } => borrowed += amount,
            TraceEvent::Repay { amount } => repaid += amount,
        }
    }
    if repaid >= borrowed {
        InvariantVerdict::Holds
    } else {
        InvariantVerdict::violated(format!(
            "flash loan trace under-repaid: borrowed={borrowed}, repaid={repaid}"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fully_repaid_trace_holds() {
        let trace = vec![
            TraceEvent::Borrow { amount: 1_000_000 },
            TraceEvent::Repay { amount: 1_000_100 },
        ];
        assert!(check_flash_loan_trace(&trace).holds());
    }

    #[test]
    fn drained_trace_is_flagged() {
        let trace = vec![
            TraceEvent::Borrow { amount: 1_000_000 },
            TraceEvent::Repay { amount: 100 },
        ];
        assert!(!check_flash_loan_trace(&trace).holds());
    }
}
