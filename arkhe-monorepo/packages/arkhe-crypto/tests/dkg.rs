//! Deteção de falha DKG (Feldman VSS) — convenção de limiar de reconstrução.

use arkhe_crypto::is_dkg_failed;

// Signature: is_dkg_failed(disq, n, t)
// Convention: reconstruction threshold. Failure iff
//   disq > t  ||  n - disq < t

#[test]
fn boundary_honest_equals_threshold_is_acceptable() {
    // disq=1, n=3, t=2 → honest = 2 == t → acceptable
    // (a) 1 > 2 → false
    // (b) 3 - 1 = 2 < 2 → false
    assert!(!is_dkg_failed(1, 3, 2));
}

#[test]
fn disq_exceeds_t_fails() {
    // disq=2, n=4, t=1 → 2 > 1 → failed
    assert!(is_dkg_failed(2, 4, 1));
}

#[test]
fn honest_below_t_fails() {
    // disq=3, n=5, t=3 → honest = 2 < 3 → failed
    // (a) 3 > 3 → false
    // (b) 5 - 3 = 2 < 3 → true
    assert!(is_dkg_failed(3, 5, 3));
}

#[test]
fn both_conditions_trigger() {
    // disq=5, n=6, t=1 → (a) 5 > 1 → true; (b) 6 - 5 = 1 < 1 → false
    // (a) already decides; saturating_sub not exercised
    assert!(is_dkg_failed(5, 6, 1));
}

#[test]
fn healthy_case() {
    // disq=0, n=10, t=3 → honest = 10 >= 3 → not failed
    assert!(!is_dkg_failed(0, 10, 3));
    // disq=3, n=10, t=3 → honest = 7 >= 3, disq = 3 <= 3 → not failed
    assert!(!is_dkg_failed(3, 10, 3));
}

#[test]
fn saturating_sub_handles_disq_greater_than_n() {
    // Malformed input (disq > n): saturating_sub → 0.
    // disq=10, n=3, t=5 → (a) 10 > 5 → true
    assert!(is_dkg_failed(10, 3, 5));

    // disq=10, n=3, t=15 → (a) 10 > 15 → false;
    // (b) 3.saturating_sub(10) = 0 < 15 → true
    assert!(is_dkg_failed(10, 3, 15));
}