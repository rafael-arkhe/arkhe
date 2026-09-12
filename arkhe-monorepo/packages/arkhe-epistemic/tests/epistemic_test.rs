use arkhe_epistemic::{a, verify_a_neq_p3_minus_p2};

#[test]
fn test_a051193() {
    assert_eq!(a(1), num_bigint::BigUint::from(2u32));
    assert_eq!(a(2), num_bigint::BigUint::from(5u32));
    assert_eq!(a(3), num_bigint::BigUint::from(10u32));
}

#[test]
fn test_verify_a_neq_p3_minus_p2() {
    for p in [3u64, 5, 7, 11, 13, 17, 19] {
        assert!(verify_a_neq_p3_minus_p2(p), "falhou para p={}", p);
    }
}

#[test]
fn test_non_prime_returns_false() {
    assert!(!verify_a_neq_p3_minus_p2(4));
    assert!(!verify_a_neq_p3_minus_p2(1));
}
