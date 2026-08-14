//! Resistive pseudo-MHD phase-field engine and its topological diagnostics.
//!
//! The phase field `Ω` plays the role of `B` in the plasma analogy and evolves
//! under the resistive induction law:
//!
//! ```text
//! ∂Ω/∂t = ∇×(u×Ω) + ν ∇²Ω
//! ```
//!
//! Implemented fixes from the Block-1 audit:
//!
//! 1. `advance` builds the cross product `u×Ω` over the **whole** grid first and
//!    only then takes the curl — the original code indexed `f64` scalars as
//!    arrays and would not compile.
//! 2. `helicity` returns the genuine magnetic-type helicity `∫ A·Ω dV`, with the
//!    vector potential `A` recovered from `∇²A = −J` (Coulomb gauge) by Jacobi
//!    relaxation — the prior `Ω·J` expression was not a helicity.

use ndarray::Array2;

use crate::error::{diffusive_cfl_dt, MhdError};

pub type Grid = Array2<f64>;

/// Static topology/plasma parameters of the phase field.
#[derive(Debug, Clone)]
pub struct PlasmaConfig {
    /// Nodes along `x`.
    pub nx: usize,
    /// Nodes along `y`.
    pub ny: usize,
    /// Domain extent along `x`.
    pub lx: f64,
    /// Domain extent along `y`.
    pub ly: f64,
    /// Resistivity `ν` (diffusion coefficient).
    pub nu: f64,
}

impl PlasmaConfig {
    /// Build a uniform domain. Requires `nx, ny >= 3` for central differences.
    pub fn new(nx: usize, ny: usize, lx: f64, ly: f64, nu: f64) -> Result<Self, MhdError> {
        if nx < 3 || ny < 3 {
            return Err(MhdError::GridTooSmall { nx, ny });
        }
        Ok(Self { nx, ny, lx, ly, nu })
    }

    /// Cell spacing along `x`.
    pub fn dx(&self) -> f64 {
        self.lx / (self.nx as f64)
    }

    /// Cell spacing along `y`.
    pub fn dy(&self) -> f64 {
        self.ly / (self.ny as f64)
    }

    /// Explicit-Euler stability ceiling for diffusion: `dt ≤ h²/(2ν)`.
    pub fn max_stable_dt(&self) -> f64 {
        let h = self.dx().min(self.dy()).max(f64::EPSILON);
        diffusive_cfl_dt(h, self.nu)
    }
}

/// The evolving pseudo-magnetic (`Ω`) state plus the correlate velocity `u`.
#[derive(Debug, Clone)]
pub struct EvoField {
    /// Geometry / transport parameters.
    pub config: PlasmaConfig,
    /// `Ω_x` grid `nx×ny`.
    pub omega_x: Grid,
    /// `Ω_y` grid `nx×ny`.
    pub omega_y: Grid,
    /// `Ω_z` grid `nx×ny`.
    pub omega_z: Grid,
    /// Velocity `u_x` grid `nx×ny`.
    pub ux: Grid,
    /// Velocity `u_y` grid `nx×ny` (`u_z = 0` assumed).
    pub uy: Grid,
}

impl EvoField {
    /// A uniformly zero state.
    pub fn zeros(config: PlasmaConfig) -> Self {
        let (nx, ny) = (config.nx, config.ny);
        Self {
            config,
            omega_x: Array2::zeros((nx, ny)),
            omega_y: Array2::zeros((nx, ny)),
            omega_z: Array2::zeros((nx, ny)),
            ux: Array2::zeros((nx, ny)),
            uy: Array2::zeros((nx, ny)),
        }
    }

    /// Build from explicit component arrays (shapes are validated).
    pub fn from_arrays(
        config: PlasmaConfig,
        omega_x: Grid,
        omega_y: Grid,
        omega_z: Grid,
        ux: Grid,
        uy: Grid,
    ) -> Result<Self, MhdError> {
        let expect = [config.nx, config.ny];
        for (name, a) in [
            ("omega_x", &omega_x),
            ("omega_y", &omega_y),
            ("omega_z", &omega_z),
            ("ux", &ux),
            ("uy", &uy),
        ] {
            if a.shape() != expect {
                return Err(MhdError::GridTooSmall {
                    nx: a.shape()[0],
                    ny: a.shape()[1],
                });
            }
            let _ = name;
        }
        Ok(Self {
            config,
            omega_x,
            omega_y,
            omega_z,
            ux,
            uy,
        })
    }

