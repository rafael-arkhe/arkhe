"""ARKHE full pipeline: extraction -> M1 model -> null test -> Bayes -> report.

Orchestrates the five-stage analysis of the real IXPE observation
(1E 1547.0-5408, ObsID 04003801):

    1. extract_ixpe_stokes    background-subtracted Stokes, PD(E), PA
    2. magthomscatt_wrapper   analytic M1 model fitted to PD(E)
    3. arkhe_null_parallel    null-hypothesis test (default 1000 sims)
    4. arkhe_bayes_v22        Bayesian resonance search H1/H1.5/H2 (dynesty)
    5. validation             against arXiv 2601.15452 (PD 47.7 +/- 2.9%)

Outputs: pipeline_report.json and pipeline_figure.png in --out-dir.
"""

from __future__ import annotations

import argparse
import contextlib
import io
import json
import os
from typing import Dict

import numpy as np

import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt

from extract_ixpe_stokes import extract_stokes
from magthomscatt_wrapper import m1_model, fit_m1
from arkhe_null_parallel import run_null_test
import arkhe_bayes_v22 as bayes

PAPER = {
    "PD": 47.7, "PD_err": 2.9, "PA": 75.8, "band": "2-6 keV",
    "ref": "arXiv 2601.15452 (Taverna et al. 2026)",
}

SOURCE = {"name": "1E1547.0-5408", "B_gauss": 3.2e14}

CALIB_PLACEHOLDER = {
    "eta_mu": 0.5, "eta_sigma": 0.3,
    "alpha_mu": 1.0, "alpha_sigma": 0.5,
    "beta_mu": 0.0, "beta_sigma": 0.3,
}


def run_bayes_stage(profile: Dict[str, object], nlive: int = 300, dlogz: float = 0.1,
                    e_res_lo: float = 1.0, e_res_hi: float = 8.0,
                    modes=("independent", "calibrated"), seed: int = 260115452,
                    f_ff_ref: float = 1.0, f_ff_sigma: float = 0.05) -> Dict[str, object]:
    """Bayesian comparison H1 (no resonance) vs H1.5 (Gaussian) vs H2 (Voigt)."""
    if len(profile["E"]) < 4:
        return {"skipped": True, "reason": "profile too sparse"}
    E = np.asarray(profile["E"], dtype=float)
    pd = np.asarray(profile["PD"], dtype=float) / 100.0
    pd_err = np.asarray(profile["PD_err"], dtype=float) / 100.0
    phi = np.zeros_like(E)

    m1 = fit_m1(E, profile["PD"], profile["PD_err"])
    pd_base = lambda E_arr: m1_model(np.asarray(E_arr), m1["A"], m1["alpha"], m1["beta"]) / 100.0

    data = bayes.Dataset(E=E, phi=phi, PD_obs=pd, PD_err=pd_err, pd_base_fn=pd_base)

    buf = io.StringIO()
    with contextlib.redirect_stdout(buf), contextlib.redirect_stderr(buf):
        bf = bayes.bayes_factor_analysis(
            data, CALIB_PLACEHOLDER, nlive=nlive, dlogz=dlogz,
            e_res_lo=e_res_lo, e_res_hi=e_res_hi, seed=seed,
            f_ff_ref=f_ff_ref, f_ff_sigma=f_ff_sigma)
    return {
        "skipped": False,
        "nlive": nlive,
        "dlogz": dlogz,
        "e_res_range_kev": [e_res_lo, e_res_hi],
        "seed": seed,
        "baseline_m1": m1,
        "n_points": int(len(E)),
        "results": {m: bf[m] for m in modes if m in bf},
    }


