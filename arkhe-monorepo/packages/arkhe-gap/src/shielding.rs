//! Selo #6 — Blindagem Coulombiana ativa (Phys. Rev. X 16, 031040, 2026).
//!
//! Barrier et al. (Manchester) suprimiram **completamente** a supercondutividade
//! e o estado isolante correlacionado do grafeno de ângulo mágico (~1.1°)
//! aumentando a densidade de portadores numa bicamada torcida adjacente
//! eletronicamente desacoplada (twist de ~10°). A eficiência da blindagem decresce
//! exponencialmente na escala ~2 nm; com separação subnanométrica a blindagem é
//! máxima: o pareamento não convencional (plasmônico) desaba.
//!
//! Aqui isso vira o **controle ativo do gap C-Z**: se o defeito `ε` do Selo #2
//! excede o limiar, a camada de blindagem é engatada (distância < 1 nm) para
//! rebaixar `ε` abaixo do regime estável.

use serde::{Deserialize, Serialize};

/// Referência do Selo #6 (PRX).
pub const COULOMB_SCREEN_REF: &str = "J. Barrier, L. Peng, S. Xu, C. De Beule, V. I. Fal'ko, \
K. Watanabe, T. Taniguchi, A. K. Geim, S. Adam, A. I. Berdyugin, 'Coulomb Screening of \
Superconductivity in Magic-Angle Graphene', Phys. Rev. X 16, 031040 (2026)";
/// Referência do ângulo mágico do TBG.
pub const TBG_SCREEN_REF: &str = "Y. Cao et al., 'Unconventional superconductivity in \
magic-angle graphene superlattices', Nature 556, 43 (2018)";
/// Ângulo mágico do grafeno bicamada torcido (~1.1°).
pub const MAGIC_ANGLE_TBG_DEG: f64 = 1.1;
/// Ângulo de rotação ARKHE entre camadas (360°/21) usado como par análogo ao twist.
pub const ARKHE_TWIST_DEG: f64 = 360.0 / 21.0;
/// Distância primária de blindagem (subnanométrica, eficiência máxima).
pub const PRIMARY_SCREENING_DIST_NM: f64 = 1.0;
/// Escala de decaimento exponencial da blindagem (~2 nm).
pub const SCREENING_DECAY_SCALE_NM: f64 = 2.0;

/// Fator de blindagem normalizado: `exp(−d/2 nm)` — máximo em contato, decai
/// exponencialmente com a distância (modelo do Selo #6).
pub fn shielding_factor(distance_nm: f64) -> f64 {
    (-distance_nm / SCREENING_DECAY_SCALE_NM).exp()
}

/// A separação suprime supercondutividade e estado isolante correlacionado?
/// Máxima eficiência exige distância subnanométrica (Selo #6: decoupling a 10°,
/// sem espaçadores de h-BN, distância < 1 nm).
pub fn is_suppressive_distance(distance_nm: f64) -> bool {
    distance_nm < PRIMARY_SCREENING_DIST_NM && shielding_factor(distance_nm) > 0.6
}

/// O gap precisa de blindagem ativa? (ε ≥ limiar de estabilidade do Selo #2).
pub fn requires_active_shielding(epsilon: f64, threshold: f64) -> bool {
    epsilon >= threshold
}

/// Protocolo de blindagem ativa do gap C-Z.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShieldingProtocol {
    pub distance_nm: f64,
    pub shielded: bool,
    pub requires_shielding: bool,
    pub eps_before: f64,
    pub eps_after: f64,
    pub threshold: f64,
    pub stable_after: bool,
}

impl ShieldingProtocol {
    /// Engata a blindagem sobre o erro `epsilon` do gap sempre que
    /// `epsilon ≥ threshold`. A blindagem atenua o defeito por um fator
    /// `1 − shielding²` (redução pela supressão da interação correlacionada).
    pub fn engage(epsilon: f64, threshold: f64) -> Self {
        let need = requires_active_shielding(epsilon, threshold);
        let (distance_nm, shielded, eps_after) = if need {
            let d = PRIMARY_SCREENING_DIST_NM * 0.5;
            let s = shielding_factor(d);
            let eps_after = epsilon * (1.0 - s * s).max(0.0);
            (d, true, eps_after)
        } else {
            (PRIMARY_SCREENING_DIST_NM * 2.0, false, epsilon)
        };
        ShieldingProtocol {
            distance_nm,
            shielded,
            requires_shielding: need,
            eps_before: epsilon,
            eps_after,
            threshold,
            stable_after: eps_after < threshold,
        }
    }
}

/// Descrição da camada de blindagem (Selo #6) para auditoria.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct CoulombShield {
    pub distance_nm: f64,
    pub factor: f64,
    pub suppresses: bool,
}

/// Insight da camada: `(distância, fator, suprime)` a qualquer separação.
pub fn coulomb_layer(distance_nm: f64) -> CoulombShield {
    CoulombShield {
        distance_nm,
        factor: shielding_factor(distance_nm),
        suppresses: is_suppressive_distance(distance_nm),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extremal::STABILITY_EPS_8_QU4;

    #[test]
    fn subnanometric_screening_is_maximal() {
        let d = PRIMARY_SCREENING_DIST_NM * 0.5;
        assert!(shielding_factor(d) > 0.7);
        assert!(is_suppressive_distance(d));
        assert!(!is_suppressive_distance(5.0), "5nm: blindagem ineficiente");
    }

    #[test]
    fn exponential_decay_on_2nm_scale() {
        assert!((shielding_factor(PRIMARY_SCREENING_DIST_NM) - (-0.5f64).exp()).abs() < 1e-9);
        let far = shielding_factor(4.0);
        assert!(far < 0.2, "4nm já enfraqueceu a blindagem");
    }

    #[test]
    fn active_shielding_recovers_stability() {
        // ε = 0.3 > 1/5: instável sem blindagem; engatada, ε cai abaixo do limiar.
        let p = ShieldingProtocol::engage(0.3, STABILITY_EPS_8_QU4);
        assert!(p.requires_shielding);
        assert!(p.shielded);
        assert!(p.stable_after, "ε_after = {} < 0.2", p.eps_after);
    }

    #[test]
    fn no_shielding_when_already_stable() {
        let p = ShieldingProtocol::engage(0.1, STABILITY_EPS_8_QU4);
        assert!(!p.requires_shielding);
        assert!(!p.shielded);
        assert!(p.stable_after);
    }

    #[test]
    fn arkhe_twist_mirrors_magic_angle() {
        assert!((ARKHE_TWIST_DEG - 17.14).abs() < 0.01);
        assert_eq!(crate::extremal::QEX_8_4_PLATEAU, 56);
    }
}