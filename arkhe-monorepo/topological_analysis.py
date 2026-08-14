#!/usr/bin/env python3
"""
topological_analysis.py
Analise topologica DESCRITIVA dos dados KL(p, r) do detector v5.1.

Dependencias: numpy, scipy, matplotlib, networkx (sklearn opcional).
'ripser'/'persim' sao opcionais e usados apenas se instalados.

AVISOS CIENTIFICOS (importantes — validados empiricamente):
  1. D_KL(p) e MONOTONA e satura (~5.7 bits); NAO ha divergencia em lei de
     potencia nem ponto critico de transicao de fase. Portanto o ajuste
     |p - p0|^beta e DESCRITIVO e o expoente beta NAO caracteriza classe de
     universalidade. A rotulagem "Painleve II" foi REMOVIDA (nao ha suporte
     teorico: Painleve II aparece em estatistica de bordo RMT/KPZ, nao em
     uma curva KL monotona).
  2. p_operate = 0.8 * p_target e um ponto operacional de ENGENHARIA (margem),
     nao um ponto de curvatura zero ou transicao. "Universo plano / curvatura
     da informacao" e metafora, nao medicao.
  3. Homologia: grade 2D -> numeros de Betti beta0 (componentes) e beta1
     (ciclos) de conjuntos de subnivel, estimados por grafo de grade.
     NAO e homologia persistente de Vietoris-Rips (usada so se ripser
     estiver instalado; aqui a grade basta).
  4. Dimensao fractal: box-counting do perfil medio KL(p); curva suave e
     monotona -> D_f ~ 1.0. NAO e a dimensao do espectro S2 (que tem apenas
     4 pontos k=1..4 e nao pode ser box-countada).

Uso:
  python topological_analysis.py [--npz fpga_kl_sweep.npz]
                                 [--n 50000] [--res 32,64,128,256,512]
                                 [--plot png] [--json json]
"""

import argparse
import json
import os
import sys

import numpy as np
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
from scipy.ndimage import gaussian_filter
from scipy.optimize import curve_fit

try:
    import networkx as nx
    HAS_NX = True
except ImportError:
    HAS_NX = False

try:
    from ripser import ripser  # noqa: F401  (opcional)
    HAS_RIPSER = True
except ImportError:
    HAS_RIPSER = False

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from lensing_threshold_detector_v5_1 import (  # noqa: E402
    build_sweep_grid, measure_kl,
)


# =============================================================================
# 1. DADOS
# =============================================================================
def load_or_generate_sweep(npz_path, n_samples, resolutions, seed=0):
    """Carrega npz existente ou gera com o detector real v5.1."""
    if npz_path and os.path.exists(npz_path):
        data = np.load(npz_path)
        p_vals = data['p_vals']
        res = data['resolutions']
        kl = data['kl_matrix']
        print(f"Dados carregados: {npz_path} "
              f"({len(p_vals)} p x {len(res)} resolucoes)")
        return p_vals, res, kl

    print(f"Gerando sweep KL(p, r) com o detector v5.1 "
          f"(n={n_samples}, resolucoes={resolutions})...")
    res = np.asarray([int(x) for x in str(resolutions).split(',')])
    p_vals = build_sweep_grid(0.5, 24, 8)
    kl = np.zeros((len(p_vals), len(res)), dtype=float)
    for i, p in enumerate(p_vals):
        for j, nb in enumerate(res):
            kl[i, j] = measure_kl(p, n_samples, seed + i, n_bins=nb)
    if npz_path:
        np.savez(npz_path, p_vals=p_vals, resolutions=res, kl_matrix=kl)
        print(f"Sweep salvo: {npz_path}")
    return p_vals, res, kl


