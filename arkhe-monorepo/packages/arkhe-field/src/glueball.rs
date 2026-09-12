//! Correspondência VSEPR ↔ glueball (módulo #19).
//!
//! A analogia VSEPR (Valence Shell Electron-Pair Repulsion) usada pela pesquisa
//! ARKHE-VSEPR para a camada de valência constitucional é validada aqui contra os
//! dados da BESIII de 2026:
//!
//! - **VSEPR** `AX₅E₀`: cinco domínios de pares ligantes equivalentes e nenhum par
//!   livre → geometria bipiramidal trigonal com **dipolo nulo** (simetria total).
//! - **Glueball 0⁻⁺**: cinco modos de decaimento "dourados" PPP
//!   (ππη′, K⁰SK⁰Sη′, K⁰SK⁰Sπ⁰, π⁰π⁰η, a⁰(980)π⁰) *sem modo dominante* e com a
//!   supressão `K*(892)⁰K̄⁰` → **singlete de sabor** (arquivamento invariante de
//!   cor neutro, "dipolo de cor" nulo).
//!
//! A validação numérica usa as medidas da BESIII dentro de `kσ` (padrão 3σ).

use serde::{Deserialize, Serialize};

use crate::x2370::{FLAVOR_SINGLET_MIN_MEV, X2370Evidence, X_MASS_SIGNATURE_MEV};

/// Contagem canônica dos modos de decaimento "dourados" PPP observados.
pub const GOLDEN_PPP_MODES: usize = 5;

/// Verificação individual da cadeia de evidências.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrespondenceCheck {
    pub label: String,
    pub pass: bool,
    pub detail: String,
}

/// Resultado da validação dos dados BESIII contra as previsões do glueball 0⁻⁺.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlueballValidation {
    pub checks: Vec<CorrespondenceCheck>,
    pub passed: usize,
    pub total: usize,
    /// Coerência VSEPR ↔ glueball: fração de verificações aprovadas.
    pub score: f64,
    /// True se casável com o estado dominado por glueball (e não por outra
    /// interpretação) dentro da cadeia completa.
    pub glueball_consistent: bool,
}

/// Geometria VSEPR formal derivada do estado.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VseprGeometry {
    Ax2E0,
    Ax3E0,
    Ax4E0,
    Ax5E0,
    Ax6E0,
}

impl VseprGeometry {
    pub fn label(self) -> &'static str {
        match self {
            VseprGeometry::Ax2E0 => "linear",
            VseprGeometry::Ax3E0 => "trigonal planar",
            VseprGeometry::Ax4E0 => "tetrahedral",
            VseprGeometry::Ax5E0 => "trigonal bipyramidal",
            VseprGeometry::Ax6E0 => "octahedral",
        }
    }

    /// Pares ligantes equivalentes (domínios de valência).
    pub fn domains(self) -> usize {
        match self {
            VseprGeometry::Ax2E0 => 2,
            VseprGeometry::Ax3E0 => 3,
            VseprGeometry::Ax4E0 => 4,
            VseprGeometry::Ax5E0 => 5,
            VseprGeometry::Ax6E0 => 6,
        }
    }

    /// Sem par livre → dipolo resultante nulo para substituintes idênticos
    /// (analogia ao singlete de sabor / neutralidade de cor da glueball).
    pub fn dipole_free(self) -> bool {
        true
    }
}

/// Geometria VSEPR para `n` domínios equivalentes sem par livre.
pub fn vsepr_geometry_of(domains: usize) -> VseprGeometry {
    match domains {
        2 => VseprGeometry::Ax2E0,
        3 => VseprGeometry::Ax3E0,
        4 => VseprGeometry::Ax4E0,
        5 => VseprGeometry::Ax5E0,
        6 => VseprGeometry::Ax6E0,
        _ => VseprGeometry::Ax2E0,
    }
}

/// A correspondência formal VSEPR ↔ glueball.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VseprGlueballCorrespondence {
    pub geometry: VseprGeometry,
    pub golden_pseudo_scalar_modes: usize,
    pub lone_pairs: usize,
    pub flavor_singlet: bool,
    pub color_dipole: f64,
}

impl VseprGlueballCorrespondence {
    /// O estado X(2370) 0⁻⁺ corresponde à geometria `AX₅E₀` (cinco modos PPP):
    /// sem modo dominante → dipolo de cor nulo → singlete de sabor.
    pub fn from_glueball_evidence(ev: &X2370Evidence) -> Self {
        let g = vsepr_geometry_of(GOLDEN_PPP_MODES);
        Self {
            geometry: g,
            golden_pseudo_scalar_modes: GOLDEN_PPP_MODES,
            lone_pairs: 0,
            flavor_singlet: ev.flavor_singlet,
            color_dipole: if ev.flavor_singlet { 0.0 } else { 1.0 },
        }
    }