    /// A deterministic low-amplitude random state (seeded `StdRng`).
    ///
    /// The `rand::Rng` trait is imported **here**, at module scope, so that the
    /// `rng.gen::<f64>()` call compiles — this was the second documented Bug 1
    /// issue (the trait was not in scope).
    pub fn with_noise(config: PlasmaConfig, seed: u64) -> Self {
        use rand::rngs::StdRng;
        use rand::{Rng, SeedableRng};
        let mut rng = StdRng::seed_from_u64(seed);
        let (nx, ny) = (config.nx, config.ny);
        let mut sample = |_r: usize, _c: usize| rng.gen::<f64>() * 1e-3;
        Self::from_arrays(
            config,
            Array2::from_shape_fn((nx, ny), |(r, cc)| sample(r, cc)),
            Array2::from_shape_fn((nx, ny), |(r, cc)| sample(r, cc)),
            Array2::from_shape_fn((nx, ny), |(r, cc)| sample(r, cc)),
            Array2::from_shape_fn((nx, ny), |(r, cc)| sample(r, cc)),
            Array2::from_shape_fn((nx, ny), |(r, cc)| sample(r, cc)),
        )
        .expect("grid is always >= 3x3 in a constructed formula")
    }

    /// One explicit-Euler step of `∂Ω/∂t = ∇×(u×Ω) + ν∇²Ω`.
    ///
    /// Corrected Block 21 changed: the nested-loop advection previously indexed
    /// scalars; building `u×Ω` grid-wide (Phase A) and then curling it in
    /// (Phase B) is both correct and simple. Interior cells only, zero at the
    /// halo (Dirichlet guard).
    pub fn advance(&mut self, dt: f64) {
        let nx = self.config.nx;
        let ny = self.config.ny;
        let (dx, dy) = (self.config.dx(), self.config.dy());
        let nu = self.config.nu;

        // Phase A: cross product u×Ω on the full grid (u_z assumed 0).
        let mut cx = Array2::zeros((nx, ny));
        let mut cy = Array2::zeros((nx, ny));
        let mut cz = Array2::zeros((nx, ny));
        for i in 0..nx {
            for j in 0..ny {
                cx[(i, j)] = self.uy[(i, j)] * self.omega_z[(i, j)];
                cy[(i, j)] = -self.ux[(i, j)] * self.omega_z[(i, j)];
                cz[(i, j)] =
                    self.ux[(i, j)] * self.omega_y[(i, j)] - self.uy[(i, j)] * self.omega_x[(i, j)];
            }
        }

        // Phase B: curl of the cross product + diffusion, applied to Ω.
        let mut adv_x;
        let mut adv_y;
        let mut adv_z;

        for i in 1..nx - 1 {
            for j in 1..ny - 1 {
                adv_x = (cz[(i, j + 1)] - cz[(i, j - 1)]) / (2.0 * dy);
                let diff_x = laplacian(&self.omega_x, i, j, dx);
                self.omega_x[(i, j)] += dt * (adv_x + nu * diff_x);

                adv_y = -(cz[(i + 1, j)] - cz[(i - 1, j)]) / (2.0 * dx);
                let diff_y = laplacian(&self.omega_y, i, j, dx);
                self.omega_y[(i, j)] += dt * (adv_y + nu * diff_y);

                adv_z = (cy[(i + 1, j)] - cy[(i - 1, j)]) / (2.0 * dx)
                    - (cx[(i, j + 1)] - cx[(i, j - 1)]) / (2.0 * dy);
                let diff_z = laplacian(&self.omega_z, i, j, dx);
                self.omega_z[(i, j)] += dt * (adv_z + nu * diff_z);
            }
        }
    }

    /// Current density `J = ∇×Ω`.
    pub fn current(&self) -> (Grid, Grid, Grid) {
        let (nx, ny) = (self.config.nx, self.config.ny);
        let (dx, dy) = (self.config.dx(), self.config.dy());
        let mut jx = Array2::zeros((nx, ny));
        let mut jy = Array2::zeros((nx, ny));
        let mut jz = Array2::zeros((nx, ny));
        for i in 1..nx - 1 {
            for j in 1..ny - 1 {
                jx[(i, j)] = (self.omega_z[(i, j + 1)] - self.omega_z[(i, j - 1)]) / (2.0 * dy)
                    - (self.omega_y[(i + 1, j)] - self.omega_y[(i - 1, j)]) / (2.0 * dx);
                jy[(i, j)] = (self.omega_x[(i + 1, j)] - self.omega_x[(i - 1, j)]) / (2.0 * dx)
                    - (self.omega_z[(i, j + 1)] - self.omega_z[(i, j - 1)]) / (2.0 * dy);
                jz[(i, j)] = (self.omega_y[(i + 1, j)] - self.omega_y[(i - 1, j)]) / (2.0 * dx)
                    - (self.omega_x[(i, j + 1)] - self.omega_x[(i, j - 1)]) / (2.0 * dy);
            }
        }
        (jx, jy, jz)
    }

