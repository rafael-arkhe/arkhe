//! Número de Skyrmion (topological charge) `Nsk` sobre um campo vetorial 2D normalizado.
//!
//! Módulo #8 da pesquisa ARKHE-VSEPR. O invariante é calculado por integração numérica:
//! `Nsk = (1/4π) ∬ m · (∂m/∂x × ∂m/∂y) dxdy`, com `m` o campo magnético unitário.
//! Campos topologicamente coerentes produzem `Nsk` inteiro (dentro de tolerância).

use serde::{Deserialize, Serialize};

const TOLERANCE: f64 = 0.25;

/// Campo vetorial magnético 2D normalizado usado no cálculo do invariante.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkyrmionField {
    /// Grade `n × n` de vetores unitários `(mx, my, mz)`, ordem row-major.
    pub grid: Vec<[f64; 3]>,
    pub size: usize,
}

impl SkyrmionField {
    /// Constrói um campo a partir de uma grade quadrada de vetores 3D.
    pub fn new(grid: Vec<[f64; 3]>, size: usize) -> Self {
        assert_eq!(grid.len(), size * size, "grade deve ser quadrada n×n");
        Self { grid, size }
    }

    /// Normaliza todos os vetores para norma unitária.
    pub fn normalized(&self) -> Vec<[f64; 3]> {
        self.grid
            .iter()
            .map(|[x, y, z]| {
                let n = (x * x + y * y + z * z).sqrt();
                if n < 1e-12 {
                    [0.0, 0.0, 0.0]
                } else {
                    [x / n, y / n, z / n]
                }
            })
            .collect()
    }

    /// Calcula o número de Skyrmion `Nsk` por diferenças finitas centradas.
    ///
    /// Diferenças centradas `m[i+1] − m[i−1]` superestimam a derivada por 2 em
    /// cada eixo (produto por 4); sobre a grade unitária de `n²` células a forma
    /// discreta converte `Σ m·(∂x m × ∂y m) / 16π` na carga topológica inteira.
    pub fn skyrmion_number(&self) -> f64 {
        let m = self.normalized();
        let n = self.size;
        let mut sum = 0.0;
        for ix in 0..n {
            for iy in 0..n {
                let xm = if ix == 0 { 0 } else { ix - 1 };
                let xp = if ix == n - 1 { n - 1 } else { ix + 1 };
                let ym = if iy == 0 { 0 } else { iy - 1 };
                let yp = if iy == n - 1 { n - 1 } else { iy + 1 };
                let dmx = sub(m[xp * n + iy], m[xm * n + iy]);
                let dmy = sub(m[ix * n + yp], m[ix * n + ym]);
                sum += dot(m[ix * n + iy], cross(dmx, dmy));
            }
        }
        sum / (16.0 * std::f64::consts::PI)
    }

    /// Critério de validade (Gap-1): `Nsk` deve ser inteiro dentro de tolerância.
    pub fn is_valid_charge(&self) -> bool {
        let nsk = self.skyrmion_number();
        (nsk - nsk.round()).abs() <= TOLERANCE
    }
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

#[cfg(test)]
mod tests {
    use super::*;

/// Um skyrmion radial (Néel) com perfil que satura em +z fora do disco de raio R:
/// `mz = cos(θ)`, `m_perp = sin(θ)·r̂`, com `θ = π·(1 − u)`, `u = r/R`. Essa
/// configuração tem carga topológica exata Q = 1 no contínuo (o gradiente anula
/// fora do disco, já que o campo está totalmente saturado).
fn neel_skyrmion(n: usize) -> SkyrmionField {
    let c = (n as f64 - 1.0) / 2.0;
    let r_max = n as f64 * 0.45; // disco dentro do grid, com borda saturada
    let mut grid = Vec::with_capacity(n * n);
    for iy in 0..n {
        for ix in 0..n {
            let gx = ix as f64 - c;
            let gy = iy as f64 - c;
            let r = (gx * gx + gy * gy).sqrt();
            let u = (r / r_max).min(1.0);
            let theta = std::f64::consts::PI * (1.0 - u);
            let (mz, in_plane) = (theta.cos(), theta.sin());
            let nx = if r > 1e-12 { gx / r } else { 0.0 };
            let ny = if r > 1e-12 { gy / r } else { 0.0 };
            grid.push([nx * in_plane, ny * in_plane, mz]);
        }
    }
    SkyrmionField::new(grid, n)
}

    #[test]
    fn neel_skyrmion_has_integer_charge() {
        let sf = neel_skyrmion(64);
        let nsk = sf.skyrmion_number();
        assert!(sf.is_valid_charge(), "Nsk={nsk} deve ser ~inteiro");
        assert!((nsk - 1.0).abs() < 0.3, "skyrmion Néel deve carregar Q=1, obteve {nsk}");
    }

    #[test]
    fn uniform_field_has_zero_charge() {
        let grid = vec![[0.0f64, 0.0, 1.0]; 32 * 32];
        let sf = SkyrmionField::new(grid, 32);
        assert!(sf.skyrmion_number().abs() < 0.1);
    }
}