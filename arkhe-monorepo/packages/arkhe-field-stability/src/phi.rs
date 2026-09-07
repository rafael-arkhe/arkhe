//! Agregação Fase 7 — Φ canônico quadrático + eixos de garantia
//! (SemanticValidity/Loopseal) num relatório coeso.
//!
//! A **fórmula de Φ não muda** — permanece a quadrática canônica
//! `Φ = 1 − sqrt(w_Ω(1−Ω)² + w_Σ(1−Σ)² + w_Λ(1−Λ)²)` com `W=(0.4,0.4,0.2)`
//! definida e provada em [`crate::coherence`] (I511–I516). Esta module
//! **adiciona** um envelope de relatório que junta, a cada janela de medição:
//!
//! * os **componentes de medição** `Ω`/`Σ`/`Λ` (que **compõem** Φ);
//! * os **eixos de garantia** — SemanticValidity ([`crate::validators`]) e
//!   Loopseal ([`crate::loopseal`]) — que **não compõem Φ**, mas qualificam a
//!   confiança do dado num relatório ortogonal (parecer v381.1 / bloco 1004).
//!
//! Isso garante que os novos eixos da Fase 7 sejam auditados sem alterar o
//! funcional de coerência já canonizado — mantendo consistência com os núcleos
//! Lean/Rust existentes.

use serde::{Deserialize, Serialize};

use crate::coherence::phi_default;
use crate::ledger::IntegrityStatus;
use crate::loopseal::LoopStatus;
use crate::validators::SemanticValidity;

/// Status consolidado dos eixos de garantia (ortogonais a Φ).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Assurance {
    /// Integridade do ledger (Fase 6 — protege o dado; não compõe Φ).
    pub integrity: IntegrityStatus,
    /// Semântica — sanção por quórum estrito de validadores (>2/3).
    pub semantic: SemanticValidity,
    /// Aelíclicidade — ausência de loops na cadeia de handovers.
    pub loop_status: LoopStatus,
}

impl Assurance {
    /// `true` quando todos os eixos de garantia estão satisfeitos.
    #[must_use]
    pub fn is_sound(&self) -> bool {
        self.integrity.is_ok()
            && self.semantic.is_satisfied()
            && matches!(self.loop_status, LoopStatus::New { .. })
    }
}

/// Agregação consolidada de uma janela de medição (Fase 7).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoherenceReport {
    /// Componente `Ω` — estabilidade de campo (compõe Φ).
    pub stability: f64,
    /// Componente `Σ` — taxa de sucesso (compõe Φ).
    pub success_rate: f64,
    /// Componente `Λ` — score de latência (compõe Φ).
    pub latency_score: f64,
    /// Φ canônico quadrático consolidado.
    pub phi: f64,
    /// Eixos de garantia ortogonais (não compõem Φ).
    pub assurance: Assurance,
    /// `true` se todos os eixos de garantia estão sólidos **e** Φ satisfaz a
    /// faixa constitucional Gap-1 (`1/√3 < Φ ≤ 0.9999`).
    pub acceptable: bool,
}

impl CoherenceReport {
    /// Consolida uma janela de medição a partir dos componentes e do envelope
    /// de garantia.
    ///
    /// * `stability`, `success_rate`, `latency_score` ∈ [0,1] — compõem Φ com
    ///   os pesos canônicos [`WEIGHTS_DEFAULT`].
    /// * `integrity`, `semantic`, `loop_status` — eixos de garantia.
    #[must_use]
    pub fn new(
        stability: f64,
        success_rate: f64,
        latency_score: f64,
        integrity: IntegrityStatus,
        semantic: SemanticValidity,
        loop_status: LoopStatus,
    ) -> Self {
        let phi = phi_default([stability, success_rate, latency_score]);
        let assurance = Assurance {
            integrity,
            semantic,
            loop_status,
        };
        let acceptable = assurance.is_sound() && gap1_satisfied(phi);
        Self {
            stability,
            success_rate,
            latency_score,
            phi,
            assurance,
            acceptable,
        }
    }
}

/// Borda inferior da faixa Gap-1 (`1/√3 ≈ 0.5773502691`).
pub const GAP1_INFERIOR: f64 = 0.5773502691896257;
/// Borda superior da faixa Gap-1 (`0.999900`).
pub const GAP1_SUPERIOR: f64 = 0.9999;

/// `true` quando `phi` satisfaz a faixa constitucional
/// `GAP1_INFERIOR < Φ ≤ GAP1_SUPERIOR` (Gap-1).
#[must_use]
pub fn gap1_satisfied(phi: f64) -> bool {
    phi > GAP1_INFERIOR && phi <= GAP1_SUPERIOR
}

