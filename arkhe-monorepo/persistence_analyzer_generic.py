#!/usr/bin/env python3
"""
persistence_analyzer_generic.py
Analise de persistencia (TDA) com janelamento temporal deslizante.

Permite detectar QUANDO um sistema perde estrutura (colapso), monitorando a
evolucao de metricas de persistencia (homologia H0) ao longo de janelas
sobrepostas de uma serie temporal / nuvem de pontos.

v3.0-window -- 2026-08-11

Uso:
  python persistence_analyzer_generic.py --data sensor.log
  python persistence_analyzer_generic.py --data sensor.log --window 1000 --step 500 --plot

Entrada:
  - Serie 1D (um valor por linha): usa persistencia de subnivel (merge tree),
    O(n log n), exata.
  - Nuvem de pontos (N linhas x D colunas numericas): usa Rips-H0 via union-find.
  - Opcional --embed D --delay T: embedding de Takens para series 1D.

Saida:
  - CSV com as metricas por janela (--out).
  - Grafico PNG com a evolucao da metrica e janelas de colapso (--plot).
  - Deteccao de colapso: janelas cuja metrica cai abaixo de (media - zscore*std)
    das janelas anteriores.
"""

import argparse
import os
import sys

import numpy as np


# ============================================================
# CARREGAMENTO DE DADOS
# ============================================================

def load_data(path, sep=None):
    """Carrega valores numericos de um arquivo (aceita cabecalho/linhas #)."""
    rows = []
    with open(path, 'r', encoding='utf-8', errors='ignore') as fh:
        for line in fh:
            line = line.strip()
            if not line or line.startswith('#') or line.startswith('//'):
                continue
            parts = line.split(sep) if sep else line.replace(',', ' ').split()
            try:
                rows.append([float(p) for p in parts])
            except ValueError:
                continue
    arr = np.asarray(rows, dtype=float)
    if arr.size == 0:
        raise ValueError(f"Nenhum dado numerico encontrado em {path}")
    if arr.ndim == 1:
        arr = arr.reshape(-1, 1)
    return arr


def select_series(arr, column):
    """Retorna serie 1D (coluna) ou usa todas as colunas como nuvem."""
    if arr.shape[1] == 1:
        return arr[:, 0], None
    if column is not None and 0 <= column < arr.shape[1]:
        return arr[:, column], None
    return None, arr


def takens_embed(series, dim, delay):
    """Embedding de Takens: serie 1D -> nuvem em R^dim."""
    n = series.size
    m = n - (dim - 1) * delay
    if m <= 0:
        raise ValueError("Embedding invalido: janela menor que (dim-1)*delay+1")
    idx = np.arange(m)[:, None] + np.arange(dim)[None, :] * delay
    return series[idx]


# ============================================================
# PERSISTENCIA H0
# ============================================================

def _uf_find(parent, x):
    while parent[x] != x:
        parent[x] = parent[parent[x]]
        x = parent[x]
    return x


def sublevel_h0_bars(values):
    """Barras H0 da filtracao de subnivel de uma funcao 1D (merge tree).

    Exato via union-find. Retorna lista de (birth, death) para barras
    finitas (a barra infinita nao e incluida).
    """
    n = values.size
    order = sorted(range(n), key=lambda i: values[i])
    parent = list(range(n))
    birth = values.copy()
    active = np.zeros(n, dtype=bool)
    bars = []

    i = 0
    while i < n:
        v = float(values[order[i]])
        j = i
        while j < n and float(values[order[j]]) == v:
            active[order[j]] = True
            j += 1
        for k in range(i, j):
            idx = order[k]
            for nb in (idx - 1, idx + 1):
                if 0 <= nb < n and active[nb]:
                    r1 = _uf_find(parent, idx)
                    r2 = _uf_find(parent, nb)
                    if r1 == r2:
                        continue
                    if birth[r1] < birth[r2]:
                        bars.append((float(birth[r2]), v))
                        parent[r2] = r1
                    elif birth[r1] > birth[r2]:
                        bars.append((float(birth[r1]), v))
                        parent[r1] = r2
                    else:
                        if r1 < r2:
                            bars.append((float(birth[r2]), v))
                            parent[r2] = r1
                        else:
                            bars.append((float(birth[r1]), v))
                            parent[r1] = r2
        i = j
    return bars