# =============================================================================
# 2. HESSIANO (curvatura DESCRITIVA do mapa KL(p, r))
# =============================================================================
def compute_hessian(kl_matrix, p_vals, resolutions, p_operate):
    """Hessiano numerico de KL(p, r) em p_operate (coordenadas reais).

    Nao ha ponto critico garantido: o reporte dos autovalores e a
    interpretacao de sinal sao DESCRITIVOS do mapa suavizado.
    """
    kl_s = gaussian_filter(kl_matrix, sigma=1.0)

    # np.gradient com coordenadas nao uniformes (log-resolucao)
    dP = np.gradient(kl_s, p_vals, axis=0)
    dR = np.gradient(kl_s, resolutions, axis=1)

    d2P = np.gradient(dP, p_vals, axis=0)
    d2R = np.gradient(dR, resolutions, axis=1)
    dPR = np.gradient(dP, resolutions, axis=1)

    idx_p = int(np.argmin(np.abs(p_vals - p_operate)))
    # media sobre as resolucoes no ponto p_operate (perfil do mapa)
    hessian = np.array([
        [np.mean(d2P[idx_p, :]), np.mean(dPR[idx_p, :])],
        [np.mean(dPR[idx_p, :]), np.mean(d2R[idx_p, :])],
    ])
    eigenvals, eigenvecs = np.linalg.eigh(hessian)
    return hessian, eigenvals, eigenvecs


# =============================================================================
# 3. AJUSTE EM LEI DE POTENCIA (DESCRITIVO — NAO e universalidade)
# =============================================================================
def fit_powerlaw(p_vals, kl_mean, p_operate, window=0.02):
    """Ajusta kl ~ A*|p - p_operate|^beta + C perto de p_operate.

    beta e DESCRITIVO: a curva KL e monotona/saturada, sem ponto critico;
    NAO se infere classe de universalidade nem Painleve II deste numero.
    """
    mask = np.abs(p_vals - p_operate) < window
    p_near = p_vals[mask]
    kl_near = kl_mean[mask]
    if len(p_near) < 5:
        print("  Aviso: <5 pontos na janela de ajuste.")
        return None, None, None, None

    def law(x, beta, A, C):
        return A * np.abs(x - p_operate) ** beta + C

    try:
        popt, pcov = curve_fit(law, p_near, kl_near,
                               p0=[0.5, 1.0, float(np.min(kl_near))],
                               bounds=([0.05, 0.0, -np.inf],
                                       [3.0, np.inf, np.inf]))
        beta, A, C = popt
        beta_err = float(np.sqrt(pcov[0, 0]))
        return beta, beta_err, A, C
    except Exception as exc:  # noqa: BLE001
        print(f"  Aviso: ajuste de lei de potencia falhou ({exc}).")
        return None, None, None, None


# =============================================================================
# 4. HOMOLOGIA DE SUBNIVEL (Betti 0/1 via grafo de grade)
# =============================================================================
def compute_sublevel_betti(kl_matrix, thresholds=None):
    """Betti de conjuntos {KL_norm > theta} na grade 2D.

    So faz sentido se HAS_NX; caso contrario retorna []. beta2 nao existe
    para subconjuntos de uma superficie 2D plana (grade), por isso omitido.
    """
    if not HAS_NX:
        return []
    kl_norm = ((kl_matrix - kl_matrix.min())
               / (kl_matrix.max() - kl_matrix.min() + 1e-12))
    if thresholds is None:
        thresholds = np.linspace(0.1, 0.9, 9)
    out = []
    nr, nc = kl_matrix.shape
    for theta in thresholds:
        G = nx.grid_2d_graph(nr, nc)
        keep = [(i, j) for i in range(nr) for j in range(nc)
                if kl_norm[i, j] > theta]
        remove = [n for n in G.nodes() if n not in keep]
        G.remove_nodes_from(remove)
        if G.number_of_nodes() == 0:
            b0 = b1 = 0
        else:
            b0 = nx.number_connected_components(G)
            # numero de ciclos independentes: E - V + C
            b1 = G.number_of_edges() - G.number_of_nodes() + b0
        out.append((float(theta), int(b0), int(b1)))
    return out