def build_figure(result: Dict[str, object], out_path: str) -> None:
    prof = result["profile"]
    m1 = result["m1_fit"]
    obs = result["observed"]

    fig, (ax1, ax2) = plt.subplots(1, 2, figsize=(12, 5))

    E = np.array(prof["E"])
    pd = np.array(prof["PD"])
    pd_err = np.array(prof["PD_err"])
    ax1.errorbar(E, pd, yerr=pd_err, fmt="o", color="tab:blue", capsize=3,
                 label="IXPE measured PD(E)")
    if m1 is not None:
        Es = np.linspace(E.min() - 0.2, E.max() + 0.2, 200)
        ax1.plot(Es, m1_model(Es, m1["A"], m1["alpha"], m1["beta"]),
                 color="tab:red", ls="--",
                 label=f'M1 fit: A={m1["A"]:.1f}%, alpha={m1["alpha"]:.3f}, '
                       f'beta={m1["beta"]:.3f}')
    ax1.axhline(PAPER["PD"], color="tab:green", ls=":", lw=1.2,
                label=f'paper {PAPER["PD"]}% (2-6 keV)')
    ax1.fill_between([2.0, 6.0], PAPER["PD"] - PAPER["PD_err"],
                     PAPER["PD"] + PAPER["PD_err"], color="tab:green",
                     alpha=0.15)
    ax1.axvspan(2.0, 6.0, color="gray", alpha=0.08)
    ax1.set_xlabel("Energy [keV]")
    ax1.set_ylabel("PD [%]")
    ax1.set_title("M1 magnetic-Thomson-scattering model fit")
    ax1.legend(fontsize=8)
    ax1.grid(alpha=0.3)

    n = result["null"]["n_sims"]
    samples = np.asarray(result["null_samples"])
    ax2.hist(samples, bins=60, color="tab:gray",
             alpha=0.8, density=True, label="H0 (PD=0) simulations")
    ax2.axvline(obs["PD"], color="tab:red", lw=2,
                label=f'observed PD={obs["PD"]:.1f}% '
                      f'(z={result["null"]["z_score"]:.1f} sigma)')
    ax2.set_xlabel("PD [%]")
    ax2.set_ylabel("density")
    ax2.set_title(f"Null-hypothesis test ({n} sims, "
                  f"p={result['null']['p_value']:.3g})")
    ax2.legend(fontsize=8)
    ax2.grid(alpha=0.3)

    fig.suptitle("ARKHE pipeline | 1E 1547.0-5408 | IXPE ObsID 04003801 | 2-6 keV")

    if "bayes" in result and not result["bayes"].get("skipped"):
        btxt = []
        for mode, r in result["bayes"]["results"].items():
            btxt.append(f'{mode}: ln BF(H2/H1)={r["ln_BF_H2_H1"]:+.1f} '
                        f'-> {r["interpretation_H2_H1"]}')
        ax1.text(0.02, 0.03, "\n".join(btxt), transform=ax1.transAxes,
                 fontsize=8, va="bottom", ha="left",
                 bbox=dict(boxstyle="round", fc="white", alpha=0.85),
                 color="tab:red")

    fig.tight_layout(rect=[0, 0, 1, 0.94])
    fig.savefig(out_path, dpi=150)
    plt.close(fig)


