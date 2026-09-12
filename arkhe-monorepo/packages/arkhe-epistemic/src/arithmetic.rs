//! Verificação aritmética de `a(p) ≠ p³ − p²` para `p` primo (A051193).

use num_bigint::BigUint;
use num_traits::One;

/// Sequência A051193: `a(n) = n² + 1`.
///
/// Para `p` primo ímpar, `a(p) = p² + 1` nunca é igual a `p³ − p²`.
pub struct A051193;

impl A051193 {
    /// Calcula `a(n) = n² + 1`.
    pub fn a(n: u64) -> BigUint {
        let n = BigUint::from(n);
        &n * &n + BigUint::one()
    }
}

/// Calcula `a(n)` para um `n` genérico.
pub fn a(n: u64) -> BigUint {
    A051193::a(n)
}

/// Verifica se `p` é primo (teste determinístico simples até 2⁶⁴).
pub fn is_prime(n: u64) -> bool {
    if n < 2 {
        return false;
    }
    if n == 2 || n == 3 {
        return true;
    }
    if n.is_multiple_of(2) || n.is_multiple_of(3) {
        return false;
    }
    let mut i = 5;
    while i * i <= n {
        if n.is_multiple_of(i) || n.is_multiple_of(i + 2) {
            return false;
        }
        i += 6;
    }
    true
}

/// Verifica `a(p) ≠ p³ − p²` para um primo `p` dado.
pub fn verify_a_neq_p3_minus_p2(p: u64) -> bool {
    if !is_prime(p) {
        return false;
    }
    let a_p = a(p);
    let p_big = BigUint::from(p);
    let p3 = &p_big * &p_big * &p_big;
    let p2 = &p_big * &p_big;
    let rhs = &p3 - &p2;
    a_p != rhs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a051193_primeiras_entradas() {
        assert_eq!(a(1), BigUint::from(2u32));
        assert_eq!(a(2), BigUint::from(5u32));
        assert_eq!(a(3), BigUint::from(10u32));
        assert_eq!(a(4), BigUint::from(17u32));
        assert_eq!(a(5), BigUint::from(26u32));
    }

    #[test]
    fn primos_verificam_a_neq_p3_minus_p2() {
        for p in [3u64, 5, 7, 11, 13, 17, 19, 23, 29, 31] {
            assert!(verify_a_neq_p3_minus_p2(p), "falhou para p={}", p);
        }
    }

    #[test]
    fn nao_primo_retorna_false() {
        assert!(!verify_a_neq_p3_minus_p2(4));
        assert!(!verify_a_neq_p3_minus_p2(1));
        assert!(!verify_a_neq_p3_minus_p2(0));
        assert!(!verify_a_neq_p3_minus_p2(9));
    }

    #[test]
    fn is_prime_casos_basicos() {
        assert!(!is_prime(0));
        assert!(!is_prime(1));
        assert!(is_prime(2));
        assert!(is_prime(3));
        assert!(!is_prime(4));
        assert!(is_prime(5));
        assert!(is_prime(7));
        assert!(!is_prime(9));
        assert!(is_prime(11));
    }

    #[test]
    fn divergencia_em_n3() {
        let a3 = a(3);
        let p = BigUint::from(3u32);
        let p3_minus_p2 = &p * &p * &p - &p * &p;
        assert_ne!(a3, p3_minus_p2);
        assert_eq!(a3, BigUint::from(10u32));
        assert_eq!(p3_minus_p2, BigUint::from(18u32));
    }
}