    /// Peak `|J|` over the interior — a tearing-mode growth marker.
    pub fn max_current(&self) -> f64 {
        let (jx, jy, jz) = self.current();
        let mut m = 0.0;
        for i in 0..self.config.nx {
            for j in 0..self.config.ny {
                let n =
                    (jx[(i, j)] * jx[(i, j)] + jy[(i, j)] * jy[(i, j)] + jz[(i, j)] * jz[(i, j)])
                        .sqrt();
                if n > m {
                    m = n;
                }
            }
        }
        m
    }

    /// Total pseudo-magnetic energy `Σ|Ω|²`.
    pub fn energy(&self) -> f64 {
        let mut e = 0.0;
        for i in 0..self.config.nx {
            for j in 0..self.config.ny {
                e += self.omega_x[(i, j)] * self.omega_x[(i, j)]
                    + self.omega_y[(i, j)] * self.omega_y[(i, j)]
                    + self.omega_z[(i, j)] * self.omega_z[(i, j)];
            }
        }
        e
    }

    /// True if every state value is finite (didn't blow up).
    pub fn is_finite(&self) -> bool {
        [&self.omega_x, &self.omega_y, &self.omega_z]
            .into_iter()
            .all(|v| v.iter().all(|x| x.is_finite()))
    }
}

/// Discrete Laplacian of a single component at interior `(i, j)`.
fn laplacian(f: &Grid, i: usize, j: usize, h: f64) -> f64 {
    (f[(i + 1, j)] + f[(i - 1, j)] + f[(i, j + 1)] + f[(i, j - 1)] - 4.0 * f[(i, j)]) / (h * h)
}

/// True magnetic-type helicity `H = ∫ A·Ω dV`.
///
/// Recover the vector potential `A` from the Coulomb-gauge Poisson equation
/// `∇²A = −J` with `iterations` Jacobi relaxations per component, then sum
/// `A·Ω`. Unlike `∫Ω·J`, this quantity is a genuine measure of topological
/// linking structure (knottedness / parity) and is preserved under ideal
/// (ν→0) transport — the Ledger's "Chern–Simons" invariant.
pub fn helicity(
    omega: (&Grid, &Grid, &Grid),
    dx: f64,
    iterations: usize,
) -> f64 {
    let (ox, oy, oz) = omega;
    let nx = ox.shape()[0];
    let ny = ox.shape()[1];
    let h = dx;

    let jacobi = |src: &Grid| -> Grid {
        let mut a = Array2::zeros((nx, ny));
        for _ in 0..iterations {
            let prev = a.clone();
            for i in 1..nx - 1 {
                for j in 1..ny - 1 {
                    a[(i, j)] = (prev[(i + 1, j)]
                        + prev[(i - 1, j)]
                        + prev[(i, j + 1)]
                        + prev[(i, j - 1)]
                        + h * h * src[(i, j)])
                        / 4.0;
                }
            }
        }
        a
    };

    let (jx, jy, jz) = curl(ox, oy, oz, dx, h);
    let ax = jacobi(&jx);
    let ay = jacobi(&jy);
    let az = jacobi(&jz);

    let mut hh = 0.0;
    for i in 0..nx {
        for j in 0..ny {
            hh += ax[(i, j)] * ox[(i, j)] + ay[(i, j)] * oy[(i, j)] + az[(i, j)] * oz[(i, j)];
        }
    }
    hh * h * h
}

/// Curl of `ω` returning `(Jx, Jy, Jz)` on interior cells.
fn curl(ox: &Grid, oy: &Grid, oz: &Grid, dx: f64, dy: f64) -> (Grid, Grid, Grid) {
    let (nx, ny) = (ox.shape()[0], ox.shape()[1]);
    let mut jx = Array2::zeros((nx, ny));
    let mut jy = Array2::zeros((nx, ny));
    let mut jz = Array2::zeros((nx, ny));
    for i in 1..nx - 1 {
        for j in 1..ny - 1 {
            jx[(i, j)] = (oz[(i, j + 1)] - oz[(i, j - 1)]) / (2.0 * dy)
                - (oy[(i + 1, j)] - oy[(i - 1, j)]) / (2.0 * dx);
            jy[(i, j)] = (ox[(i + 1, j)] - ox[(i - 1, j)]) / (2.0 * dx)
                - (oz[(i, j + 1)] - oz[(i, j - 1)]) / (2.0 * dy);
            jz[(i, j)] = (oy[(i + 1, j)] - oy[(i - 1, j)]) / (2.0 * dx)
                - (ox[(i, j + 1)] - ox[(i, j - 1)]) / (2.0 * dy);
        }
    }
    (jx, jy, jz)
}