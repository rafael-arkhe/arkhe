"""Null-hypothesis significance test for the IXPE polarization detection.

Under H0 (unpolarized source, PD = 0) the net Stokes sums Q and U of a large
ensemble are zero-mean Gaussians with variances equal to the sum of the squared
event Stokes weights (including the background-scaled contribution).  We
realize n_sims independent draws of (Q, U) from that null distribution, derive
the corresponding PD statistic, and compute the p-value as the fraction of
simulations whose PD exceeds the observed one.

MPI is used if mpi4py is importable and MPI.COMM_WORLD.size > 1; otherwise a
multiprocessing Pool fallback is used.
"""

from __future__ import annotations

import argparse
import json
import os
from typing import Dict, List

import numpy as np

try:
    import mpi4py.MPI as MPI
    HAVE_MPI = True
except Exception:
    HAVE_MPI = False

from extract_ixpe_stokes import extract_stokes, modulation_factor, pi_to_kev
from magthomscatt_wrapper import fit_m1, m1_model

DEFAULT_RNG = np.random.default_rng(260115452)


def null_pd_samples(rng, sigma_q: float, sigma_u: float, I: float, mu_eff: float,
                    n: int) -> np.ndarray:
    """PD values (percent) drawn from the null distribution (Rayleigh-like)."""
    q = rng.normal(0.0, sigma_q, n)
    u = rng.normal(0.0, sigma_u, n)
    return 100.0 * np.hypot(q, u) / (I * mu_eff)


def _worker_impl(task) -> np.ndarray:
    seed, chunk, sQ, sU, I, mu_eff = task
    rng = np.random.default_rng(seed)
    return null_pd_samples(rng, sQ, sU, I, mu_eff, chunk)


def energy_binned_pd(data_dir: str, pi_lo: int = 96, pi_hi: int = 287,
                     bin_width_pi: int = 24) -> Dict[str, object]:
    """Binned PD(E) profile used for the M1 and Bayesian model comparison."""
    edges = list(range(pi_lo, pi_hi + bin_width_pi, bin_width_pi))
    E, PD, E_lo, E_hi, PDerr, n = [], [], [], [], [], []
    for lo, hi in zip(edges[:-1], edges[1:]):
        r = extract_stokes(data_dir, pi_lo=lo, pi_hi=hi)
        if r["n_src"] < 30 or r["mu_eff"] <= 0:
            continue
        E.append(0.5 * (lo + hi) * 0.02 + 0.05)
        E_lo.append(lo * 0.02 + 0.05)
        E_hi.append(hi * 0.02 + 0.05)
        PD.append(r["PD"])
        PDerr.append(r["PD_err"])
        n.append(r["n_src"])
    return {"E": E, "E_lo": E_lo, "E_hi": E_hi, "PD": PD, "PD_err": PDerr, "n_src": n}


def run_null_test(data_dir: str, n_sims: int = 1000, seed: int = 260115452,
                  pi_lo: int = 96, pi_hi: int = 287) -> Dict[str, object]:
    """Run the null hypothesis test against the observed Stokes extraction."""
    obs = extract_stokes(data_dir, pi_lo=pi_lo, pi_hi=pi_hi)
    I = obs["I"]
    # Stokes-sum variances under the null (sum of squared per-event weights)
    sQ = np.sqrt(sum(d.get("ss_Q", 0.0) for d in obs["dets"]))
    sU = np.sqrt(sum(d.get("ss_U", 0.0) for d in obs["dets"]))
    mu_eff = obs["mu_eff"]

    observed_pd = obs["PD"]

    if HAVE_MPI and MPI.COMM_WORLD.size > 1:
        comm = MPI.COMM_WORLD
        rank = comm.Get_rank()
        size = comm.Get_size()
        local = null_pd_samples(DEFAULT_RNG, sQ, sU, I, mu_eff, n_sims // size)
        all_samples = None
        if rank == 0:
            all_samples = np.concatenate(comm.gather(local, root=0))
    else:
        import multiprocessing as mp
        n_cpu = min(16, max(1, (os.cpu_count() or 1) - 1), n_sims)
        chunk = int(np.ceil(n_sims / n_cpu))
        tasks = [(int(DEFAULT_RNG.integers(0, 2**31)), chunk, sQ, sU, I, mu_eff)
                 for _ in range(n_cpu)]
        with mp.Pool(n_cpu) as pool:
            parts = pool.map(_worker_impl, tasks)
        all_samples = np.concatenate(parts)[:n_sims]

    p_value = float(np.mean(all_samples >= observed_pd))
    sigma = float(np.std(all_samples, ddof=1)) if len(all_samples) > 1 else 0.0
    z_score = float((observed_pd - 0.0) / sigma) if sigma > 0 else float("inf")

    # compare against the analytic M1 model for the binned profile
    prof = energy_binned_pd(data_dir)
    m1 = None
    if len(prof["E"]) >= 3:
        m1 = fit_m1(prof["E"], prof["PD"], prof["PD_err"])

    return {
        "observed": obs,
        "null": {
            "n_sims": int(len(all_samples)),
            "p_value": p_value,
            "z_score": z_score,
            "sigma_null": float(sigma),
            "sigma_Q": float(sQ),
            "sigma_U": float(sU),
            "mu_eff": float(mu_eff),
            "pd_percentiles": {
                "50": float(np.percentile(all_samples, 50)),
                "90": float(np.percentile(all_samples, 90)),
                "99": float(np.percentile(all_samples, 99)),
                "max": float(all_samples.max()),
            },
        },
        "null_samples": all_samples,
        "m1_fit": m1,
        "profile": prof,
    }


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--data-dir", required=True)
    ap.add_argument("--n-sims", type=int, default=1000)
    ap.add_argument("--seed", type=int, default=260115452)
    ap.add_argument("--out-json", default=None)
    args = ap.parse_args()
    result = run_null_test(args.data_dir, args.n_sims, args.seed)
    print(json.dumps(result, indent=2, default=float))
    if args.out_json:
        with open(args.out_json, "w") as fh:
            json.dump(result, fh, indent=2, default=float)


if __name__ == "__main__":
    main()
