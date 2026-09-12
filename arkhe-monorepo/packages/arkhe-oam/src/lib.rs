//! Pureza da carga de Momento Angular Orbital (OAM) de um feixe (#10).
//!
//! A pureza é o peso do modo LG desejado `ℓ` na decomposição modal do campo:
//! `P = |c_ℓ|² / Σ|c_k|²`. O z-puro (módulo 546) exige pureza OAM alta e
//! coerência Φ_C no limite Gap-1.

use serde::{Deserialize, Serialize};

/// Modo Laguerre–Gauss com carga ℓ e amplitude complexa.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LaguerreMode {
    pub charge: i64,
    pub amplitude: f64,
    pub phase_rad: f64,
}

/// Análise da pureza OAM de um conjunto de modos.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OamPurity {
    pub modes: Vec<LaguerreMode>,
    pub dominant_charge: i64,
    pub purity: f64,
    /// Entropia modal de Shannon em nats [0, ln(n)].
    pub modal_entropy: f64,
    pub exceeds_threshold: bool,
}

impl OamPurity {
    /// Analisa a decomposição modal; `target_charge` é a carga esperada.
    pub fn analyze(modes: Vec<LaguerreMode>, target_charge: i64) -> Self {
        assert!(!modes.is_empty(), "ao menos um modo é necessário");
        let total_power: f64 = modes
            .iter()
            .map(|m| m.amplitude * m.amplitude)
            .sum::<f64>()
            .max(1e-12);
        let target_amp = modes
            .iter()
            .find(|m| m.charge == target_charge)
            .map(|m| m.amplitude)
            .unwrap_or(0.0);
        let purity = (target_amp * target_amp) / total_power;

        let mut entropy = 0.0;
        for m in &modes {
            let p = (m.amplitude * m.amplitude) / total_power;
            if p > 1e-12 {
                entropy -= p * p.ln();
            }
        }
        let dominant = modes
            .iter()
            .max_by(|a, b| a.amplitude.total_cmp(&b.amplitude))
            .map(|m| m.charge)
            .unwrap_or(0);
        Self {
            exceeds_threshold: purity >= 0.95 && dominant == target_charge,
            purity,
            dominant_charge: dominant,
            modal_entropy: entropy,
            modes,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_oam_beam_passes() {
        let modes = vec![
            LaguerreMode { charge: 3, amplitude: 1.0, phase_rad: 0.0 },
            LaguerreMode { charge: -1, amplitude: 0.01, phase_rad: 0.0 },
        ];
        let p = OamPurity::analyze(modes, 3);
        assert!(p.purity > 0.99, "pureza {:.4}", p.purity);
        assert!(p.exceeds_threshold);
    }

    #[test]
    fn mixed_beam_fails_threshold() {
        let modes = vec![
            LaguerreMode { charge: 3, amplitude: 0.6, phase_rad: 0.0 },
            LaguerreMode { charge: 1, amplitude: 0.8, phase_rad: 0.0 },
            LaguerreMode { charge: -2, amplitude: 0.5, phase_rad: 0.0 },
        ];
        let p = OamPurity::analyze(modes, 3);
        assert!(!p.exceeds_threshold);
        assert!(p.modal_entropy > 0.0, "misto deve ter entropia modal > 0");
    }

    #[test]
    fn wrong_dominant_charge_fails() {
        let modes = vec![
            LaguerreMode { charge: 2, amplitude: 1.0, phase_rad: 0.0 },
            LaguerreMode { charge: 3, amplitude: 0.01, phase_rad: 0.0 },
        ];
        let p = OamPurity::analyze(modes, 3);
        assert_eq!(p.dominant_charge, 2);
        assert!(!p.exceeds_threshold);
    }
}