def rips_h0_bars(points):
    """Barras H0 de Vietoris-Rips 0-esqueleto (union-find)."""
    from scipy.spatial.distance import pdist, squareform
    n = points.shape[0]
    if n < 2:
        return []
    D = squareform(pdist(points))
    edges = np.array([(D[i, j], i, j) for i in range(n) for j in range(i + 1, n)],
                     dtype=float)
    edges = edges[np.argsort(edges[:, 0])]
    parent = list(range(n))
    bars = []
    for w, a, b in edges:
        ra, rb = int(a), int(b)
        r1 = _uf_find(parent, ra)
        r2 = _uf_find(parent, rb)
        if r1 != r2:
            bars.append((0.0, float(w)))
            parent[r2] = r1
    return bars


def compute_bars(series, cloud, embed, delay):
    """Escolhe o pipeline conforme o tipo de dados e retorna as barras H0."""
    if cloud is not None:
        return rips_h0_bars(cloud)
    if embed is not None and embed > 1:
        return rips_h0_bars(takens_embed(series, embed, delay))
    return sublevel_h0_bars(series)


# ============================================================
# METRICAS DE PERSISTENCIA
# ============================================================

METRICS = ('entropy_norm', 'entropy', 'total_lifetime', 'max_lifetime', 'n_bars')


def persistence_metrics(bars):
    """Metricas de persistencia a partir das barras H0 finitas."""
    lengths = np.array([d - b for b, d in bars], dtype=float)
    lengths = lengths[lengths > 0]
    out = {'n_bars': int(lengths.size), 'total_lifetime': 0.0,
           'max_lifetime': 0.0, 'entropy': 0.0, 'entropy_norm': 0.0}
    if lengths.size == 0:
        return out
    total = float(lengths.sum())
    out['total_lifetime'] = total
    out['max_lifetime'] = float(lengths.max())
    if lengths.size > 1:
        p = lengths / total
        ent = float(-np.sum(p * np.log(p)))
        out['entropy'] = ent
        out['entropy_norm'] = ent / np.log(lengths.size)
    elif lengths.size == 1:
        out['entropy'] = 0.0
        out['entropy_norm'] = 0.0
    return out


# ============================================================
# JANELAMENTO + DETECCAO DE COLAPSO
# ============================================================

def sliding_windows(n, window, step):
    """Gerador de (inicio, fim) para janelas deslizantes sobrepostas."""
    if window is None or window <= 0:
        yield 0, n
        return
    start = 0
    while start + window <= n:
        yield start, start + window
        start += step


def detect_collapses(metric_vals, zscore, warmup=5, lookback=None):
    """Janelas com queda de metrica > zscore*std em relacao ao historico."""
    flags = []
    for t in range(warmup, len(metric_vals)):
        base = metric_vals[max(0, t - lookback):t] if lookback else metric_vals[:t]
        mu = float(np.mean(base)) if len(base) else np.nan
        sd = float(np.std(base)) if len(base) > 1 else 0.0
        if not np.isnan(mu) and sd > 1e-12 and metric_vals[t] < mu - zscore * sd:
            flags.append(t)
    return flags


# ============================================================
# SAIDA / PLOT
# ============================================================

def write_csv(out_path, rows, metric, window):
    header = ("window_start,window_end,midpoint,metric_name,metric_value\n")
    with open(out_path, 'w', encoding='utf-8') as fh:
        fh.write(header)
        for r in rows:
            fh.write(f"{r['start']},{r['end']},{r['mid']},{metric},{r['value']:.6f}\n")


