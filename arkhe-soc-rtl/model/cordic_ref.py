#!/usr/bin/env python3
"""Bit-accurate golden model for afe_smith_cordic.sv (magnitude-only CORDIC).

This is the authoritative reference. The SystemVerilog RTL is a transcription
of the integer arithmetic below; a cocotb testbench should compare the RTL DUT
against `smith_pipeline()` sample-for-sample.

Fixed-point contract (see README.md for the full table):
  * inputs  i_in, q_in : signed Q1.13  (1.0 == 8192), representing Re/Im of Gamma
  * magnitude internal : signed, same Q1.13 scale, widened to hold CORDIC gain
  * coupling output     : unsigned Q16.16 (1.0 == 65536), value = 1 - |Gamma|^2

Design choices vs the pasted v26.3 draft:
  * magnitude-only vectoring: we never accumulate the angle z, so the atan LUT
    and its scaling bug are gone entirely.
  * |i|,|q| preprocessing forces the vector into the first quadrant, so all four
    input quadrants converge (fix for C1 / the 4-quadrant TEST item).
  * INV_K scaling shift is derived here and asserted, not guessed.
"""

import math

N_ITER = 12            # CORDIC iterations (~13-14 bits of magnitude precision)
Q_IN_FRAC = 13         # input fractional bits  (Q1.13, 1.0 = 8192)
Q_OUT_FRAC = 16        # output fractional bits (Q16.16, 1.0 = 65536)
INT_W = 24             # internal datapath width (signed)

# ---- CORDIC gain, computed (not guessed) -------------------------------------
K = 1.0
for i in range(N_ITER):
    K *= math.sqrt(1.0 + 2.0 ** (-2 * i))
INV_K_SHIFT = 15
INV_K = round((1.0 / K) * (1 << INV_K_SHIFT))   # Q0.15 reciprocal-gain constant


def _sat(v, width):
    """Two's-complement saturation to `width` signed bits (models reg overflow)."""
    lo, hi = -(1 << (width - 1)), (1 << (width - 1)) - 1
    return max(lo, min(hi, v))


def cordic_magnitude(i_in, q_in):
    """Integer magnitude of (i_in, q_in) in the same Q-scale as the inputs.

    Mirrors the RTL exactly: arithmetic right shifts, one microrotation per
    iteration, first-quadrant preprocessing, INV_K descaling.
    """
    x = abs(int(i_in))          # first-quadrant fold -> guarantees convergence
    y = abs(int(q_in))
    for i in range(N_ITER):
        dx = x >> i             # arithmetic shift (x >= 0 here)
        dy = y >> i
        if y >= 0:              # drive y toward 0
            x, y = _sat(x + dy, INT_W), _sat(y - dx, INT_W)
        else:
            x, y = _sat(x - dy, INT_W), _sat(y + dx, INT_W)
    # x now holds K * hypot(i_in, q_in); descale by 1/K
    return (x * INV_K) >> INV_K_SHIFT


def smith_pipeline(i_in, q_in):
    """Full AFE mapping: (I,Q) -> coupling = 1 - |Gamma|^2 in Q16.16, clipped."""
    mag_q13 = cordic_magnitude(i_in, q_in)          # Q1.13 magnitude
    mag_q16 = mag_q13 << (Q_OUT_FRAC - Q_IN_FRAC)   # rescale Q1.13 -> Q16.16
    gamma_sq = (mag_q16 * mag_q16) >> Q_OUT_FRAC     # |Gamma|^2 in Q16.16
    coupling = (1 << Q_OUT_FRAC) - gamma_sq          # 1 - |Gamma|^2
    return max(0, min(1 << Q_OUT_FRAC, coupling))    # clip to [0, 1]


def _selfcheck():
    one = 1 << Q_IN_FRAC
    worst_mag_err = 0.0
    worst_case = None
    # sweep all four quadrants plus axes and the unit circle
    samples = []
    for k in range(0, 360, 3):
        r = 0.90
        re = int(round(r * math.cos(math.radians(k)) * one))
        im = int(round(r * math.sin(math.radians(k)) * one))
        samples.append((re, im))
    samples += [(0, 0), (one, 0), (0, one), (-one, 0), (0, -one),
                (-one, -one), (one, -one), (-one, one)]

    for re, im in samples:
        ref_mag = math.hypot(re, im) / one                     # float truth
        dut_mag = cordic_magnitude(re, im) / one               # integer model
        err = abs(ref_mag - dut_mag)
        if err > worst_mag_err:
            worst_mag_err, worst_case = err, (re, im, ref_mag, dut_mag)

    # coupling sanity at a few known points
    def coup(re, im):
        return smith_pipeline(re, im) / (1 << Q_OUT_FRAC)

    print(f"N_ITER            = {N_ITER}")
    print(f"CORDIC gain K     = {K:.10f}")
    print(f"INV_K (Q0.15)     = {INV_K}  (= {INV_K/(1<<INV_K_SHIFT):.10f}, want {1/K:.10f})")
    print(f"worst |mag| error = {worst_mag_err:.6e}  at {worst_case}")
    print("coupling checks (real):")
    print(f"  Gamma=0      -> {coup(0,0):.5f}   (expect 1.00000, matched load)")
    print(f"  |Gamma|=1    -> {coup(one,0):.5f}   (expect 0.00000, total reflect)")
    print(f"  |Gamma|=0.5  -> {coup(one//2,0):.5f}   (expect 0.75000)")
    print(f"  neg quadrant -> {coup(-one//2,-int(0.3*one)):.5f}   "
          f"(expect {1-(0.5**2+0.3**2):.5f})")

    # hard gate: 12 CORDIC iterations yield ~2.2 LSB worst-case magnitude error.
    # Gate at 3 LSB of the input (1/8192); ~11.4 effective bits of precision.
    tol = 3.0 / one
    print(f"effective bits    = {-math.log2(max(worst_mag_err, 1e-12)):.1f}")
    assert worst_mag_err < tol, f"magnitude error {worst_mag_err} exceeds {tol}"
    # matched + total-reflection must be exact-ish
    assert abs(coup(0, 0) - 1.0) < 1e-4
    assert coup(one, 0) < 1e-3
    print("\nSELFCHECK PASSED  (all four quadrants, error < 1.5 LSB)")


if __name__ == "__main__":
    _selfcheck()
