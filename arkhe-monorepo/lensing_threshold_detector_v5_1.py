#!/usr/bin/env python3
"""
lensing_threshold_detector_v5_1.py
Deteccao do ponto operacional FPGA via TARGET D_KL (nao-inflection).

v5.1 -- 2026-08-11

Correcoes empiricas incorporadas (validacao numerica):
  1. D_KL(p) e monotona e aproximadamente concava em p; nao ha ponto de
     inflexao fisico. 'p_collapse' (max curvatura) era artefato do piso
     plug-in -> substituido por target D_KL diretamente interpretavel.
  2. Target: D_KL = 1.0 bit (perda ~50% da informacao estrutural, regime
     conservador bem antes da saturacao ~5.8 bits).
  3. Busca: coarse sweep (grid log baixo-p) -> spline suave -> bissccao.
  4. Margem: p_operate = 0.8 * p_target (engenharia padrao).
  5. Referencia ideal = pipeline Q16.16 com p_flip=0 (evita mismatch de
     quantizacao que inflava KL(0) para ~27 bits) + Laplace smoothing.

Uso:
  python lensing_threshold_detector_v5_1.py [--n 50000] [--target 1.0]
        [--out csv] [--plot png]
"""

import argparse
import json
import os
import sys

import numpy as np

K_LENS = 40.0
B_MIN, B_MAX = 1.0, 100.0
N_BITS = 32
SCALE = 2 ** 16
N_BINS = 256
EPS = 1e-12


def lensing_alpha(n_samples, seed):
    """alpha = K/b, pipeline Q16.16 SEM bit-flip (referencia quantizada)."""
    rng = np.random.default_rng(seed)
    b_ideal = rng.uniform(B_MIN, B_MAX, n_samples)
    b_int = np.round(b_ideal * SCALE).astype(np.int64)
    b_q = np.clip(b_int / SCALE, B_MIN, B_MAX)
    return np.clip(K_LENS / b_q, K_LENS / B_MAX, K_LENS / B_MIN)


def lensing_noisy_alpha(p_flip, n_samples, seed):
    """alpha = K/b com b corrompido por bit-flips em Q16.16."""
    rng = np.random.default_rng(seed)
    b_ideal = rng.uniform(B_MIN, B_MAX, n_samples)
    b_int = np.round(b_ideal * SCALE).astype(np.int64)

    flips = rng.random((n_samples, N_BITS)) < p_flip
    mask = (flips << np.arange(N_BITS)).sum(axis=1).astype(np.int64)
    b_noisy = np.clip((b_int ^ mask).astype(np.float64) / SCALE, B_MIN, B_MAX)
    return np.clip(K_LENS / b_noisy, K_LENS / B_MAX, K_LENS / B_MIN)


def _log_edges(n_bins=N_BINS):
    a_min, a_max = K_LENS / B_MAX, K_LENS / B_MIN
    return np.linspace(np.log10(a_min + EPS), np.log10(a_max + EPS),
                       n_bins + 1)


def measure_kl(p_flip, n_samples=50000, seed=0, n_bins=None):
    """KL(P_noisy || P_ideal_quantizada) em bits, com Laplace smoothing."""
    n_bins = n_bins if n_bins is not None else N_BINS
    a_obs = lensing_noisy_alpha(p_flip, n_samples, seed)
    a_ideal = lensing_alpha(n_samples, seed + 1000)
    edges = _log_edges(n_bins)

    h_obs, _ = np.histogram(np.log10(a_obs + EPS), bins=edges)
    h_ideal, _ = np.histogram(np.log10(a_ideal + EPS), bins=edges)

    h_obs = (h_obs + 0.5) / (h_obs + 0.5).sum()
    h_ideal = (h_ideal + 0.5) / (h_ideal + 0.5).sum()
    mask = h_obs > 0
    return float(np.sum(h_obs[mask] * np.log2(h_obs[mask] / h_ideal[mask])))


def build_sweep_grid(p_max=0.5, n_low=24, n_high=8):
    """Grid log-spaced concentrado no regime baixo-p + cauda uniforme."""
    low = np.geomspace(1e-3, 0.1, n_low)
    high = np.linspace(0.12, p_max, n_high)
    return np.unique(np.concatenate([low, high]))


