//! Auditoria de consistência de assinaturas via pseudo-inversa.

use nalgebra::{DMatrix, DVector};

#[derive(Debug, Clone)]
pub struct AuditResult {
    pub residual: f64,
    pub condition_number: f64,
    pub consistent: bool,
}

pub fn audit_signature_consistency(
    sigs: &DMatrix<f64>,
    msgs: &DVector<f64>,
    threshold: f64,
) -> AuditResult {
    let svd = sigs.clone().svd(true, true);
    let s_max = svd.singular_values.max();
    let s_min = svd.singular_values.min();
    let cond = if s_min > 1e-12 { s_max / s_min } else { f64::INFINITY };

    match svd.solve(msgs, 1e-10) {
        Ok(x) => {
            let predicted = sigs * x;
            let residual = (predicted - msgs).norm();
            AuditResult {
                residual,
                condition_number: cond,
                consistent: residual < threshold,
            }
        }
        Err(_) => AuditResult {
            residual: f64::INFINITY,
            condition_number: cond,
            consistent: false,
        },
    }
}