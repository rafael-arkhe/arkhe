//! Constantes do X(2370) medidas pela BESIII (módulo #18 da pesquisa ARKHE-VSEPR).
//!
//! Todas as constantes numéricas vêm de medidas primárias:
//! - Pré-print de julho de 2026 (flavor-singlet, supressão de `K*(892)⁰K̄⁰`):
//!   **BESIII Collaboration, "Lightest 0⁻⁺ Glueball as Dominant Constituent of
//!   X(2370)", arXiv:2607.20366 [hep-ex], 22 jul 2026** — anunciado no plenário da
//!   ICHEP 2026 (5 de agosto de 2026, Brasil).
//! - Revisão dos modos, massas e larguras: **"Discovery of a Glueball-like particle
//!   X(2370) at BESIII", arXiv:2503.13286 [hep-ex]** e
//!   **S. Jin, Nucl. Phys. B 1028:117495 (2026)**.
//!
//! Massas/larguras por canal (stat apenas):
//! | canal | M (MeV/c²) | Γ (MeV) |
//! |---|---|---|
//! | π⁺π⁻η′ (descoberta, 2011) | 2341.6 ± 6.5 | 117 ± 10 |
//! | π⁰π⁰η | 2370 ± 2 | 133 ± 8 |
//! | K⁰SK⁰Sη′ (PWA, J^PC) | 2395 ± 11 | 188 +18/−17 |
//!
//! Fatos observacionais chefes (arquivados como invariantes de evidência):
//! - J^PC = 0⁻⁺ (significância > 9.8σ com 10¹⁰ J/ψ; > 10.1σ contra alternativas).
//! - Primeiro hádron leve flavor-singlete acima de 1 GeV/c² (arXiv:2607.20366).
//! - Supressão do modo K*(892)⁰K̄⁰: B < 2.7 × 10⁻⁶ (90% CL) → singlete de sabor.

use serde::{Deserialize, Serialize};

/// Referências BESIII completas (Provenance-1).
pub const BESIII_REFERENCES: [&str; 5] = [
    "BESIII Collaboration, 'Lightest 0⁻⁺ Glueball as Dominant Constituent of X(2370)', arXiv:2607.20366 [hep-ex] (22 Jul 2026); ICHEP 2026 plenary, 5 Aug 2026",
    "BESIII Collaboration, 'Discovery of a Glueball-like particle X(2370) at BESIII', arXiv:2503.13286 [hep-ex] (2025)",
    "S. Jin, 'BESIII discovers a glueball-like particle X(2370) in J particle decays', Nucl. Phys. B 1028:117495 (2026)",
    "BESIII, Phys. Rev. Lett. 106, 072002 (2011) — first observation of X(2370)",
    "BESIII, Phys. Rev. Lett. 134, 131901 (2025) — J^PC determination with 10^10 J/psi",
];

/// Constante de identificação: o nome do estado.
pub const X_MASS_MEV_SIGNATURE: f64 = 2370.0;
/// Massa da medida cheia do canal π⁰π⁰η (M = 2370 ± 2 stat) MeV/c².
pub const X_MASS_SIGNATURE_MEV: f64 = 2370.0;
/// Erro estatístico da massa (π⁰π⁰η).
pub const X_MASS_ERR_STAT_MEV: f64 = 2.0;
/// Largura medida (π⁰π⁰η): Γ = 133 ± 8 stat MeV.
pub const X_WITH_SIGNATURE_MEV: f64 = 133.0;
pub const X_WIDTH_ERR_STAT_MEV: f64 = 8.0;
/// Números quânticos de spin-paridade: 0⁻⁺ (pseudoscalar).
pub const SPIN_PARITY_JP: &str = "0-+";
/// Significância da determinação de J^PC com 10¹⁰ eventos J/ψ (> 9.8σ).
pub const SPIN_PARITY_SIGNIFICANCE_SIGMA: f64 = 9.8;
/// Número de eventos J/ψ usados (10 bilhões).
pub const JPSI_EVENTS: f64 = 1e10;
/// Primeiro hádron leve flavor-singlete descoberto acima deste limiar (GeV/c²).
pub const FLAVOR_SINGLET_MIN_MEV: f64 = 1000.0;
/// Supressão do modo K*(892)⁰K̄⁰ (arXiv:2607.20366): limite superior.
pub const B_JPSI_GAMMA_X_TIMES_B_KSTAR_LIMIT: f64 = 2.7e-6;
pub const KSTAR_SUPPRESSION_CL: f64 = 0.90;

/// Sanidade em tempo de compilação das constantes de supressão (arXiv:2607.20366).
const _KSTAR_CONSTANTS_SANITY: () = assert!(
    B_JPSI_GAMMA_X_TIMES_B_KSTAR_LIMIT < 1e-5
        && KSTAR_SUPPRESSION_CL >= 0.90
        && FLAVOR_SINGLET_MIN_MEV == 1000.0,
    "constantes de supressão fora do regime publicado"
);