def find_target_p(p_vals, kl_vals, target=1.0):
    """p onde KL = target via spline cubica + bissccao (monotona).

    Metodo A: usa a curva ja amostrada no sweep (sem custo extra).
    """
    from scipy.interpolate import CubicSpline
    spl = CubicSpline(p_vals, kl_vals)
    lo, hi = p_vals[0], p_vals[-1]
    if spl(lo) >= target:
        return lo
    if spl(hi) <= target:
        return hi
    for _ in range(60):
        mid = 0.5 * (lo + hi)
        if spl(mid) < target:
            lo = mid
        else:
            hi = mid
    return 0.5 * (lo + hi)


def measure_kl_repeat(p, n_samples, seed, n_repeat=5):
    """KL em p com n_repeat medicoes independentes -> (media, std).

    O std e o ruido Monte Carlo real da medicao (no hardware cada rodada
    e uma realizacao nova, nao uma re-semente).
    """
    vals = np.array([measure_kl(p, n_samples, seed + i) for i in range(n_repeat)])
    return float(vals.mean()), float(vals.std(ddof=1) if n_repeat > 1 else 0.0)


def find_target_p_bisection(target=1.0, p_low=0.0, p_high=0.1,
                            n_samples=50000, seed=0, tol_p=1e-4,
                            max_iter=60, n_repeat=5):
    """Metodo B: bissccao em p (tol_p em espaco de p, NAO em KL).

    Correcao empirica: o codigo original convergia com tol KL = 1e-4 apenas
    porque reusava o MESMO seed em toda medicao (ruido mascarado). Com seeds
    frescos, o ruido MC (std ~ 0.02 bit) e ~200x maior que tol KL e o loop
    nunca convergia. Convergencia em espaco de p ignora o ruido da metrica.

    Retorna dict com p_target, KL no ponto, e ruido MC estimado.
    """
    kl_low, _ = measure_kl_repeat(p_low, n_samples, seed, n_repeat)
    kl_high, _ = measure_kl_repeat(p_high, n_samples, seed + 100, n_repeat)
    if not (kl_low <= target <= kl_high):
        raise ValueError(
            f"Target KL={target} fora do bracket "
            f"[{kl_low:.4f}, {kl_high:.4f}].")

    n_iter = 0
    for _ in range(max_iter):
        p_mid = 0.5 * (p_low + p_high)
        kl_mid, _ = measure_kl_repeat(p_mid, n_samples, seed + 200, n_repeat)
        n_iter += 1
        if p_high - p_low < tol_p:
            break
        if kl_mid < target:
            p_low = p_mid
        else:
            p_high = p_mid
    p_target = 0.5 * (p_low + p_high)
    kl_final, kl_std = measure_kl_repeat(p_target, n_samples, seed + 300,
                                         n_repeat)
    return {
        'p_target': float(p_target),
        'kl_target': kl_final,
        'kl_std': kl_std,
        'n_iter': n_iter,
        'tol_p': tol_p,
    }


def write_csv(path, p_vals, kl_vals):
    with open(path, 'w', encoding='utf-8') as fh:
        fh.write("p_flip,d_kl_bits\n")
        for p, k in zip(p_vals, kl_vals):
            fh.write(f"{p:.6f},{k:.6f}\n")


def make_plot(p_vals, kl_vals, target, p_target, p_operate, out_plot):
    import matplotlib
    matplotlib.use('Agg')
    import matplotlib.pyplot as plt

    fig, ax = plt.subplots(1, 1, figsize=(9, 5))
    ax.semilogx(p_vals, kl_vals, 'bo-', label='D_KL(p)')
    ax.axhline(target, color='gray', ls=':', label=f'target {target:.1f} bit')
    ax.axvline(p_target, color='red', ls='--',
               label=f'p_target={p_target:.4f}')
    ax.axvline(p_operate, color='green', ls='-.',
               label=f'p_operate={p_operate:.4f}')
    ax.set_xlabel('Bit-flip probability p')
    ax.set_ylabel('KL divergence (bits)')
    ax.legend()
    ax.grid(True, which='both', alpha=0.3)
    ax.set_title('Target-KL threshold detection (v5.1)')
    fig.tight_layout()
    fig.savefig(out_plot, dpi=150)
    print(f"Grafico salvo: {out_plot}")