def validate(result: Dict[str, object]) -> Dict[str, object]:
    obs_pd = result["observed"]["PD"]
    obs_pa = result["observed"]["PA_deg"]
    d_pd = obs_pd - PAPER["PD"]
    checks = {
        "PD_within_1sigma": bool(abs(d_pd) <= PAPER["PD_err"]),
        "PD_within_2sigma": bool(abs(d_pd) <= 2 * PAPER["PD_err"]),
        "PA_magnitude_consistent": bool(abs(abs(obs_pa) - PAPER["PA"]) <= 12.0),
        "null_rejected_p0.01": bool(result["null"]["p_value"] < 0.01),
        "significance_gt_3sigma": bool(result["observed"]["significance"] > 3.0),
        "MDP_not_exceeded": bool(obs_pd > result["observed"]["MDP99"]),
    }
    checks["summary"] = {
        "delta_PD_percent": float(d_pd),
        "paper_PD": PAPER["PD"],
        "paper_PD_err": PAPER["PD_err"],
        "obs_PD": obs_pd,
        "obs_PA_deg": obs_pa,
        "obs_PD_err": result["observed"]["PD_err"],
        "obs_significance": result["observed"]["significance"],
        "p_value": result["null"]["p_value"],
    }
    checks["all_pass"] = bool(all(v for k, v in checks.items() if k not in ("summary",)))
    return checks


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--data-dir", required=True)
    ap.add_argument("--n-sims", type=int, default=1000)
    ap.add_argument("--seed", type=int, default=260115452)
    ap.add_argument("--out-dir", default=".")
    ap.add_argument("--pi-lo", type=int, default=96)
    ap.add_argument("--pi-hi", type=int, default=287)
    ap.add_argument("--nlive", type=int, default=300, help="dynesty live points")
    ap.add_argument("--dlogz", type=float, default=0.1)
    ap.add_argument("--e-res-lo", type=float, default=1.0)
    ap.add_argument("--e-res-hi", type=float, default=8.0)
    ap.add_argument("--no-bayes", action="store_true", help="skip Bayesian stage")
    ap.add_argument("--source-name", default="1E1547.0-5408")
    ap.add_argument("--b-gauss", type=float, default=3.2e14)
    ap.add_argument("--paper-pd", type=float, default=47.7)
    ap.add_argument("--paper-pd-err", type=float, default=2.9)
    ap.add_argument("--paper-pa", type=float, default=75.8)
    ap.add_argument("--paper-band", default="2-6 keV")
    ap.add_argument("--paper-ref", default="arXiv 2601.15452 (Taverna et al. 2026)")
    args = ap.parse_args()

    SOURCE["name"] = args.source_name
    SOURCE["B_gauss"] = args.b_gauss
    PAPER.update({
        "PD": args.paper_pd, "PD_err": args.paper_pd_err,
        "PA": args.paper_pa, "band": args.paper_band,
        "ref": args.paper_ref,
    })

    os.makedirs(args.out_dir, exist_ok=True)

    result = run_null_test(args.data_dir, args.n_sims, args.seed,
                           pi_lo=args.pi_lo, pi_hi=args.pi_hi)

    if args.no_bayes:
        result["bayes"] = {"skipped": True, "reason": "disabled by --no-bayes"}
    else:
        ff, ff_sigma, ff_status = bayes.get_f_FF_with_uncertainty(
            SOURCE["B_gauss"], SOURCE["name"])
        print(f"[pipeline] running Bayesian resonance search (dynesty) with "
              f"f_FF={ff:.2f}+-{ff_sigma:.2f} ({ff_status})...")
        result["bayes"] = run_bayes_stage(
            result["profile"], nlive=args.nlive, dlogz=args.dlogz,
            e_res_lo=args.e_res_lo, e_res_hi=args.e_res_hi,
            seed=args.seed, f_ff_ref=ff, f_ff_sigma=ff_sigma)
        result["bayes"]["f_FF"] = {"value": ff, "sigma": ff_sigma,
                                   "status": ff_status}

    result["validation"] = validate(result)
    result["paper"] = PAPER

    fig_path = os.path.join(args.out_dir, "pipeline_figure.png")
    build_figure(result, fig_path)

    out_json = os.path.join(args.out_dir, "pipeline_report.json")

    def _json_default(o):
        if isinstance(o, np.ndarray):
            return o.tolist()
        return float(o)

    with open(out_json, "w") as fh:
        json.dump(result, fh, indent=2, default=_json_default)

    print(f"[pipeline] figure  -> {fig_path}")
    print(f"[pipeline] report  -> {out_json}")
    print(f"[pipeline] PD={result['observed']['PD']:.1f}% +/- "
          f"{result['observed']['PD_err']:.1f}%  "
          f"(paper {PAPER['PD']}% +/- {PAPER['PD_err']}%)")
    print(f"[pipeline] PA={result['observed']['PA_deg']:.1f} deg "
          f"(paper {PAPER['PA']} deg)")
    print(f"[pipeline] null test: p={result['null']['p_value']:.3g}, "
          f"z={result['null']['z_score']:.1f} sigma, "
          f"n_sims={result['null']['n_sims']}")
    if "bayes" in result and not result["bayes"].get("skipped"):
        ff = result["bayes"].get("f_FF", {})
        print(f"[pipeline] bayes f_FF={ff.get('value', 1.0)} +- "
              f"{ff.get('sigma', 0.05)} ({ff.get('status', 'n/a')})")
        for mode, r in result["bayes"]["results"].items():
            print(f"[pipeline] bayes [{mode}]: ln BF(H2/H1)="
                  f"{r['ln_BF_H2_H1']:+.2f} "
                  f"({r['interpretation_H2_H1']}); "
                  f"ln BF(H2/H1.5)={r['ln_BF_H2_H1.5']:+.2f}")
    print(f"[pipeline] validation all_pass = "
          f"{result['validation']['all_pass']}")
    if not result["validation"]["all_pass"]:
        for k, v in result["validation"].items():
            if k != "summary" and not v:
                print(f"[pipeline]   FAIL: {k}")


if __name__ == "__main__":
    main()
