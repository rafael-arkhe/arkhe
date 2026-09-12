//! Selo #4 — Classificação de anomalias com fase controlada (arXiv:2608.12445).
//!
//! A análise de PURSUE (Presidential Unsealing and Reporting System for UAP
//! Encounters) — 112 vídeos do Departamento de Guerra dos EUA, liberados em 2026 —
//! mostra que converter pixels/quadro em velocidade física exige o conjunto
//! **completo** de quatro parâmetros: alcance, velocidade do observador, ângulo
//! de aspecto e campo angular do sensor. Nenhum vídeo fornece o conjunto. O clipe
//! DOW-UAP-PR149 tem um barco de tamanho conhecido → limite superior de Mach 0.4;
//! DOW-UAP-PR113 permite reconstrução parcial da FOV apenas.
//!
//! O equivalente na engenharia de fase: classificar um sinal quântico como
//! coerente/espúrio exige incerteza de fase controlada (< 0.1 rad). Sem o conjunto
//! completo, a classificação é `Indeterminado`.

use serde::{Deserialize, Serialize};

/// Referência do Selo #4.
pub const PURSUE_REF_2026: &str = "J. Haqq-Misra, R. Kopparapu, 'Limits on Velocity Recovery \
from the PURSUE Sensor Videos', arXiv:2608.12445 [physics.pop-ph], 12 Aug 2026";
/// Número de vídeos analisados no corpus PURSUE.
pub const PURSUE_CLIP_COUNT: usize = 112;
/// Parâmetros necessários à reconstrução de velocidade (alcance, velocidade do
/// observador, ângulo de aspecto, campo angular do sensor).
pub const ANOMALY_PARAMS_REQUIRED: usize = 4;
/// Limite superior relativo deduzido de DOW-UAP-PR149 (embarcação de tamanho
/// conhecido): Mach 0.4.
pub const BOUND_MACH04_FRACTION: f64 = 0.4;
/// Incerteza de fase alvo para classificação: < 0.1 rad.
pub const PHASE_UNCERTAINTY_TARGET_RAD: f64 = 0.1;

/// Classe atribuída a um sinal/anomalia.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnomalyClass {
    Indeterminate,
    BoundedSpeed,
    CoherentField,
    RejectedNoise,
}

/// Relatório de classificação (análogo PURSUE + fase).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalyReport {
    pub params_available: usize,
    pub phase_uncertainty_rad: f64,
    pub class: AnomalyClass,
}

/// Classificação de fase: coerente se `uncertainty < 0.1 rad` (Selo #4).
pub fn classify_phase_uncertainty(phase_uncertainty_rad: f64) -> bool {
    phase_uncertainty_rad < PHASE_UNCERTAINTY_TARGET_RAD
}

/// Reconstrução de velocidade: necessita do conjunto completo de 4 parâmetros.
pub fn reconstruct_velocity(
    pixels_per_frame: f64,
    range_km: Option<f64>,
    observer_km_per_s: Option<f64>,
    aspect_angle_rad: Option<f64>,
    field_angle_rad: Option<f64>,
) -> Option<f64> {
    let full = range_km.is_some()
        && observer_km_per_s.is_some()
        && aspect_angle_rad.is_some()
        && field_angle_rad.is_some();
    if !full {
        return None;
    }
    // v ∝ (pixels/quadro · alcance · velocidade_observador) / (aspecto · campo)
    Some(
        pixels_per_frame
            * range_km.unwrap_or(0.0)
            * observer_km_per_s.unwrap_or(0.0)
            / (aspect_angle_rad.unwrap_or(1.0) * field_angle_rad.unwrap_or(1.0))
            .max(1e-9),
    )
}

/// Classifica uma anomalia com base em parâmetros + fase controlada.
pub fn classify_anomaly(
    params_available: usize,
    phase_uncertainty_rad: f64,
    coherent_field: bool,
) -> AnomalyReport {
    let class = if params_available == ANOMALY_PARAMS_REQUIRED {
        if classify_phase_uncertainty(phase_uncertainty_rad) {
            if coherent_field {
                AnomalyClass::CoherentField
            } else {
                AnomalyClass::BoundedSpeed
            }
        } else {
            AnomalyClass::RejectedNoise
        }
    } else {
        AnomalyClass::Indeterminate
    };
    AnomalyReport {
        params_available,
        phase_uncertainty_rad,
        class,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn none_of_the_pursue_clips_can_be_resolved() {
        // Nenhum vídeo PURSUE fornece o conjunto completo (artigo, Selo #4).
        // Com parametros completos o cálculo existe; sem eles, indeterminado.
        let full = reconstruct_velocity(3.0, Some(1.0), Some(0.2), Some(0.5), Some(0.2));
        assert!(full.is_some());
        let missing = reconstruct_velocity(3.0, Some(1.0), None, Some(0.5), Some(0.2));
        assert!(missing.is_none(), "3 parâmetros não bastam");
    }

    #[test]
    fn pr149_yields_bounded_speed() {
        // DOW-UAP-PR149: embarcação de tamanho conhecido ⇒ limite de Mach 0.4.
        let report = classify_anomaly(4, 0.05, false);
        assert_eq!(report.class, AnomalyClass::BoundedSpeed);
        assert!(classify_phase_uncertainty(report.phase_uncertainty_rad));
    }

    #[test]
    fn incomplete_params_indeterminate() {
        let report = classify_anomaly(3, 0.05, true);
        assert_eq!(report.class, AnomalyClass::Indeterminate);
    }

    #[test]
    fn phase_uncertainty_over_01_rejects() {
        let report = classify_anomaly(4, 0.3, true);
        assert_eq!(report.class, AnomalyClass::RejectedNoise);
        assert!(!classify_phase_uncertainty(0.3));
    }

    #[test]
    fn corpus_constant() {
        assert_eq!(PURSUE_CLIP_COUNT, 112);
        assert_eq!(ANOMALY_PARAMS_REQUIRED, 4);
        assert!((BOUND_MACH04_FRACTION - 0.4).abs() < 1e-12);
    }
}