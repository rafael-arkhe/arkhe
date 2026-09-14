//! SVD incremental (Brand 2006 + Zhang 2022).
//!
//! Correções aplicadas:
//!   - U: matriz aumentada [U | q] @ Ub (evita broadcasting errado)
//!   - Vt: extended @ Vtb^T com identidade axial
//!   - rank efetivo: base 2 (não base e)
//!   - eigenvalues: expostos via get_spectral_state
//!   - reortogonalização periódica via re-SVD central (Zhang)

use nalgebra::{DMatrix, DVector};

/// SVD incremental com correções de Brand e reortogonalização opcional.
pub struct IncrementalSVD {
    pub n_rows: usize,
    pub max_rank: usize,
    pub k: usize,
    pub u: DMatrix<f64>,      // n_rows × max_rank
    pub s: DVector<f64>,      // max_rank
    pub vt: DMatrix<f64>,     // max_rank × n_cols
    pub n_cols: usize,
    reorth_every: Option<usize>,
    steps_since_reorth: usize,
}

impl IncrementalSVD {
    pub fn new(n_rows: usize, max_rank: usize, reorth_every: Option<usize>) -> Self {
        Self {
            n_rows,
            max_rank,
            k: 0,
            u: DMatrix::zeros(n_rows, max_rank),
            s: DVector::zeros(max_rank),
            vt: DMatrix::zeros(max_rank, 0),
            n_cols: 0,
            reorth_every,
            steps_since_reorth: 0,
        }
    }

    pub fn add_column(&mut self, col: &DVector<f64>) {
        assert_eq!(col.len(), self.n_rows, "dimensão incompatível");
        self.n_cols += 1;
        self.steps_since_reorth += 1;

        // Caso base
        if self.k == 0 {
            let norm = col.norm();
            if norm > 1e-12 {
                self.u.set_column(0, &(col / norm));
                self.s[0] = norm;
                self.vt = DMatrix::zeros(self.max_rank, 1);
                self.vt[(0, 0)] = 1.0;
                self.k = 1;
            }
            return;
        }

        let k = self.k;

        // Projeção e resíduo
        let u_k = self.u.columns(0, k).into_owned();
        let proj = u_k.transpose() * col;
        let residual = col - &u_k * &proj;
        let norm_residual = residual.norm();

        // Matriz B (k+1) × (k+1)
        let mut b = DMatrix::<f64>::zeros(k + 1, k + 1);
        for i in 0..k {
            b[(i, i)] = self.s[i];
        }
        for i in 0..k {
            b[(i, k)] = proj[i];
        }
        b[(k, k)] = norm_residual;

        // SVD de B
        let svd_b = b.svd(true, true);
        let ub = svd_b.u.unwrap();
        let sb = svd_b.singular_values;
        let vtb = svd_b.v_t.unwrap();

        let new_k = std::cmp::min(k + 1, self.max_rank);

        // === U: matriz aumentada [U | q] @ Ub ===
        let mut u_aug = DMatrix::<f64>::zeros(self.n_rows, k + 1);
        u_aug.columns_mut(0, k).copy_from(&u_k);
        if norm_residual > 1e-12 {
            u_aug.set_column(k, &(residual / norm_residual));
        } else if new_k > k {
            // Direção ortogonal arbitrária
            let mut q = DVector::<f64>::from_fn(self.n_rows, |i, _| {
                (i as f64 + 1.0) * 0.5773
            });
            q -= &u_k * (u_k.transpose() * &q);
            let qn = q.norm();
            if qn > 1e-12 {
                u_aug.set_column(k, &(q / qn));
            }
        }
        let u_new = &u_aug * ub.columns(0, new_k);

        // === Vt: extended @ Vtb^T ===
        let mut extended = DMatrix::<f64>::zeros(k + 1, self.n_cols);
        for i in 0..k {
            for j in 0..self.n_cols - 1 {
                extended[(i, j)] = self.vt[(i, j)];
            }
        }
        extended[(k, self.n_cols - 1)] = 1.0;

        let vtb_slice = vtb.rows(0, new_k).into_owned();
        let vt_new = vtb_slice * extended;

        // Atualizar fatores
        self.u.columns_mut(0, new_k).copy_from(&u_new);
        for i in 0..new_k {
            self.s[i] = sb[i];
        }
        if self.vt.ncols() != self.n_cols {
            self.vt = DMatrix::zeros(self.max_rank, self.n_cols);
        }
        self.vt.rows_mut(0, new_k).copy_from(&vt_new);
        self.k = new_k;

        // Truncamento adaptativo
        let limiar = if self.s[0] > 0.0 {
            1e-10 * self.s[0]
        } else {
            1e-12
        };
        let keep: Vec<usize> = (0..self.k)
            .filter(|&i| self.s[i] > limiar)
            .collect();
        if keep.is_empty() {
            // manter pelo menos o primeiro
            let new_u = self.u.column(0).into_owned();
            let new_s = DVector::from_element(1, self.s[0]);
            let new_vt = self.vt.row(0).into_owned();
            self.u = DMatrix::from_column_slice(self.n_rows, 1, new_u.as_slice());
            self.s = new_s;
            self.vt = DMatrix::from_row_slice(1, self.n_cols, new_vt.as_slice());
            self.k = 1;
        } else if keep.len() < self.k {
            let new_u = self.u.columns(0, self.k).into_owned();
            let new_vt = self.vt.rows(0, self.k).into_owned();
            let new_s = self.s.rows(0, self.k).into_owned();
            let keep_u = new_u.columns(0, 1).into_owned(); // placeholder
            let _ = keep_u;
            self.u = DMatrix::from_fn(self.n_rows, keep.len(), |i, j| new_u[(i, keep[j])]);
            self.s = DVector::from_fn(keep.len(), |i, _| new_s[keep[i]]);
            self.vt = DMatrix::from_fn(keep.len(), self.n_cols, |i, j| new_vt[(keep[i], j)]);
            self.k = keep.len();
        }

        // Reortogonalização periódica
        if let Some(every) = self.reorth_every {
            if self.steps_since_reorth >= every {
                self.reorthogonalize();
            }
        }
    }

