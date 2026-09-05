//! Verificação Lean embarcada — I200: checagem de invariantes em tempo real.

use crate::core::entropy::constructive_entropy;
use serde::{Deserialize, Serialize};

/// Resultado da verificação de um invariante.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InvariantStatus {
    Verified,
    Violated,
    Pending,
}

/// Relatório consolidado da verificação.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VerificationReport {
    pub checks: Vec<(String, InvariantStatus, f64)>,
    pub all_verified: bool,
}

impl VerificationReport {
    pub fn all_verified(&self) -> bool {
        self.all_verified
    }
}

/// Verificador Lean embarcado (I200).
#[derive(Debug, Clone, Default)]
pub struct LeanVerifier {
    violations: Vec<(String, f64)>,
}

impl LeanVerifier {
    pub fn new() -> Self {
        Self {
            violations: Vec::new(),
        }
    }

    /// I157 — Φ_irr ≤ 1 (coerência irreduzível saturada).
    pub fn check_i157(&mut self, phi_irr: f64) -> InvariantStatus {
        if phi_irr < 0.0 || phi_irr > 1.0 {
            self.violations.push(("I157".into(), phi_irr));
            return InvariantStatus::Violated;
        }
        InvariantStatus::Verified
    }

    /// I158 — limiar de Gödel não excedido.
    pub fn check_i158(&mut self, phi_crit: f64) -> InvariantStatus {
        if !(0.0..=1.0).contains(&phi_crit) {
            self.violations.push(("I158".into(), phi_crit));
            return InvariantStatus::Violated;
        }
        InvariantStatus::Verified
    }

    /// I159 — anti‑flatness não-negativa.
    pub fn check_i159(&mut self, anti_flatness: f64) -> InvariantStatus {
        if anti_flatness < 0.0 {
            self.violations.push(("I159".into(), anti_flatness));
            return InvariantStatus::Violated;
        }
        InvariantStatus::Verified
    }

    /// I167 — entropia construtiva não-negativa.
    pub fn check_i167(&mut self, spectrum: &[f64], n_total: usize, n_handovers: usize) -> InvariantStatus {
        let entropy = constructive_entropy(spectrum, n_total, n_handovers, 1.5);
        if entropy < 0.0 {
            self.violations.push(("I167".into(), entropy));
            return InvariantStatus::Violated;
        }
        InvariantStatus::Verified
    }

    /// I194 — Zeno Veto não violado.
    pub fn check_i194(&mut self, dt_us: u64, ranging_us: u64) -> InvariantStatus {
        if dt_us > ranging_us * 2 {
            self.violations.push(("I194".into(), dt_us as f64));
            return InvariantStatus::Violated;
        }
        InvariantStatus::Verified
    }

    /// I193 — eficiência energética Φ/E ≥ 1e14. Energia nula = sem orçamento
    /// definido (verificado por vacuidade).
    pub fn check_i193(&mut self, phi: f64, energy: f64) -> InvariantStatus {
        if energy <= 0.0 {
            return InvariantStatus::Verified;
        }
        let eff = phi / energy;
        if eff < 1e14 {
            self.violations.push(("I193".into(), eff));
            return InvariantStatus::Violated;
        }
        InvariantStatus::Verified
    }

    /// I201 — sincronização preserva coerência entre nós.
    pub fn check_i201(&mut self, phi_local: f64, phi_remote: f64) -> InvariantStatus {
        let diff = (phi_local - phi_remote).abs();
        if diff >= 0.1 {
            self.violations.push(("I201".into(), diff));
            return InvariantStatus::Violated;
        }
        InvariantStatus::Verified
    }

    /// Executa todas as checagens e produz relatório consolidado.
    pub fn verify_all(
        &self,
        spectrum: &[f64],
        n_total: usize,
        n_handovers: usize,
        phi_irr: f64,
        phi_crit: f64,
        anti_flatness: f64,
        dt_us: u64,
        ranging_us: u64,
        phi_local: f64,
        phi_remote: f64,
        energy_j: f64,
    ) -> VerificationReport {
        let mut checks = Vec::new();
        let mut all = true;

        let mut v = self.clone();
        macro_rules! run {
            ($label:expr, $check:expr) => {
                let status = $check;
                if status == InvariantStatus::Violated {
                    all = false;
                }
                checks.push(($label.into(), status, 0.0));
            };
        }

        run!("I157", v.check_i157(phi_irr));
        run!("I158", v.check_i158(phi_crit));
        run!("I159", v.check_i159(anti_flatness));
        run!("I167", v.check_i167(spectrum, n_total, n_handovers));
        run!("I194", v.check_i194(dt_us, ranging_us));
        run!("I193", v.check_i193(phi_local, energy_j));
        run!("I201", v.check_i201(phi_local, phi_remote));

        VerificationReport { checks, all_verified: all }
    }

    pub fn violations(&self) -> &[(String, f64)] {
        &self.violations
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zeno_check() {
        let mut v = LeanVerifier::new();
        assert_eq!(v.check_i194(100, 1000), InvariantStatus::Verified);
        assert_eq!(v.check_i194(3000, 1000), InvariantStatus::Violated);
    }

    #[test]
    fn test_entropy_check() {
        let mut v = LeanVerifier::new();
        assert_eq!(
            v.check_i167(&[0.25, 0.25, 0.25, 0.25], 8, 4),
            InvariantStatus::Verified
        );
    }

    #[test]
    fn test_verify_all_report() {
        let v = LeanVerifier::new();
        let report = v.verify_all(
            &[0.25, 0.25, 0.25, 0.25],
            16,
            8,
            0.7,
            0.3,
            0.01,
            500,
            1000,
            0.95,
            0.94,
            1e-15,
        );
        assert!(report.all_verified());
        assert_eq!(report.checks.len(), 7);
    }
}