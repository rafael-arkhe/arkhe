//! Segurança PQ de hash (preimage n/2, collision n/3) — 3 casos.

use arkhe_crypto::HashSecurity;

#[test]
fn pq_bits_preimage_256_epsilon() {
    let got = HashSecurity::Preimage.pq_bits(256);
    assert!((got - 128.0).abs() < f64::EPSILON, "expected 128.0, got {got}");
}

#[test]
fn pq_bits_preimage_256_bit_exact() {
    assert_eq!(
        HashSecurity::Preimage.pq_bits(256).to_bits(),
        128.0_f64.to_bits()
    );
}

#[test]
fn pq_bits_collision_256() {
    let got = HashSecurity::Collision.pq_bits(256);
    let expected = 256.0 / 3.0;
    assert!((got - expected).abs() < f64::EPSILON, "expected {expected}, got {got}");
}