    /// Reortogonalização via re‑SVD central (Zhang 2022).
    pub fn reorthogonalize(&mut self) {
        if self.k < 2 {
            self.steps_since_reorth = 0;
            return;
        }
        // QR de U
        let u_k = self.u.columns(0, self.k).into_owned();
        let qr = u_k.qr();
        let q = qr.q();
        let r = qr.r();

        // M = R · diag(s) · Vt
        let s_mat = DMatrix::from_diagonal(&self.s.rows(0, self.k));
        let vt_k = self.vt.rows(0, self.k).into_owned();
        let m = &r * s_mat * &vt_k;

        // SVD de M
        let svd_m = m.svd(true, true);
        let um = svd_m.u.unwrap();
        let sm = svd_m.singular_values;
        let vtm = svd_m.v_t.unwrap();

        let new_k = std::cmp::min(sm.len(), self.k);

        // U_new = Q @ Um
        let u_new = &q * um.columns(0, new_k);
        self.u.columns_mut(0, new_k).copy_from(&u_new);
        for i in 0..new_k {
            self.s[i] = sm[i];
        }
        let vt_new = vtm.rows(0, new_k).into_owned();
        self.vt.rows_mut(0, new_k).copy_from(&vt_new);
        self.k = new_k;
        self.steps_since_reorth = 0;
    }

    pub fn get_entropy(&self) -> f64 {
        if self.k == 0 {
            return 0.0;
        }
        let s2: f64 = (0..self.k).map(|i| self.s[i] * self.s[i]).sum();
        if s2 < 1e-12 {
            return 0.0;
        }
        let mut h = 0.0;
        for i in 0..self.k {
            let p = self.s[i] * self.s[i] / s2;
            if p > 1e-12 {
                h -= p * p.log2();
            }
        }
        h
    }

    pub fn get_effective_rank(&self) -> f64 {
        let h = self.get_entropy();
        if h > 0.0 { 2.0_f64.powf(h) } else { 1.0 }
    }

    pub fn spectral_state(&self) -> SpectralState {
        if self.k == 0 {
            return SpectralState {
                entropy: 0.0,
                effective_rank: 1.0,
                dominant_ratio: 1.0,
                eigenvalues: DVector::zeros(0),
                rank: 0,
                n_experiences: self.n_cols,
            };
        }
        let s2: f64 = (0..self.k).map(|i| self.s[i] * self.s[i]).sum();
        let p = DVector::from_fn(self.k, |i, _| {
            if s2 > 1e-12 {
                self.s[i] * self.s[i] / s2
            } else {
                0.0
            }
        });
        let mut h = 0.0;
        for i in 0..self.k {
            if p[i] > 1e-12 {
                h -= p[i] * p[i].log2();
            }
        }
        let dom = (0..self.k).map(|i| p[i]).fold(0.0_f64, f64::max);
        SpectralState {
            entropy: h,
            effective_rank: 2.0_f64.powf(h),
            dominant_ratio: dom,
            eigenvalues: p,
            rank: self.k,
            n_experiences: self.n_cols,
        }
    }

    pub fn reconstruct(&self) -> DMatrix<f64> {
        if self.k == 0 {
            return DMatrix::zeros(self.n_rows, self.n_cols);
        }
        let u_k = self.u.columns(0, self.k).into_owned();
        let s_mat = DMatrix::from_diagonal(&self.s.rows(0, self.k));
        let vt_k = self.vt.rows(0, self.k).into_owned();
        u_k * s_mat * vt_k
    }
}

#[derive(Debug, Clone)]
pub struct SpectralState {
    pub entropy: f64,
    pub effective_rank: f64,
    pub dominant_ratio: f64,
    pub eigenvalues: DVector<f64>,
    pub rank: usize,
    pub n_experiences: usize,
}