def make_plot(data_x, data_y, rows, metric, flags, out_plot):
    import matplotlib
    matplotlib.use('Agg')
    import matplotlib.pyplot as plt

    fig, (ax1, ax2) = plt.subplots(2, 1, figsize=(10, 7), sharex=False)

    ax1.plot(np.arange(len(data_y)), data_y, lw=0.8, color='#444')
    ax1.set_title('Dados de entrada')
    ax1.set_ylabel('valor')

    ax2.plot([r['mid'] for r in rows], [r['value'] for r in rows],
             lw=1.4, color='#0b62a4', marker='o', ms=3)
    if flags:
        fx = [rows[f]['mid'] for f in flags]
        fy = [rows[f]['value'] for f in flags]
        ax2.scatter(fx, fy, s=70, marker='x', color='red',
                    label=f'{len(flags)} colapso(s)')
        ax2.legend()
    ax2.set_title(f'Evolucao da metrica de persistencia ({metric})')
    ax2.set_xlabel('meio da janela (indice)')
    ax2.set_ylabel(metric)
    ax2.grid(True, alpha=0.3)

    fig.tight_layout()
    fig.savefig(out_plot, dpi=120)
    print(f"Grafico salvo: {out_plot}")


# ============================================================
# MAIN
# ============================================================

def main(argv=None):
    ap = argparse.ArgumentParser(
        description="Analise de persistencia com janelamento temporal.")
    ap.add_argument('--data', required=True, help='arquivo de dados numericos')
    ap.add_argument('--window', type=int, default=None,
                    help='tamanho da janela deslizante')
    ap.add_argument('--step', type=int, default=None,
                    help='passo entre janelas (default = window)')
    ap.add_argument('--column', type=int, default=None,
                    help='coluna a usar quando ha multiplas colunas')
    ap.add_argument('--sep', default=None, help='separador de colunas')
    ap.add_argument('--embed', type=int, default=None,
                    help='dimensao do embedding de Takens (serie 1D)')
    ap.add_argument('--delay', type=int, default=1,
                    help='delay do embedding de Takens')
    ap.add_argument('--metric', default='entropy_norm',
                    choices=list(METRICS), help='metrica de persistencia')
    ap.add_argument('--zscore', type=float, default=2.0,
                    help='limiar de deteccao de colapso')
    ap.add_argument('--lookback', type=int, default=None,
                    help='janela de historico para deteccao (default: tudo)')
    ap.add_argument('--out', default=None, help='CSV de saida')
    ap.add_argument('--plot', default=None, help='PNG de saida (--plot gera)')
    ap.add_argument('--warmup', type=int, default=5,
                    help='janelas iniciais ignoradas na deteccao')
    args = ap.parse_args(argv)

    if args.plot is None and args.out is None:
        args.plot = os.path.splitext(args.data)[0] + '_persistence.png'

    arr = load_data(args.data, args.sep)
    series, cloud = select_series(arr, args.column)

    step = args.step if args.step else (args.window if args.window else arr.shape[0])
    rows = []
    for start, end in sliding_windows(arr.shape[0], args.window, step):
        if series is not None:
            win = series[start:end]
            bars = compute_bars(win, None, args.embed, args.delay)
        else:
            bars = rips_h0_bars(arr[start:end])
        m = persistence_metrics(bars)
        rows.append({'start': start, 'end': end, 'mid': (start + end) / 2.0,
                     'value': m[args.metric]})

    if args.out:
        write_csv(args.out, rows, args.metric, args.window)

    print(f"Dados: {arr.shape[0]} linhas x {arr.shape[1]} colunas | "
          f"janela={args.window or 'tudo'} passo={step}")
    print(f"Pipeline: {'Rips-H0 (nuvem)' if cloud is not None else
                      (f'Takens(d={args.embed}) Rips-H0' if (series is not None and args.embed)
                       else 'merge-tree H0 (serie 1D)')}")
    print(f"Metrica: {args.metric} | janelas: {len(rows)}")

    vals = np.array([r['value'] for r in rows])
    if len(vals):
        print(f"  min={vals.min():.5f}  max={vals.max():.5f}  "
              f"media={vals.mean():.5f}  std={vals.std():.5f}")

    flags = detect_collapses(vals, args.zscore, args.warmup, args.lookback)
    if flags:
        print(f"Colapsos detectados ({len(flags)}): janelas iniciando em "
              + ", ".join(str(rows[f]['start']) for f in flags))
    else:
        print("Nenhum colapso detectado.")

    if args.plot:
        data_y = series if series is not None else arr[:, 0]
        make_plot(np.arange(data_y.size), data_y, rows, args.metric,
                  flags, args.plot)

    return 0


if __name__ == '__main__':
    sys.exit(main())