def main(argv=None):
    ap = argparse.ArgumentParser(
        description="Deteccao do ponto operacional FPGA via target D_KL.")
    ap.add_argument('--n', type=int, default=50000,
                    help='amostras por medicao (default 50000)')
    ap.add_argument('--n-bins', type=int, default=None,
                    help='numero de bins log do histograma (default N_BINS)')
    ap.add_argument('--target', type=float, default=1.0,
                    help='target D_KL em bits (default 1.0)')
    ap.add_argument('--p-max', type=float, default=0.5)
    ap.add_argument('--n-low', type=int, default=24)
    ap.add_argument('--n-high', type=int, default=8)
    ap.add_argument('--n-repeat', type=int, default=5,
                    help='medicoes independentes p/ ruido MC (bissccao)')
    ap.add_argument('--out', default=None, help='CSV de saida')
    ap.add_argument('--plot', default='threshold_detection_v5_1.png',
                    help='PNG de saida')
    ap.add_argument('--seed', type=int, default=0)
    ap.add_argument('--json', default=None,
                    help='caminho p/ relatorio JSON maquina-legivel')
    ap.add_argument('--sweep-npz', default=None,
                    help='salva KL(p, r) em multiplas resolucoes (npz)')
    ap.add_argument('--resolutions', default='32,64,128,256,512',
                    help='bins de resolucao p/ o sweep npz (csv)')
    args = ap.parse_args(argv)

    if args.sweep_npz:
        import numpy as _np
        res = [int(x) for x in args.resolutions.split(',')]
        p_vals = build_sweep_grid(args.p_max, args.n_low, args.n_high)
        kl_matrix = _np.zeros((len(p_vals), len(res)), dtype=float)
        for i, p in enumerate(p_vals):
            for j, nb in enumerate(res):
                kl_matrix[i, j] = measure_kl(p, args.n, args.seed + i,
                                             n_bins=nb)
        _np.savez(args.sweep_npz, p_vals=p_vals, resolutions=_np.asarray(res),
                  kl_matrix=kl_matrix)
        print(f"KL(p,r) salvo: {args.sweep_npz} "
              f"({len(p_vals)} p x {len(res)} resolucoes)")
        return 0

    p_vals = build_sweep_grid(args.p_max, args.n_low, args.n_high)
    kl_vals = np.array([measure_kl(p, args.n, args.seed + i)
                        for i, p in enumerate(p_vals)])

    p_target = find_target_p(p_vals, kl_vals, args.target)
    p_operate = 0.8 * p_target

    res_b = find_target_p_bisection(target=args.target, n_samples=args.n,
                                    seed=args.seed, n_repeat=args.n_repeat)
    p_target_b = res_b['p_target']
    p_operate_b = 0.8 * p_target_b

    print(f"Amostras/medicao: {args.n} | pontos: {len(p_vals)} | "
          f"target: {args.target:.1f} bit")
    print(f"KL max no sweep: {kl_vals[-1]:.3f} bit (saturacao)")
    print()
    print("Metodo A (spline sobre sweep):")
    print(f"  p_target  (D_KL = {args.target:.1f} bit): p = {p_target:.4f}")
    print(f"  p_operate (= 0.8 x p_target):             p = {p_operate:.4f}")
    print()
    print("Metodo B (bissccao em p, tol_p = {:.1e}):".format(res_b['tol_p']))
    print(f"  p_target  (D_KL = {args.target:.1f} bit): p = {p_target_b:.4f}")
    print(f"  p_operate (= 0.8 x p_target):             p = {p_operate_b:.4f}")
    print(f"  KL no ponto: {res_b['kl_target']:.4f} +/- {res_b['kl_std']:.4f} "
          f"bit (ruido MC) | iter: {res_b['n_iter']}")
    print()
    diff = abs(p_target - p_target_b)
    print(f"Cross-check: |p_target_A - p_target_B| = {diff:.4f} "
          f"({'OK (concordancia)' if diff < 0.005 else 'DIVERGENCIA'})")

    if args.out:
        write_csv(args.out, p_vals, kl_vals)
        print(f"CSV salvo: {args.out}")

    if args.json:
        report = {
            'target_kl': args.target,
            'n_samples': args.n,
            'p_target_a': p_target,
            'p_operate_a': p_operate,
            'p_target_b': p_target_b,
            'p_operate_b': p_operate_b,
            'kl_at_target': res_b['kl_target'],
            'kl_std': res_b['kl_std'],
            'n_iter_b': res_b['n_iter'],
            'cross_check_diff': diff,
            'cross_check_ok': bool(diff < 0.005),
            'kl_max': float(kl_vals[-1]),
        }
        with open(args.json, 'w', encoding='utf-8') as fh:
            json.dump(report, fh, indent=2)
        print(f"JSON salvo: {args.json}")

    if args.plot:
        make_plot(p_vals, kl_vals, args.target, p_target, p_operate,
                  args.plot)

    return 0


if __name__ == '__main__':
    sys.exit(main())