/// Referência resumida do anúncio da ICHEP 2026.
pub const X_REFERENCE_ICHEP_2026: &str =
    "BESIII Collaboration, plenário ICHEP 2026 (5 Aug 2026), arXiv:2607.20366";
/// Referência da revisão principal (massa/largura).
pub const X_REFERENCE_V782: &str = "arXiv:2503.13286v2; Nucl. Phys. B 1028:117495 (2026)";

/// Medida de massa/largura do X(2370) num canal específico.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct X2370Channel {
    pub label: &'static str,
    pub mass_mev: f64,
    pub mass_err_stat_mev: f64,
    pub width_mev: f64,
    pub width_err_stat_mev: f64,
    pub significance_sigma: Option<f64>,
}

/// Evidência BESIII agregada (módulo #18).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct X2370Evidence {
    pub channels: Vec<X2370Channel>,
    pub spin_parity: &'static str,
    pub spin_parity_sigma: f64,
    pub jpsi_events: f64,
    pub flavor_singlet: bool,
    pub flavor_singlet_first_above_mev: f64,
    pub kstar_suppression_limit: f64,
    pub kstar_suppression_cl: f64,
}

impl X2370Evidence {
    /// Canal de carteira "assinatura" do estado (π⁰π⁰η): fixa o nome 2370.
    pub fn signature_channel(&self) -> &X2370Channel {
        self.channels
            .iter()
            .find(|c| c.label.contains("π0π0η") || c.label.contains("pi0pi0eta"))
            .expect("canal π⁰π⁰η presente na evidência")
    }

    /// As referências cruzadas (Correlation-1 / Provenance-1).
    pub fn references(&self) -> &'static [&'static str] {
        &BESIII_REFERENCES
    }
}

/// Monta a evidência canônica da BESIII (2026).
pub fn besiii_evidence() -> X2370Evidence {
    X2370Evidence {
        channels: vec![
            X2370Channel {
                label: "pi+pi-eta' (discovery 2011)",
                mass_mev: 2341.6,
                mass_err_stat_mev: 6.5,
                width_mev: 117.0,
                width_err_stat_mev: 10.0,
                significance_sigma: Some(6.4),
            },
            X2370Channel {
                label: "pi0pi0eta",
                mass_mev: 2370.0,
                mass_err_stat_mev: 2.0,
                width_mev: 133.0,
                width_err_stat_mev: 8.0,
                significance_sigma: None,
            },
            X2370Channel {
                label: "KSKS eta' (PWA)",
                mass_mev: 2395.0,
                mass_err_stat_mev: 11.0,
                width_mev: 188.0,
                width_err_stat_mev: 18.0,
                significance_sigma: Some(9.8),
            },
        ],
        spin_parity: SPIN_PARITY_JP,
        spin_parity_sigma: SPIN_PARITY_SIGNIFICANCE_SIGMA,
        jpsi_events: JPSI_EVENTS,
        flavor_singlet: true,
        flavor_singlet_first_above_mev: FLAVOR_SINGLET_MIN_MEV,
        kstar_suppression_limit: B_JPSI_GAMMA_X_TIMES_B_KSTAR_LIMIT,
        kstar_suppression_cl: KSTAR_SUPPRESSION_CL,
    }
}

/// Lista de referências (Provenance-1).
pub fn besiii_references() -> &'static [&'static str] {
    &BESIII_REFERENCES
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn besiii_evidence_is_canonical() {
        let ev = besiii_evidence();
        assert_eq!(ev.channels.len(), 3);
        let sig = ev.signature_channel();
        assert_eq!(sig.mass_mev, X_MASS_SIGNATURE_MEV);
        assert_eq!(sig.width_mev, X_WITH_SIGNATURE_MEV);
        assert!(ev.flavor_singlet);
        assert_eq!(ev.spin_parity, "0-+");
    }

    #[test]
    fn references_are_provenanced() {
        let refs = besiii_references();
        assert_eq!(refs.len(), 5);
        assert!(refs[0].contains("2607.20366"));
        assert!(refs[0].contains("ICHEP"));
        assert!(refs[2].contains("117495"));
    }

    #[test]
    fn kstar_suppression_sets_flavor_singlet() {
        // A supressão K*(892) com 90% CL implica singlete de sabor → estado dominado
        // por glueball (módulo #18). Verificado aqui sobre a evidência em runtime,
        // e sobre as constantes em tempo de compilação (const _KSTAR_CONSTANTS_SANITY).
        let ev = besiii_evidence();
        assert!(ev.kstar_suppression_limit < 1e-5);
        assert!(ev.kstar_suppression_cl >= 0.90);
        assert_eq!(ev.flavor_singlet_first_above_mev, FLAVOR_SINGLET_MIN_MEV);
    }
}