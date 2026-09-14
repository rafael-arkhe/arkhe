use approx::assert_relative_eq;
use nalgebra::{DMatrix, DVector};
use signature_audit::audit_signature_consistency;

/// THEOREM: sistema consistente → residual 0, cond = √3.
#[test]
fn test_consistent_exact() {
    let a = DMatrix::from_row_slice(3, 2, &[1.0, 0.0, 0.0, 1.0, 1.0, 1.0]);
    let b = DVector::from_row_slice(&[1.0, 2.0, 3.0]);
    let r = audit_signature_consistency(&a, &b, 1e-6);
    assert!(r.consistent);
    assert!(r.residual < 1e-10);
    assert_relative_eq!(r.condition_number, 3.0_f64.sqrt(), epsilon = 1e-9);
}

/// THEOREM: sistema inconsistente → residual = 97√3/3.
#[test]
fn test_inconsistent_exact_residual() {
    let a = DMatrix::from_row_slice(3, 2, &[1.0, 0.0, 0.0, 1.0, 1.0, 1.0]);
    let b = DVector::from_row_slice(&[1.0, 2.0, 100.0]);
    let r = audit_signature_consistency(&a, &b, 1e-6);
    let expected = 97.0 * 3.0_f64.sqrt() / 3.0;
    assert!(!r.consistent);
    assert_relative_eq!(r.residual, expected, epsilon = 1e-9);
}