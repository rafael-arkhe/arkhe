#!/usr/bin/env python3
"""
lensing_inflection_detector_v5.py
Deteccao do ponto de colapso estrutural via D_KL(p) monotona.

Correcoes empiricas incorporadas (validacao numerica 2026-08-11):
  1. D_KL(p) e monotonicamente crescente em p -> busca binaria e valida.
  2. A transicao vive em p ~= 0.03-0.04. Sweep uniforme [0,0.5] com 12 pts
     amostra o primeiro interior em p=0.045 -> PERDE o colapso inteiro.
     -> usar grid log-spaced concentrado no regime baixo-p.
  3. argmax|d2D/dp2| e fragil: cai na borda do ruido plug-in (~0.01 em p=0).
     -> medir KL(0) como piso e subtrair; usar inclinacao maxima (max d1).
  4. p_op = 0.7 * p_collapse mantem o FPGA no regime estavel.

v5.0-emp -- 2026-08-11
"""

import argparse
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
    """alpha = K/b com b de [B_MIN,B_MAX] corrompido por bit-flips em Q16.16."""
    rng = np.random.default_rng(seed)
    b_ideal = rng.uniform(B_MIN, B_MAX, n_samples)
    b_int = np.round(b_ideal * SCALE).astype(np.int64)

    flips = rng.random((n_samples, N_BITS)) < p_flip
    mask = (flips << np.arange(N_BITS)).sum(axis=1).astype(np.int64)
    b_noisy = np.clip((b_int ^ mask).astype(np.float64) / SCALE, B_MIN, B_MAX)
    return np.clip(K_LENS / b_noisy, K_LENS / B_MAX, K_LENS / B_MIN)


def ideal_alpha_samples(n_samples, seed):
    """Referencia ideal = pipeline quantizado SEM ruido (p_flip = 0)."""
    return lensing_alpha(n_samples, seed)


def kl(obs, ref):
    """KL(P_obs || P_ref) em bits."""
    return float(np.sum(obs * np.log2(obs / ref)))


def measure_kl_vs_ideal(p_flip, n_samples=50000, seed=0):
    """KL entre lensing ruidoso (p_flip) e a referencia quantizada sem ruido.

    A referencia ideal usa o MESMO pipeline Q16.16 com p_flip=0, garantindo
    que KL(0) ~= piso plug-in (~bins/(2n)) e nao ~27 bits de um mismatch
    de quantizacao entre histogramas continuos vs discretos.

    Laplace smoothing (+0.5 contagem/bin) evita log2(0): sem ele, bins de
    cauda com poucas contagens produzem KL infinita que domina o sweep.
    """
    a_obs = lensing_noisy_alpha(p_flip, n_samples, seed)
    a_ideal = lensing_alpha(n_samples, seed + 1000)
    a_min, a_max = K_LENS / B_MAX, K_LENS / B_MIN
    edges = np.linspace(np.log10(a_min + EPS), np.log10(a_max + EPS), N_BINS + 1)

    h_obs, _ = np.histogram(np.log10(a_obs + EPS), bins=edges)
    h_ideal = np.histogram(np.log10(a_ideal + EPS), bins=edges)[0]

    h_obs = (h_obs + 0.5) / (h_obs + 0.5).sum()
    h_ideal = (h_ideal + 0.5) / (h_ideal + 0.5).sum()
    mask = h_obs > 0
    return float(np.sum(h_obs[mask] * np.log2(h_obs[mask] / h_ideal[mask])))


def build_sweep_grid(p_max=0.5, n_low=24, n_high=8):
    """Grid log-spaced concentrado no regime baixo-p + cauda uniforme."""
    low = np.geomspace(1e-3, 0.1, n_low)
    high = np.linspace(0.12, p_max, n_high)
    return np.unique(np.concatenate([low, high]))


def find_collapse_threshold(p_vals, kl_vals, floor=None):
    """p_collapse via inclinacao maxima apos subtracao do piso.

    Retorna (p_collapse_steep, p_collapse_curv, floor, kls_floor).
    """
    if floor is None:
        floor = kl_vals[0]
    kls = np.maximum(kl_vals - floor, 0.0)
    d1 = np.gradient(kls, p_vals)
    d2 = np.gradient(d1, p_vals)
    i_steep = int(np.argmax(d1))
    i_curv = int(np.argmax(np.abs(d2)))
    return float(p_vals[i_steep]), float(p_vals[i_curv]), float(floor), kls


