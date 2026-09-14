use approx::assert_relative_eq;
use nalgebra::{Matrix3, Rotation3, Vector3};
use spectral_core::procrustes::*;

/// THEOREM: rotação pura → residual 0, det(R)=1.
#[test]
fn test_align_known_rotation_residual_zero() {
    let a = Matrix3::identity();
    let theta = std::f64::consts::FRAC_PI_4;
    let r_z = Rotation3::from_axis_angle(&Vector3::z_axis(), theta).into_inner();
    let b = r_z.transpose();

    let r = align_keys(&a, &b).unwrap();
    assert!((a - b * r).norm() < 1e-9);
    assert!((r.determinant() - 1.0).abs() < 1e-9);
}

/// THEOREM: reflexão → residual mínimo = 2.0.
#[test]
fn test_align_reflection_residual_minimum() {
    let a = Matrix3::new(1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, -1.0);
    let b = Matrix3::identity();

    let r = align_keys(&a, &b).unwrap();
    assert!((r.determinant() - 1.0).abs() < 1e-9);
    let residual = (a - b * r).norm();
    assert_relative_eq!(residual, 2.0, epsilon = 1e-9);
}