# =============================================================================
# 5. DIMENSAO FRACTAL DO PERFIL KL(p) (box-counting)
# =============================================================================
def compute_fractal_dimension(p_vals, kl_mean, p_operate, window=0.05):
    """Box-counting do perfil medio KL(p) na janela [p_operate -/+ window].

    Perfil suave e monotono -> D_f ~ 1.0 esperado.
    """
    mask = np.abs(p_vals - p_operate) < window
    p_near = p_vals[mask]
    kl_near = kl_mean[mask]
    if len(p_near) < 8:
        print("  Aviso: poucos pontos para box-counting.")
        return None

    kl_norm = ((kl_near - kl_near.min())
               / (kl_near.max() - kl_near.min() + 1e-12))
    pts = np.column_stack([p_near, kl_norm])

    box_sizes = np.arange(2, 14)
    counts = []
    for size in box_sizes:
        xb = np.linspace(p_near.min(), p_near.max(), size + 1)
        yb = np.linspace(0.0, 1.0, size + 1)
        occ = np.zeros((size, size), dtype=bool)
        for x, y in pts:
            ix = min(int(np.digitize(x, xb)) - 1, size - 1)
            iy = min(int(np.digitize(y, yb)) - 1, size - 1)
            if ix >= 0 and iy >= 0:
                occ[ix, iy] = True
        counts.append(int(occ.sum()))
    counts = np.asarray(counts, dtype=float)
    valid = counts > 0
    if valid.sum() < 3:
        return None
    coeffs = np.polyfit(np.log(box_sizes[valid]), np.log(counts[valid]), 1)
    # N(box) ~ box^(-D): slope = -D
    return -float(coeffs[0]), float(coeffs[1])


