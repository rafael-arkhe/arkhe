"""MAGTHOMSCATT wrapper with analytic M1 fallback.

The MAGTHOMSCATT executable is not installed on this system, so the wrapper
keeps the original interface but implements the M1 model analytically:

    PD(E) = A * exp(-alpha * (E - 2)) * (1 + beta * (E - 2))

with E in keV, A in percent, alpha and beta in keV^-1.  The model is fitted
to the measured PD(E) profile with scipy curve_fit (weighted by PD errors).

If the MAGTHOMSCATT_EXE environment variable points to a working executable
it is invoked instead; otherwise the analytic fallback is used.
"""

from __future__ import annotations

import json
import os
import subprocess
import tempfile
from typing import Dict, List, Optional, Sequence

import numpy as np
from scipy.optimize import curve_fit

M1_DEFAULT = {"A": 47.7, "alpha": 0.02, "beta": 0.01}


def m1_model(energy_kev: np.ndarray, A: float, alpha: float, beta: float) -> np.ndarray:
    """M1 analytic magnetic-Thomson-scattering polarization model (percent)."""
    x = np.asarray(energy_kev, dtype=float) - 2.0
    return A * np.exp(-alpha * x) * (1.0 + beta * x)


def fit_m1(
    energy_kev: Sequence[float],
    pd: Sequence[float],
    pd_err: Optional[Sequence[float]] = None,
) -> Dict[str, object]:
    """Fit the M1 model to measured PD(E), return params + covariance."""
    E = np.asarray(energy_kev, dtype=float)
    P = np.asarray(pd, dtype=float)
    if pd_err is None:
        sigma = np.ones_like(P)
    else:
        sigma = np.asarray(pd_err, dtype=float)
        sigma[sigma <= 0] = 1.0
    p0 = [M1_DEFAULT["A"], M1_DEFAULT["alpha"], M1_DEFAULT["beta"]]
    bounds = ([0.0, -5.0, -5.0], [100.0, 5.0, 5.0])
    popt, pcov = curve_fit(m1_model, E, P, p0=p0, sigma=sigma, bounds=bounds,
                           absolute_sigma=True, maxfev=20000)
    perr = np.sqrt(np.diag(pcov))
    red_chi2 = float(np.sum(((P - m1_model(E, *popt)) / sigma) ** 2) / max(len(E) - 3, 1))
    return {
        "A": float(popt[0]), "alpha": float(popt[1]), "beta": float(popt[2]),
        "A_err": float(perr[0]), "alpha_err": float(perr[1]), "beta_err": float(perr[2]),
        "red_chi2": red_chi2,
        "n_points": int(len(E)),
    }


def _run_external(energy_kev: Sequence[float], pd: Sequence[float]) -> Optional[Dict[str, object]]:
    exe = os.environ.get("MAGTHOMSCATT_EXE")
    if not exe or not os.path.exists(exe):
        return None
    with tempfile.NamedTemporaryFile("w", suffix=".json", delete=False) as fh:
        json.dump({"energy_kev": list(energy_kev), "pd": list(pd)}, fh)
        inpath = fh.name
    outpath = inpath + ".out"
    try:
        subprocess.run([exe, inpath, outpath], check=True, timeout=300)
        with open(outpath) as fh:
            return json.load(fh)
    except Exception:
        return None
    finally:
        for p in (inpath, outpath):
            if os.path.exists(p):
                os.remove(p)


class MAGTHOMSCATT:
    """Fit the M1 magnetic-Thomson-scattering model to PD(E) measurements."""

    def __init__(self, energy_kev: Sequence[float], pd: Sequence[float],
                 pd_err: Optional[Sequence[float]] = None):
        self.energy_kev = np.asarray(energy_kev, dtype=float)
        self.pd = np.asarray(pd, dtype=float)
        self.pd_err = pd_err
        self.result = None
        self.mode = "analytical-M1"

    def fit(self) -> Dict[str, object]:
        ext = _run_external(self.energy_kev, self.pd)
        if ext is not None:
            self.mode = "executable"
            self.result = ext
            return ext
        self.result = fit_m1(self.energy_kev, self.pd, self.pd_err)
        return self.result

    def predict(self, energy_kev: Sequence[float]) -> np.ndarray:
        if self.result is None:
            self.fit()
        return m1_model(np.asarray(energy_kev), self.result["A"],
                        self.result["alpha"], self.result["beta"])


def main() -> None:
    import argparse
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--energy", nargs="+", type=float, required=True)
    ap.add_argument("--pd", nargs="+", type=float, required=True)
    ap.add_argument("--pd-err", nargs="+", type=float, default=None)
    ap.add_argument("--out-json", default=None)
    args = ap.parse_args()
    fit = MAGTHOMSCATT(args.energy, args.pd, args.pd_err).fit()
    print(json.dumps({"mode": "analytical-M1", **fit}, indent=2))
    if args.out_json:
        with open(args.out_json, "w") as fh:
            json.dump({"mode": "analytical-M1", **fit}, fh, indent=2)


if __name__ == "__main__":
    main()
