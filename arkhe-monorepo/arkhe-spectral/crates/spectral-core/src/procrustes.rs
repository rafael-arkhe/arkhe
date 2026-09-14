//! Alinhamento ortogonal (Procrustes / Kabsch) para compatibilidade TPM↔enclave.

use nalgebra::{Matrix3, SVD};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProcrustesError {
    #[error("Matriz singular")]
    Singular,
}

/// Alinha A e B via Procrustes ortogonal. Retorna R tal que A ≈ B·R.
pub fn align_keys(a: &Matrix3<f64>, b: &Matrix3<f64>) -> Result<Matrix3<f64>, ProcrustesError> {
    let m = b.transpose() * a;
    let svd = SVD::new(m, true, true);

    let u = svd.u.ok_or(ProcrustesError::Singular)?;
    let v_t = svd.v_t.ok_or(ProcrustesError::Singular)?;

    let mut r = u * v_t;

    if r.determinant() < 0.0 {
        let mut d = Matrix3::identity();
        d[(2, 2)] = -1.0;
        r = u * d * v_t;
    }
    Ok(r)
}

pub fn key_compatibility_error(a: &Matrix3<f64>, b: &Matrix3<f64>) -> f64 {
    match align_keys(a, b) {
        Ok(r) => (a - b * r).norm(),
        Err(_) => f64::INFINITY,
    }
}