def write_csv(path, p_vals, kl_vals):
    with open(path, 'w', encoding='utf-8') as fh:
        fh.write("p_flip,d_kl_bits\n")
        for p, k in zip(p_vals, kl_vals):
            fh.write(f"{p:.6f},{k:.6f}\n")


def make_plot(p_vals, kl_vals, kls_floor, floor, p_steep, p_curv, out_plot):
    import matplotlib
    matplotlib.use('Agg')
    import matplotlib.pyplot as plt

    fig, (ax1, ax2) = plt.subplots(1, 2, figsize=(13, 5))
    ax1.semilogx(p_vals, kl_vals, 'bo-', label='D_KL(p) bruto')
    ax1.axhline(floor, color='gray', ls=':', label=f'piso ruido {floor:.4f}')
    ax1.axvline(p_steep, color='red', ls='--',
                label=f'colapso (max d1) p={p_steep:.4f}')
    ax1.axvline(p_curv, color='orange', ls=':',
                label=f'argmax|d2| p={p_curv:.4f}')
    ax1.set_xlabel('Bit-flip probability p')
    ax1.set_ylabel('KL divergence (bits)')
    ax1.legend(fontsize=8)
    ax1.grid(True, which='both', alpha=0.3)
    ax1.set_title('D_KL monotona (log-p)')

    ax2.semilogx(p_vals, kls_floor, 'go-', label='D_KL apos piso')
    ax2.axvline(p_steep, color='red', ls='--', label='max inclinacao')
    ax2.set_xlabel('Bit-flip probability p')
    ax2.set_ylabel('KL - floor (bits)')
    ax2.legend(fontsize=8)
    ax2.grid(True, which='both', alpha=0.3)
    ax2.set_title('Regime de transicao (piso subtraido)')

    fig.suptitle('Inflection-point detection for structural collapse (v5.0-emp)')
    fig.tight_layout()
    fig.savefig(out_plot, dpi=150)
    print(f"Grafico salvo: {out_plot}")


def main(argv=None):
    ap = argparse.ArgumentParser(
        description="Deteccao do ponto de colapso via D_KL monotona.")
    ap.add_argument('--n', type=int, default=50000,
                    help='amostras por medicao (default 50000)')
    ap.add_argument('--p-max', type=float, default=0.5,
                    help='limite superior do sweep')
    ap.add_argument('--n-low', type=int, default=24,
                    help='pontos log-spaced em p<0.1')
    ap.add_argument('--n-high', type=int, default=8,
                    help='pontos uniformes em p>0.12')
    ap.add_argument('--out', default=None, help='CSV de saida')
    ap.add_argument('--plot', default='inflection_detection_v5.png',
                    help='PNG de saida')
    ap.add_argument('--seed', type=int, default=0,
                    help='semente global do sweep')
    args = ap.parse_args(argv)

    p_vals = build_sweep_grid(args.p_max, args.n_low, args.n_high)
    kl_vals = np.array([measure_kl_vs_ideal(p, args.n, args.seed + i)
                        for i, p in enumerate(p_vals)])

    p_steep, p_curv, floor, kls_floor = find_collapse_threshold(
        p_vals, kl_vals)

    print(f"Amostras/medicao: {args.n} | pontos: {len(p_vals)} | "
          f"piso KL(0): {floor:.4f}")
    print(f"p_collapse (max inclinacao):  p = {p_steep:.4f}  "
          f"-> p_op = {0.7 * p_steep:.4f}")
    print(f"p_collapse (argmax|d2|):      p = {p_curv:.5f}  "
          f"[fragil: rastreia piso de ruido]")

    if args.out:
        write_csv(args.out, p_vals, kl_vals)
        print(f"CSV salvo: {args.out}")

    if args.plot:
        make_plot(p_vals, kl_vals, kls_floor, floor, p_steep, p_curv,
                  args.plot)

    return 0


if __name__ == '__main__':
    sys.exit(main())
