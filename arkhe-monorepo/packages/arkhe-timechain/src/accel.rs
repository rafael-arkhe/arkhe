//! Hardware acceleration interface for the field-op hot path.
//!
//! The two heaviest primitives — the MHD helicity (`Jacobi` relaxation) and the
//! Shadow SVD — are exposed behind a trait so a GPU (CUDA/OpenCL) or FPGA kernel
//! can be dropped in without touching the peers. A pure-Rust `CpuAccelerator`
//! is provided as a default (and as the reference implementation for `cargo
//! test`).

use arkhe_mhd::{helicity, EvoField};
use arkhe_shadow::Shadow;

/// Accelerated kernel backends available on this build.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum Acceleration {
    /// Pure-Rust reference (the only one guaranteed to build anywhere).
    #[default]
    Cpu,
    /// JIT GPU kernels (CUDA/OpenCL) — requires a native accelerator crate.
    Gpu,
    /// Reconfigurable-logic data path on a connected FPGA.
    Fpga,
}

/// Abstractions for the two hot kernels.
pub trait FieldAccelerator {
    /// Which hardware is in play.
    fn acceleration(&self) -> Acceleration;

    /// Topological helicity (`Chern–Simons`) of a field.
    fn helicity(&self, field: &EvoField, iterations: usize) -> f64 {
        let dx = field.config.dx();
        helicity(
            (&field.omega_x, &field.omega_y, &field.omega_z),
            dx,
            iterations,
        )
    }

    /// SVD tail of a matrix — the visible low-rank head + the Shadow tail.
    ///
    /// Offloaded to hardware when `acceleration != Cpu`.
    fn split_shadow(&self, density: &nalgebra::DMatrix<f64>, cut: usize) -> Shadow;
}

/// The pure-Rust reference accelerator.
#[derive(Debug, Clone, Copy)]
pub struct CpuAccelerator;

impl FieldAccelerator for CpuAccelerator {
    fn acceleration(&self) -> Acceleration {
        Acceleration::Cpu
    }

    fn split_shadow(&self, density: &nalgebra::DMatrix<f64>, cut: usize) -> Shadow {
        let svd = density.clone().svd(true, true);
        let (_, shadow) = Shadow::split(
            &svd.u.expect("left basis"),
            &svd.v_t.expect("right basis"),
            &svd.singular_values,
            cut,
        );
        shadow
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arkhe_mhd::PlasmaConfig;

    #[test]
    fn cpu_accelerator_is_the_default_reference() {
        let accel = CpuAccelerator;
        assert_eq!(accel.acceleration(), Acceleration::Cpu);
        let config = PlasmaConfig::new(8, 6, 1.0, 0.7, 1e-3).unwrap();
        let field = EvoField::with_noise(config, 42);
        let h = accel.helicity(&field, 5);
        assert!(h.is_finite());
    }

    #[test]
    fn cpu_svd_shadow_produces_a_tail() {
        use rand::rngs::StdRng;
        use rand::{Rng, SeedableRng};
        let mut rng = StdRng::seed_from_u64(1);
        let a: nalgebra::DMatrix<f64> =
            nalgebra::DMatrix::from_fn(20, 20, |_, _| rng.gen::<f64>());
        let accel = CpuAccelerator;
        let shadow = accel.split_shadow(&a, 4);
        assert_eq!(shadow.len(), 20 - 4);
        assert!(shadow.energy_frac > 0.0);
    }
}