/// Serializa um [`CoherenceReport`] como JSON pretty.
#[must_use]
pub fn report_to_json_pretty(report: &CoherenceReport) -> String {
    serde_json::to_string_pretty(report).expect("report serializable")
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::ledger::GENESIS;
    use crate::loopseal::ChainLink;

    fn sound_assurance() -> (IntegrityStatus, SemanticValidity, LoopStatus) {
        (
            IntegrityStatus::Ok,
            SemanticValidity::new(3, 3),
            LoopStatus::New {
                previous_hash: Some(GENESIS.into()),
            },
        )
    }

    #[test]
    fn phi_unchanged_by_content_media() {
        // V1: Ω=0.9517, Σ=1.0, Λ=0.96 → Φ = 0.9646 (mesmo da Fase 1).
        let r = CoherenceReport::new(
            0.9517,
            1.0,
            0.96,
            IntegrityStatus::Ok,
            SemanticValidity::new(3, 3),
            LoopStatus::New {
                previous_hash: Some(GENESIS.into()),
            },
        );
        assert!((r.phi - 0.9646).abs() < 1e-4, "Φ V1 = {}", r.phi);
        assert!((crate::WEIGHTS_DEFAULT[0] - 0.4).abs() < 1e-12);
    }

    #[test]
    fn acceptable_when_sound_and_gap1() {
        let (i, s, l) = sound_assurance();
        let r = CoherenceReport::new(0.9517, 1.0, 0.96, i, s, l);
        assert!(r.assurance.is_sound());
        assert!(gap1_satisfied(r.phi));
        assert!(r.acceptable);
    }

    #[test]
    fn not_acceptable_when_assurance_unsound() {
        let loop_status = LoopStatus::Loop { duplicate_index: 0 };
        let r = CoherenceReport::new(
            0.9517,
            1.0,
            0.96,
            IntegrityStatus::Ok,
            SemanticValidity::new(3, 3),
            loop_status,
        );
        assert!(!r.assurance.is_sound());
        assert!(!r.acceptable);
    }

    #[test]
    fn not_acceptable_when_semantic_unmet() {
        let (i, _s, l) = sound_assurance();
        // 2/3 exato não sanciona SemanticValidity.
        let semantic = SemanticValidity::new(2, 3);
        let r = CoherenceReport::new(0.9517, 1.0, 0.96, i, semantic, l);
        assert!(!r.acceptable);
    }

    #[test]
    fn not_acceptable_when_integrity_broken() {
        let (_i, s, l) = sound_assurance();
        let broken = IntegrityStatus::Broken { window_ids: vec![0] };
        let r = CoherenceReport::new(0.9517, 1.0, 0.96, broken, s, l);
        assert!(!r.acceptable);
    }

    #[test]
    fn not_acceptable_when_below_gap1() {
        let (i, s, l) = sound_assurance();
        // Ω=Σ=Λ=0.5 → Φ=0.5 < 1/√3 (V3, rejeitado por Gap-1).
        let r = CoherenceReport::new(0.5, 0.5, 0.5, i, s, l);
        assert!(r.assurance.is_sound(), "assurance sólida mesmo com Φ baixo");
        assert!(!gap1_satisfied(r.phi));
        assert!(!r.acceptable);
    }

    #[test]
    fn beyond_horizon_still_sound_but_docs_flagged() {
        let beyond = IntegrityStatus::BeyondHorizon { len: 5, max_windows: 4 };
        let (_, s, l) = sound_assurance();
        let r = CoherenceReport::new(0.9517, 1.0, 0.96, beyond, s, l);
        // BeyondHorizon é integridade confirmada (is_ok) → garantia sólida.
        assert!(r.assurance.is_sound());
        assert!(r.acceptable);
    }

    #[test]
    fn gap1_bounds_constants() {
        use crate::coherence::phi_default;
        let inferior = GAP1_INFERIOR;
        assert!(inferior > 0.577350 && inferior < 0.577351);
        let superior = GAP1_SUPERIOR;
        assert!((superior - 0.9999).abs() < 1e-12);
        assert!(gap1_satisfied(phi_default([0.9517, 1.0, 0.96])));
        assert!(!gap1_satisfied(phi_default([0.5, 0.5, 0.5])));
        assert!(!gap1_satisfied(1.0));
    }

    #[test]
    fn loop_status_new_from_chainlink_matchess() {
        // Bridge entre loopseal e o envelope: um ChainLink novo → LoopStatus::New.
        let mut ls = crate::loopseal::LoopSeal::new();
        let ok = match ls.push(&ChainLink {
            hash: "H1".into(),
            previous_hash: GENESIS.into(),
        }) {
            LoopStatus::New { .. } => true,
            LoopStatus::Loop { .. } => false,
        };
        assert!(ok);
    }

    #[test]
    fn json_pretty_roundtrip() {
        let (i, s, l) = sound_assurance();
        let r = CoherenceReport::new(0.9517, 1.0, 0.96, i, s, l);
        let json = report_to_json_pretty(&r);
        let parsed: CoherenceReport = serde_json::from_str(&json).expect("roundtrip");
        assert_eq!(parsed.phi, r.phi);
        assert_eq!(parsed.acceptable, r.acceptable);
    }

    #[test]
    fn property_phi_bounded_unit_interval_grid() {
        // Boundedness (7.4): Φ ∈ [0, 1] para toda a grade de componentes e,
        // em conformidade com o Gap-1, só é `acceptable` dentro da faixa.
        let grid = [0.0, 0.1, 0.5, 0.9, 1.0];
        let sound = sound_assurance();
        for &a in &grid {
            for &b in &grid {
                for &c in &grid {
                    let r = CoherenceReport::new(a, b, c, sound.0.clone(), sound.1.clone(), sound.2.clone());
                    assert!(
                        (0.0..=1.0).contains(&r.phi),
                        "Φ={} fora de [0,1] para ({a},{b},{c})",
                        r.phi
                    );
                    // O veredito de `acceptable` deve coincidir com Gap-1 (não
                    // deve depender dos componentes fora da faixa).
                    assert_eq!(r.acceptable, gap1_satisfied(r.phi), "({a},{b},{c})");
                }
            }
        }
    }

    #[test]
    fn property_gap1_satisfied_monotone_upward_band() {
        // Monotonicidade (7.4): Gap-1 é uma faixa — Φ crescente que entra na
        // banda torna-se acceptable e permanece até a borda superior.
        let banda = [0.6, 0.7, 0.8, 0.9, 0.99];
        for &phi in &banda {
            assert!(gap1_satisfied(phi), "Φ={phi} deveria estar na banda");
        }
        // Acima da borda superior deixa de ser aceite (Gap-1 exclusivo no topo).
        assert!(!gap1_satisfied(0.99991));
        assert!(!gap1_satisfied(1.0));
    }
}