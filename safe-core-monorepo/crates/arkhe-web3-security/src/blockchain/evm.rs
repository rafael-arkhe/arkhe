//! Arithmetic Vulnerabilities (SC07:2026) e limite de profundidade de call na EVM.
//!
//! `∀ op, result = safe_math(op)`

use crate::InvariantVerdict;

/// Erro de aritmética segura.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ArithmeticError {
    #[error("addition overflow: {0} + {1}")]
    AddOverflow(u128, u128),
    #[error("subtraction underflow: {0} - {1}")]
    SubUnderflow(u128, u128),
    #[error("multiplication overflow: {0} * {1}")]
    MulOverflow(u128, u128),
}

/// Adição verificada (substitui `a + b` sem wraparound silencioso).
pub fn safe_add(a: u128, b: u128) -> Result<u128, ArithmeticError> {
    a.checked_add(b).ok_or(ArithmeticError::AddOverflow(a, b))
}

/// Subtração verificada.
pub fn safe_sub(a: u128, b: u128) -> Result<u128, ArithmeticError> {
    a.checked_sub(b).ok_or(ArithmeticError::SubUnderflow(a, b))
}

/// Multiplicação verificada.
pub fn safe_mul(a: u128, b: u128) -> Result<u128, ArithmeticError> {
    a.checked_mul(b).ok_or(ArithmeticError::MulOverflow(a, b))
}

/// Limite de profundidade de chamadas recursivas na EVM (proteção de DoS por
/// stack exhaustion). O limite real da EVM é 1024.
pub const MAX_CALL_DEPTH: u32 = 1024;

/// FI: verifica se a profundidade de chamada atual respeita o limite.
pub fn check_call_depth(depth: u32) -> InvariantVerdict {
    if depth < MAX_CALL_DEPTH {
        InvariantVerdict::Holds
    } else {
        InvariantVerdict::violated(format!("call depth {depth} >= max {MAX_CALL_DEPTH}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_add_overflow_is_rejected() {
        assert_eq!(safe_add(u128::MAX, 1), Err(ArithmeticError::AddOverflow(u128::MAX, 1)));
    }

    #[test]
    fn safe_sub_underflow_is_rejected() {
        assert_eq!(safe_sub(0, 1), Err(ArithmeticError::SubUnderflow(0, 1)));
    }

    #[test]
    fn safe_mul_overflow_is_rejected() {
        assert_eq!(safe_mul(u128::MAX, 2), Err(ArithmeticError::MulOverflow(u128::MAX, 2)));
    }

    #[test]
    fn normal_arithmetic_succeeds() {
        assert_eq!(safe_add(2, 3), Ok(5));
        assert_eq!(safe_sub(5, 3), Ok(2));
        assert_eq!(safe_mul(2, 3), Ok(6));
    }

    #[test]
    fn call_depth_limit_enforced() {
        assert!(check_call_depth(1023).holds());
        assert!(!check_call_depth(1024).holds());
    }
}