# =============================================================================
# 6. RELATORIO
# =============================================================================
def generate_report(p_vals, resolutions, kl_matrix, p_operate, out_plot,
                    out_json):
    print("=" * 70)
    print("RELATORIO TOPOLOGICO (DESCRITIVO) — GEOMETRIA DA INFORMACAO")
    print("=" * 70)

    report = {'p_operate': p_operate,
              'n_p': int(len(p_vals)),
              'n_res': int(len(resolutions))}

    # 1. Hessiano
    hess, evals, evecs = compute_hessian(kl_matrix, p_vals, resolutions,
                                         p_operate)
    det = float(np.linalg.det(hess))
    tr = float(np.trace(hess))
    print("\n--- HESSIANO DE KL(p, r) em p_operate (descritivo) ---")
    print(f"  Autovalores : {evals[0]:+.5f}, {evals[1]:+.5f}")
    print(f"  Determinante: {det:+.5f} | Traco: {tr:+.5f}")
    if evals[0] > 0 and evals[1] > 0:
        print("  -> curvatura positiva (minimo local do mapa suavizado)")
    elif evals[0] < 0 and evals[1] < 0:
        print("  -> curvatura negativa (maximo local do mapa suavizado)")
    elif evals[0] * evals[1] < 0:
        print("  -> ponto de sela do mapa suavizado")
    else:
        print("  -> ponto degenerado")
    print("  NOTA: descritivo do mapa suavizado; p_operate e limiar de")
    print("        engenharia, nao um ponto critico de transicao de fase.")
    report['hessian'] = {'eigenvalues': evals.tolist(), 'det': det,
                         'trace': tr}

    # 2. Lei de potencia
    kl_mean = np.mean(kl_matrix, axis=1)
    beta, beta_err, A, C = fit_powerlaw(p_vals, kl_mean, p_operate)
    print("\n--- LEI DE POTENCIA |p - p_operate|^beta (descritivo) ---")
    if beta is not None:
        print(f"  beta = {beta:.4f} +/- {beta_err:.4f}, A = {A:.4f}, "
              f"C = {C:.4f}")
        print("  AVISO: D_KL(p) e monotona e saturada; beta NAO caracteriza")
        print("         classe de universalidade nem Painleve II.")
        report['powerlaw'] = {'beta': beta, 'beta_err': beta_err,
                              'A': A, 'C': C}
    else:
        report['powerlaw'] = None

    # 3. Betti (subnivel)
    betti = compute_sublevel_betti(kl_matrix)
    print("\n--- HOMOLOGIA DE SUBNIVEL (grade 2D; beta0, beta1) ---")
    for theta, b0, b1 in betti:
        print(f"  theta = {theta:.2f}: beta0 = {b0}, beta1 = {b1}")
    report['betti'] = betti

    # 4. Fractal do perfil
    fd = compute_fractal_dimension(p_vals, kl_mean, p_operate)
    print("\n--- DIMENSAO FRACTAL DO PERFIL KL(p) (box-counting) ---")
    if fd is not None:
        d_f, _ = fd
        print(f"  D_f = {d_f:.4f} "
              f"(curva suave/monotona => ~1.0 esperado; NAO e S2)")
        report['fractal_dim'] = d_f
    else:
        report['fractal_dim'] = None

    # 5. Plot
    fig, axes = plt.subplots(1, 2, figsize=(13, 5.5))
    X, Y = np.meshgrid(resolutions, p_vals)
    ax = axes[0]
    cf = ax.contourf(X, Y, kl_matrix, levels=20, cmap='viridis')
    ax.contour(X, Y, kl_matrix, levels=10, colors='white', alpha=0.3)
    ax.axhline(p_operate, color='r', ls='--',
               label=f'p_operate = {p_operate:.4f}')
    ax.set_xlabel('Resolucao (bins)')
    ax.set_ylabel('p (bit-flip prob.)')
    ax.set_title('Mapa KL(p, r)')
    ax.legend()
    plt.colorbar(cf, ax=ax, label='D_KL (bits)')

    ax = axes[1]
    kl_std = np.std(kl_matrix, axis=1)
    ax.errorbar(p_vals, kl_mean, yerr=kl_std, fmt='bo-', capsize=3,
                label='KL medio')
    ax.axvline(p_operate, color='r', ls='--',
               label=f'p_operate = {p_operate:.4f}')
    if beta is not None:
        mask = np.abs(p_vals - p_operate) < 0.02
        if mask.sum() > 2:
            pp = np.linspace(p_vals[mask].min(), p_vals[mask].max(), 50)
            ax.plot(pp, A * np.abs(pp - p_operate) ** beta + C, 'g--',
                    label=f'fit A|p-p0|^beta, beta={beta:.3f}')
    ax.set_xlabel('p')
    ax.set_ylabel('D_KL (bits)')
    ax.set_title('Perfil KL(p) medio')
    ax.legend()
    ax.grid(alpha=0.3)
    fig.suptitle('Geometria da Informacao — analise topologica descritiva')
    fig.tight_layout()
    fig.savefig(out_plot, dpi=150)
    print(f"\nPlot salvo: {out_plot}")

    if out_json:
        with open(out_json, 'w', encoding='utf-8') as fh:
            json.dump(report, fh, indent=2, default=float)
        print(f"JSON salvo: {out_json}")

    return report


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--npz', default='fpga_kl_sweep.npz')
    ap.add_argument('--n', type=int, default=50000,
                    help='amostras/medicao ao gerar o sweep')
    ap.add_argument('--res', default='32,64,128,256,512')
    ap.add_argument('--p-operate', type=float, default=0.0306)
    ap.add_argument('--plot', default='topological_analysis_plot.png')
    ap.add_argument('--json', default='topological_report.json')
    args = ap.parse_args(argv)

    if not HAS_NX:
        print("AVISO: networkx ausente; a secao de Betti sera vazia.")
    if not HAS_RIPSER:
        print("AVISO: ripser ausente; usando subnivel em grade 2D (nao "
              "persistencia de Vietoris-Rips).")

    p_vals, resolutions, kl = load_or_generate_sweep(
        args.npz, args.n, args.res)
    generate_report(p_vals, resolutions, kl, args.p_operate,
                    args.plot, args.json)
    return 0


if __name__ == '__main__':
    sys.exit(main())
