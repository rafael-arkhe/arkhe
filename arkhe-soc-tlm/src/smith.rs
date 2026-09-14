//! Smith-chart AFE model: (I,Q) -> coupling = 1 - |Γ|², via a magnitude-only
//! CORDIC. This is a line-for-line port of `arkhe-soc-rtl/model/cordic_ref.py`
//! and must stay bit-identical to it and to `rtl/afe_smith_cordic.sv`.
//!
//! Fixed-point contract (see arkhe-soc-rtl/README.md):
//!   * inputs  i,q     : signed Q1.13  (1.0 == 8192)
//!   * magnitude       : same Q1.13 scale, descaled by 1/K (INV_K, Q0.15)
//!   * coupling output : unsigned Q16.16 (1.0 == 65536), = 1 - |Γ|², clipped

pub const N_ITER: u32 = 12; // CORDIC iterations (~11.8 effective bits)
pub const Q_IN_FRAC: u32 = 13; // input fractional bits  (Q1.13, 1.0 = 8192)
pub const Q_OUT_FRAC: u32 = 16; // output fractional bits (Q16.16, 1.0 = 65536)
pub const INV_K: i64 = 19898; // round((1/K) * 2^15), Q0.15 reciprocal-gain
pub const INV_K_SHIFT: u32 = 15;

pub const ONE_IN: i64 = 1 << Q_IN_FRAC; // 8192
pub const ONE_OUT: i64 = 1 << Q_OUT_FRAC; // 65536

/// Integer magnitude of (i,q) in the same Q-scale as the inputs.
///
/// Mirrors the RTL exactly: first-quadrant fold, arithmetic right shifts,
/// one microrotation per iteration, INV_K descaling.
pub fn cordic_magnitude(i_in: i64, q_in: i64) -> i64 {
    let mut x = i_in.abs(); // first-quadrant fold -> guarantees convergence
    let mut y = q_in.abs();
    for i in 0..N_ITER {
        let dx = x >> i; // x >= 0 throughout
        let dy = y >> i;
        if y >= 0 {
            // drive y toward 0
            x += dy;
            y -= dx;
        } else {
            x -= dy;
            y += dx;
        }
    }
    (x * INV_K) >> INV_K_SHIFT // x holds K*hypot(i,q); descale by 1/K
}

/// Full AFE mapping: (I,Q) -> coupling = 1 - |Γ|² in Q16.16, clipped to [0,1].
pub fn smith_coupling(i_in: i64, q_in: i64) -> u32 {
    let mag_q13 = cordic_magnitude(i_in, q_in);
    let mag_q16 = mag_q13 << (Q_OUT_FRAC - Q_IN_FRAC); // Q1.13 -> Q16.16
    let gamma_sq = (mag_q16 * mag_q16) >> Q_OUT_FRAC; // |Γ|² in Q16.16
    let coupling = ONE_OUT - gamma_sq;
    coupling.clamp(0, ONE_OUT) as u32
}

/// Convenience: coupling as an f64 in [0,1].
pub fn smith_coupling_f64(i_in: i64, q_in: i64) -> f64 {
    smith_coupling(i_in, q_in) as f64 / ONE_OUT as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    // Mirrors cordic_ref.py::_selfcheck — proves Rust == Python == (intended) RTL.
    #[test]
    fn magnitude_accurate_all_quadrants() {
        let one = ONE_IN as f64;
        let mut worst = 0.0f64;
        for k in (0..360).step_by(3) {
            let r = 0.90;
            let re = (r * (k as f64).to_radians().cos() * one).round() as i64;
            let im = (r * (k as f64).to_radians().sin() * one).round() as i64;
            let refm = ((re * re + im * im) as f64).sqrt() / one;
            let dutm = cordic_magnitude(re, im) as f64 / one;
            worst = worst.max((refm - dutm).abs());
        }
        // 12 iterations -> ~2.2 LSB worst case; gate at 3 LSB.
        assert!(worst < 3.0 / one, "worst magnitude error {worst} too large");
    }

    #[test]
    fn coupling_known_points() {
        let one = ONE_IN;
        // matched load: Γ=0 -> coupling 1.0
        assert_eq!(smith_coupling(0, 0), ONE_OUT as u32);
        // total reflection along an axis: |Γ|≈1 -> ~0
        assert!(smith_coupling(one, 0) < (0.002 * ONE_OUT as f64) as u32);
        // |Γ| = 0.5 -> 1 - 0.25 = 0.75
        assert!((smith_coupling_f64(one / 2, 0) - 0.75).abs() < 0.01);
        // negative quadrant must match magnitude of positive one (rotation-invariant)
        assert_eq!(smith_coupling(-one / 2, 0), smith_coupling(one / 2, 0));
        let a = smith_coupling(-(one / 2), -(3 * one / 10)); // (-0.5,-0.3)
        assert!((a as f64 / ONE_OUT as f64 - (1.0 - (0.25 + 0.09))).abs() < 0.01);
    }
}