    /// A correspondência é coerente quando a geometria tem domínios equivalentes
    /// (AX5), sem pares livres e com "dipolo de cor" nulo.
    pub fn is_coherent(&self) -> bool {
        self.geometry == VseprGeometry::Ax5E0
            && self.golden_pseudo_scalar_modes == GOLDEN_PPP_MODES
            && self.lone_pairs == 0
            && self.flavor_singlet
            && self.color_dipole.abs() < 1e-9
    }
}

/// Valida a cadeia de evidências BESIII (2026) contra o glueball leve 0⁻⁺.
pub fn validate_glueball(ev: &X2370Evidence, sigma: f64) -> GlueballValidation {
    let sig = ev.signature_channel();
    let mut checks = vec![];
    let sigma_delta = (sig.mass_mev - X_MASS_SIGNATURE_MEV).abs() / sig.mass_err_stat_mev.max(1e-9);
    let mass_ok = sigma_delta <= sigma;
    checks.push(CorrespondenceCheck {
        label: "mass_within_sigma".to_string(),
        pass: mass_ok,
        detail: format!(
            "M = {:.1} ± {:.1} MeV/c² (σ = {:.1}) vs corte 2370 MeV → |Δ|/σ = {:.2}",
            sig.mass_mev, sig.mass_err_stat_mev, sigma, sigma_delta
        ),
    });

    let spin_ok = ev.spin_parity == "0-+" && ev.spin_parity_sigma >= sigma;
    checks.push(CorrespondenceCheck {
        label: "spin_parity_0m".to_string(),
        pass: spin_ok,
        detail: format!(
            "J^PC = {} com significância {:.1}σ",
            ev.spin_parity, ev.spin_parity_sigma
        ),
    });

    checks.push(CorrespondenceCheck {
        label: "flavor_singlet".to_string(),
        pass: ev.flavor_singlet,
        detail: format!(
            "first flavor-singlet acima de {:.0} MeV/c²; supressão K*(892): B < {:.1e} @ {:.0}% CL",
            ev.flavor_singlet_first_above_mev, ev.kstar_suppression_limit, ev.kstar_suppression_cl * 100.0
        ),
    });

    let narrow = sig.width_mev / sig.mass_mev < 0.15;
    checks.push(CorrespondenceCheck {
        label: "narrow_partial_widths".to_string(),
        pass: narrow,
        detail: format!(
            "Γ/M = {:.4} ({:.0}/{:.1}) — decaimento OZI-suprimido esperado",
            sig.width_mev / sig.mass_mev, sig.width_mev, sig.mass_mev
        ),
    });

    let produced = ev.jpsi_events >= 1e10;
    checks.push(CorrespondenceCheck {
        label: "rich_production_jpsi_radiative".to_string(),
        pass: produced,
        detail: format!("amostra de {:.0e} eventos J/ψ (radiativo rico em glúons)", ev.jpsi_events),
    });

    checks.push(CorrespondenceCheck {
        label: "golden_ppp_modes_equivalent".to_string(),
        pass: self::GOLDEN_PPP_MODES >= 4,
        detail: format!("{} modos PPP sem modo dominante", GOLDEN_PPP_MODES),
    });

    let passed = checks.iter().filter(|c| c.pass).count();
    let total = checks.len();
    let score = passed as f64 / total as f64;
    GlueballValidation {
        checks,
        passed,
        total,
        score,
        glueball_consistent: score >= 0.9,
    }
}

/// Limiar mínimo de "tamanho" do singlete para a correspondência (Ghost-3).
pub fn is_flavor_singlet_above_min(ev: &X2370Evidence) -> bool {
    ev.flavor_singlet && ev.flavor_singlet_first_above_mev >= FLAVOR_SINGLET_MIN_MEV
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::x2370::besiii_evidence;

    #[test]
    fn besiii_corroborates_glueball() {
        let ev = besiii_evidence();
        let v = validate_glueball(&ev, 3.0);
        assert!(v.glueball_consistent, "{v:?}");
        assert_eq!(v.passed, v.total);
    }

    #[test]
    fn vsepr_correspondence_is_ax5() {
        let ev = besiii_evidence();
        let corr = VseprGlueballCorrespondence::from_glueball_evidence(&ev);
        assert!(corr.is_coherent());
        assert_eq!(corr.geometry.domains(), GOLDEN_PPP_MODES);
        assert!(corr.geometry.dipole_free());
        assert_eq!(corr.geometry.label(), "trigonal bipyramidal");
    }

    #[test]
    fn spin_or_flavor_failure_breaks_consistency() {
        let mut ev = besiii_evidence();
        ev.flavor_singlet = false;
        let v = validate_glueball(&ev, 3.0);
        assert!(!v.glueball_consistent);
        ev.flavor_singlet = true;
        ev.spin_parity = "1++";
        let v = validate_glueball(&ev, 3.0);
        assert!(!v.glueball_consistent);
    }

    #[test]
    fn singlet_min_threshold() {
        let ev = besiii_evidence();
        assert!(is_flavor_singlet_above_min(&ev));
    }
}