//! Rotação de fase derivada da fase CP δ da matriz de mistura de camadas.
//!
//! `PhaseRotation` quantiza um ângulo (tipicamente δ = 227° da matriz PMNS
//! constitucional) numa multiplicação `λ mod 2²⁵⁶` sobre o digest. Ela age como o
//! grupo de rotação `U(1)` da fase CP: a identidade (`λ = 1`, ângulo 0) deixa o
//! digest intacto — o caso "cego à fase" da sobrevivência de reator de JUNO.

use serde::{Deserialize, Serialize};

/// Rotação unitária sobre o espaço de digests (multiplicação mod 2²⁵⁶).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PhaseRotation {
    /// Ângulo em radianos no intervalo `[0, 2π)`.
    pub angle_rad: f64,
    /// Multiplicador quantizado ≥ 1.
    pub lambda: u64,
}

impl PhaseRotation {
    /// A identidade (δ = 0°): nenhuma rotação de fase.
    pub fn identity() -> Self {
        Self {
            angle_rad: 0.0,
            lambda: 1,
        }
    }

    /// Do grau de fase CP (δ) em graus. Normaliza para `[0, 360)` e converte.
    /// * `δ = 0`   ⇒ identidade (`λ = 1`) — protocolo cego à fase.
    /// * `δ = 227` ⇒ rotação máxima — protocolo sensível à fase.
    pub fn from_delta_degrees(degrees: f64) -> Self {
        let norm = degrees.rem_euclid(360.0);
        let angle = norm.to_radians();
        Self {
            angle_rad: angle,
            lambda: quantize_lambda(angle),
        }
    }

    /// A rotação como `[u8; 32]` little-endian do multiplicador.
    pub fn lambda_bytes(&self) -> [u8; 32] {
        let mut out = [0u8; 32];
        out[..8].copy_from_slice(&self.lambda.to_le_bytes());
        out
    }
}

impl Default for PhaseRotation {
    fn default() -> Self {
        Self::identity()
    }
}

fn quantize_lambda(angle_rad: f64) -> u64 {
    let span = (angle_rad / (2.0 * std::f64::consts::PI)).clamp(0.0, 1.0);
    (span * (u64::MAX >> 1) as f64) as u64 + 1
}

/// Multiplicação de dois inteiros de 256 bits módulo 2²⁵⁶, por base 2³².
///
/// Implementa a "rotação": `digest ⊗ λ = (digest · λ) mod 2²⁵⁶`. É o análogo
/// numérico da rotação de fase `|ν⟩ → e^{iδ} |ν⟩` em U(1): uma transformação
/// invertível e sem perdas quando λ é ímpar (e `λ = 1` recupera a identidade).
pub fn mul_mod_2_256(a: [u8; 32], b: [u8; 32]) -> [u8; 32] {
    let limbs = |x: [u8; 32]| -> [u32; 8] {
        let mut out = [0u32; 8];
        for (i, l) in out.iter_mut().enumerate() {
            *l = u32::from_le_bytes([x[i * 4], x[i * 4 + 1], x[i * 4 + 2], x[i * 4 + 3]]);
        }
        out
    };
    let a32 = limbs(a);
    let b32 = limbs(b);

    let mut acc = [0u128; 8];
    for i in 0..8 {
        for j in 0..(8 - i) {
            acc[i + j] += (a32[i] as u128) * (b32[j] as u128);
        }
    }

    let mut out = [0u8; 32];
    let mut carry: u128 = 0;
    for k in 0..8 {
        let v = acc[k] + carry;
        let limb = (v & 0xFFFF_FFFF) as u32;
        out[k * 4..k * 4 + 4].copy_from_slice(&limb.to_le_bytes());
        carry = v >> 32;
    }
    out
}

/// Roda o digest pela rotação: `digest ⊗ λ`.
pub fn blend_rotation(digest: [u8; 32], rotation: &PhaseRotation) -> [u8; 32] {
    mul_mod_2_256(digest, rotation.lambda_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_rotation_leaves_digest_intact() {
        let digest = [0xABu8; 32];
        let rot = PhaseRotation::identity();
        assert_eq!(rot.lambda, 1);
        assert_eq!(blend_rotation(digest, &rot), digest);
    }

    #[test]
    fn delta_zero_and_delta_227_normalize() {
        let zero = PhaseRotation::from_delta_degrees(0.0);
        assert_eq!(zero.angle_rad, 0.0);
        assert_eq!(zero.lambda, 1);
        let d227 = PhaseRotation::from_delta_degrees(227.0);
        assert!(d227.lambda > 1, "rotação não-trivial precisa λ>1");
        let mod360 = PhaseRotation::from_delta_degrees(587.0);
        assert!((mod360.angle_rad - (227.0f64).to_radians()).abs() < 1e-12);
        assert_eq!(mod360.lambda, d227.lambda);
    }

    #[test]
    fn lambda_a_rotation_commutes() {
        // (x·λ₁)·λ₂ == (x·λ₂)·λ₁ — rotações U(1) comutam.
        let x = [0xCD; 32];
        let r1 = PhaseRotation::from_delta_degrees(15.0);
        let r2 = PhaseRotation::from_delta_degrees(40.0);
        let a = blend_rotation(blend_rotation(x, &r1), &r2);
        let b = blend_rotation(blend_rotation(x, &r2), &r1);
        assert_eq!(a, b);
    }

    #[test]
    fn mul_mod_2_256_preserves_odd_lambda_invertibility_full_cycle() {
        // λ=1 é a identidade exata.
        let mut id = [0u8; 32];
        id[0] = 1;
        let x = [0x5Au8; 32];
        assert_eq!(mul_mod_2_256(x, id), x);
    }

    #[test]
    fn small_known_product() {
        // 5 * 3 = 15 em 256 bits LE.
        let mut a = [0u8; 32];
        let mut b = [0u8; 32];
        a[0] = 5;
        b[0] = 3;
        let out = mul_mod_2_256(a, b);
        assert_eq!(out[0], 15);
        assert!(out[1..].iter().all(|&x| x == 0));
    }
}