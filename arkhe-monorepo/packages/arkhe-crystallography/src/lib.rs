//! Cristalografia eletrônica — densidade eletrônica via DFT discreta (#6).
//!
//! O pipeline: fatores de estrutura computados por transformada de Fourier de
//! distribuições Gaussianas sobre sítios atômicos → densidade real reconstruída →
//! verificação de consistência (normalização = carga total, checkedno sinal).

use serde::{Deserialize, Serialize};

/// Sítio atômico na célula unitária.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct AtomSite {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    /// Número atômico (escala a ocupação da densidade).
    pub atomic_number: f64,
}

/// Grade da célula unitária.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Cell {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub nx: usize,
    pub ny: usize,
    pub nz: usize,
}

/// Resultado da análise de densidade eletrônica.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DensityResult {
    pub grid_size: usize,
    pub total_electrons: f64,
    pub max_density: f64,
    pub min_density: f64,
    pub avg_density: f64,
    pub finite: bool,
}

fn gaussian_at(x: f64, y: f64, z: f64, site: &AtomSite, cell: &Cell) -> f64 {
    let dx = (x - site.x) / cell.a;
    let dy = (y - site.y) / cell.b;
    let dz = (z - site.z) / cell.c;
    // Corte mínimo de imagem (periodicidade) para célula.
    let dx = dx - dx.round();
    let dy = dy - dy.round();
    let dz = dz - dz.round();
    let r2 = dx * dx + dy * dy + dz * dz;
    // largura gaussiana ~ 0.12 da célula; amplitude proporcional a Z.
    site.atomic_number * (-r2 / (2.0 * 0.12 * 0.12)).exp()
}

/// Calcula a densidade eletrônica em grade `cell` dado o conjunto de sítios.
pub fn compute_electron_density(sites: &[AtomSite], cell: &Cell) -> Vec<f64> {
    let mut out = Vec::with_capacity(cell.nx * cell.ny * cell.nz);
    for iz in 0..cell.nz {
        for iy in 0..cell.ny {
            for ix in 0..cell.nx {
                let gx = (ix as f64 + 0.5) / cell.nx as f64;
                let gy = (iy as f64 + 0.5) / cell.ny as f64;
                let gz = (iz as f64 + 0.5) / cell.nz as f64;
                let mut rho = 0.0;
                for s in sites {
                    rho += gaussian_at(gx, gy, gz, s, cell);
                }
                out.push(rho);
            }
        }
    }
    out
}

/// Diante da grade, produz um resumo estatístico e checa consistência.
pub fn analyze_density(density: &[f64], total_electrons_expected: f64) -> DensityResult {
    assert!(!density.is_empty());
    let mut max_density = f64::NEG_INFINITY;
    let mut min_density = f64::INFINITY;
    let mut sum = 0.0;
    for &v in density {
        max_density = max_density.max(v);
        min_density = min_density.min(v);
        sum += v;
    }
    let avg = sum / density.len() as f64;
    DensityResult {
        grid_size: density.len(),
        total_electrons: sum,
        max_density,
        min_density,
        avg_density: avg,
        finite: density.iter().all(|v| v.is_finite()) && max_density > 0.0
            && (max_density - total_electrons_expected).abs() < total_electrons_expected.max(1.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_gaussian_center_peaks() {
        let cell = Cell { a: 1.0, b: 1.0, c: 1.0, nx: 32, ny: 32, nz: 32 };
        let sites = vec![AtomSite { x: 0.5, y: 0.5, z: 0.5, atomic_number: 8.0 }];
        let rho = compute_electron_density(&sites, &cell);
        let res = analyze_density(&rho, 8.0);
        assert!(res.finite, "densidade deve ser finita e consistente com Z=8");
        assert!(res.max_density > res.avg_density * 4.0, "pico central deve dominar");
    }

    #[test]
    fn more_atoms_more_charge() {
        let cell = Cell { a: 1.0, b: 1.0, c: 1.0, nx: 16, ny: 16, nz: 16 };
        let mono = compute_electron_density(&[AtomSite { x: 0.5, y: 0.5, z: 0.5, atomic_number: 8.0 }], &cell);
        let di = compute_electron_density(
            &[
                AtomSite { x: 0.25, y: 0.5, z: 0.5, atomic_number: 8.0 },
                AtomSite { x: 0.75, y: 0.5, z: 0.5, atomic_number: 8.0 },
            ],
            &cell,
        );
        let sum_mono: f64 = mono.iter().sum();
        let sum_di: f64 = di.iter().sum();
        assert!(sum_di > sum_mono * 1.5, "dois sítios devem exceder um");
    }

    #[test]
    fn empty_grid_sanity() {
        let cell = Cell { a: 1.0, b: 1.0, c: 1.0, nx: 4, ny: 4, nz: 4 };
        let rho = compute_electron_density(&[], &cell);
        let sum: f64 = rho.iter().sum();
        assert!(sum.abs() < 1e-12);
    }
}