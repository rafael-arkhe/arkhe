"""
ARKHE-RSI-v3.3 — Recursao de Interferencia com Ensemble
============================================================================
Aprimoramentos sobre v3.1 (as tres melhorias acordadas):
  1. --eig-jitter EPS : rho <- (1-eps)*rho + eps*I/dim antes de eigvalsh,
     estabilizando o gradiente perto de autovalores degenerados (0 = off).
  2. Modo "explore" : MAXIMIZA o rank efetivo (1/Tr rho^2). Implementado como
     MINIMIZAR a pureza Tr(rho^2) -- objetivo suave e estavel, matematicamente
     equivalente a maximizar 1/P, mas sem o 1/P^2 explosivo no gradiente.
     (Modo "sharp" continua minimizando S(rho) -> estado puro.)
  3. Visualizacao interativa : todas as espirais sao PRE-COMPUTADAS em arrays
     NumPy; o callback do slider so faz set_data/set_sizes -- zero JAX no GUI.

Otimizadores: adam (default) e sgd-momentum, ambos com gradient clipping global.
Medicao honesta: loss usa chave de ensemble VARIAVEL (SGD estocastico); as
metricas de trajetoria usam uma chave FIXA (dinamica pura); o piso de ruido de
amostragem e medido separadamente ao final.

Uso:
  python arkhe_rsi_v3_3.py --mode sharp   --steps 80  --eig-jitter 1e-6
  python arkhe_rsi_v3_3.py --mode explore --steps 150 --optimizer adam
  python arkhe_rsi_v3_3.py --mode sharp   --steps 60  --interactive
============================================================================
"""

import os
import argparse
import datetime
from functools import partial
from typing import NamedTuple, Any
import warnings

warnings.filterwarnings("ignore", category=UserWarning, module="matplotlib")

import jax
import jax.numpy as jnp
from jax import vmap, random, jit, value_and_grad
import numpy as np

import matplotlib
matplotlib.use("Agg")  # padrao headless-safe; trocado sob --interactive
import matplotlib.pyplot as plt
from matplotlib.widgets import Slider, Button

# ==============================================================================
# CONFIGURACAO
# ==============================================================================
DEFAULT_DIM = 127
DEFAULT_ENSEMBLE_N = 64
DEFAULT_SEED = 42
DEFAULT_PERT = 0.5
DEFAULT_LR = 0.1
DEFAULT_STEPS = 80
DEFAULT_MODE = "sharp"
DEFAULT_OPTIMIZER = "adam"
DEFAULT_OUTPUT_DIR = "."
DEFAULT_CLIP_NORM = 10.0
DEFAULT_EIG_JITTER = 1e-6
FIXED_ENSEMBLE_KEY = random.PRNGKey(2024)  # chave fixa p/ medir dinamica pura

# ==============================================================================
# ESTADO RSI (pytree registrado)
# ==============================================================================


class RSIState:
    __slots__ = ("B", "W1", "b1", "W2", "b2", "Lambda", "I", "J", "dim")

    def __init__(self, dim, key=None):
        if key is None:
            key = random.PRNGKey(DEFAULT_SEED)
        k_B, k_W1, k_W2, k_L, k_I, k_J = random.split(key, 6)
        self.B = random.normal(k_B, (dim, dim)) * 0.05
        self.W1 = random.normal(k_W1, (dim, dim)) * 0.1
        self.b1 = jnp.zeros(dim)
        self.W2 = random.normal(k_W2, (dim, dim)) * 0.1
        self.b2 = jnp.zeros(dim)
        self.Lambda = random.normal(k_L, (dim,)) * 0.1
        self.I = self.Lambda + random.normal(k_I, (dim,)) * DEFAULT_PERT  # I != J
        self.J = self.Lambda + random.normal(k_J, (dim,)) * DEFAULT_PERT
        self.dim = dim

    def tree_flatten(self):
        children = (self.B, self.W1, self.b1, self.W2, self.b2,
                    self.Lambda, self.I, self.J)
        return children, {"dim": self.dim}

    @classmethod
    def tree_unflatten(cls, aux, children):
        obj = cls.__new__(cls)
        (obj.B, obj.W1, obj.b1, obj.W2, obj.b2,
         obj.Lambda, obj.I, obj.J) = children
        obj.dim = aux["dim"]
        return obj

    def replace(self, **kw):
        obj = RSIState.__new__(RSIState)
        for f in ("B", "W1", "b1", "W2", "b2", "Lambda", "I", "J", "dim"):
            setattr(obj, f, kw.get(f, getattr(self, f)))
        return obj


jax.tree_util.register_pytree_node(
    RSIState, lambda s: s.tree_flatten(), RSIState.tree_unflatten)

# ==============================================================================
# OPERADORES / ENSEMBLE / MATRIZ DE DENSIDADE
# ==============================================================================


def R_operator(x, W1, b1, W2, b2):
    h = jnp.tanh(W1 @ x + b1)
    return jnp.tanh(W2 @ h + b2)


def membrane(I, J, B, W1, b1, W2, b2):
    BI, BJ = B @ I, B @ J
    return (R_operator(BI + BJ, W1, b1, W2, b2)
            - R_operator(BI, W1, b1, W2, b2)
            - R_operator(BJ, W1, b1, W2, b2))


def sample_pair(Lambda, key, pert=DEFAULT_PERT):
    k1, k2 = random.split(key)
    I = Lambda + random.normal(k1, Lambda.shape) * pert
    J = Lambda + random.normal(k2, Lambda.shape) * pert
    return I, J


def _density_term(Lambda, B, W1, b1, W2, b2, key, pert):
    I, J = sample_pair(Lambda, key, pert)
    M = membrane(I, J, B, W1, b1, W2, b2)
    return jnp.outer(M, M) / (jnp.dot(M, M) + 1e-10)


@partial(jit, static_argnames=("n_samples", "pert"))
def density_matrix(Lambda, B, W1, b1, W2, b2, key,
                   n_samples=DEFAULT_ENSEMBLE_N, pert=DEFAULT_PERT):
    keys = random.split(key, n_samples)
    terms = vmap(lambda k: _density_term(Lambda, B, W1, b1, W2, b2, k, pert))(keys)
    rho = jnp.mean(terms, axis=0)
    rho = (rho + rho.T) / 2
    return rho / (jnp.trace(rho) + 1e-10)

# ==============================================================================
# METRICAS
# ==============================================================================


def von_neumann_entropy(rho, eig_jitter=0.0):
    """S(rho) = -Tr(rho log rho), com regularizacao espectral opcional.

    eig_jitter>0: rho <- (1-eps)*rho + eps*I/dim  (convexo, preserva traco=1),
    afastando autovalores de 0 e estabilizando o gradiente de eigvalsh.
    """
    if eig_jitter > 0.0:
        d = rho.shape[0]
        rho = (1.0 - eig_jitter) * rho + eig_jitter * jnp.eye(d) / d
    ev = jnp.clip(jnp.linalg.eigvalsh(rho), 1e-12, 1.0)
    return -jnp.sum(ev * jnp.log(ev))


def purity(rho):
    return jnp.trace(rho @ rho)


def effective_rank(rho):
    """Rank de participacao 1/Tr(rho^2) — sem threshold, robusto."""
    return 1.0 / (purity(rho) + 1e-10)


def fidelity(I, J):
    return jnp.abs(jnp.dot(I, J)) ** 2 / (jnp.dot(I, I) * jnp.dot(J, J) + 1e-10)


def visibility(M):
    M2 = M ** 2
    hi, lo = jnp.max(M2), jnp.min(M2)
    return (hi - lo) / (hi + lo + 1e-10)

# ==============================================================================
# LOSS SOBRE A TUPLA DE PARAMETROS
# ==============================================================================
# params = (B, W1, b1, W2, b2, Lambda)


def _rho_from_params(params, key, n_samples, pert):
    B, W1, b1, W2, b2, Lambda = params
    return density_matrix(Lambda, B, W1, b1, W2, b2, key, n_samples, pert)


def _loss(params, key, n_samples, pert, mode, eig_jitter):
    rho = _rho_from_params(params, key, n_samples, pert)
    if mode == "sharp":
        return von_neumann_entropy(rho, eig_jitter)         # minimiza S -> puro
    # explore: minimizar pureza == maximizar rank efetivo (1/P). Suave/estavel.
    return purity(rho)

# ==============================================================================
# OTIMIZADORES (NamedTuple => pytrees validos, jit-compativeis)
# ==============================================================================


class AdamState(NamedTuple):
    m: Any
    v: Any
    t: Any


class SgdState(NamedTuple):
    v: Any


def adam_init(params):
    z = lambda: jax.tree_util.tree_map(jnp.zeros_like, params)
    return AdamState(m=z(), v=z(), t=jnp.asarray(0, jnp.int32))


def sgd_init(params):
    return SgdState(v=jax.tree_util.tree_map(jnp.zeros_like, params))


@partial(jit, static_argnames=("beta1", "beta2", "eps"))
def adam_update(params, grads, st, lr, beta1=0.9, beta2=0.999, eps=1e-8):
    t = st.t + 1
    m = jax.tree_util.tree_map(lambda m, g: beta1 * m + (1 - beta1) * g, st.m, grads)
    v = jax.tree_util.tree_map(lambda v, g: beta2 * v + (1 - beta2) * g * g, st.v, grads)
    bc1, bc2 = 1 - beta1 ** t, 1 - beta2 ** t
    new = jax.tree_util.tree_map(
        lambda p, mm, vv: p - lr * (mm / bc1) / (jnp.sqrt(vv / bc2) + eps),
        params, m, v)
    return new, AdamState(m=m, v=v, t=t)


@partial(jit, static_argnames=("mu",))
def sgd_update(params, grads, st, lr, mu=0.9):
    v = jax.tree_util.tree_map(lambda v, g: mu * v - lr * g, st.v, grads)
    new = jax.tree_util.tree_map(lambda p, vv: p + vv, params, v)
    return new, SgdState(v=v)


def _global_norm(grads):
    return jnp.sqrt(sum(jnp.sum(g * g)
                        for g in jax.tree_util.tree_leaves(grads)))


def _clip(grads, clip_norm):
    gnorm = _global_norm(grads)
    coeff = jnp.minimum(1.0, clip_norm / (gnorm + 1e-10))
    return jax.tree_util.tree_map(lambda g: g * coeff, grads), gnorm

# ==============================================================================
# TREINO
# ==============================================================================


def train(state, key, steps=DEFAULT_STEPS, lr=DEFAULT_LR, mode=DEFAULT_MODE,
          optimizer=DEFAULT_OPTIMIZER, n_samples=DEFAULT_ENSEMBLE_N,
          pert=DEFAULT_PERT, clip_norm=DEFAULT_CLIP_NORM,
          eig_jitter=DEFAULT_EIG_JITTER):
    params = (state.B, state.W1, state.b1, state.W2, state.b2, state.Lambda)
    opt_state, opt_update = ((adam_init(params), adam_update)
                             if optimizer == "adam"
                             else (sgd_init(params), sgd_update))

    hist = {k: [] for k in ("S", "P", "eff_rank", "F", "V", "M_norm",
                            "loss", "grad_norm", "states")}

    for _ in range(steps):
        key, sub = random.split(key)
        # Gradiente contra a chave de ensemble FIXA -> objetivo deterministico e
        # reproduzivel. Necessario para o modo explore: com chave estocastica por
        # passo, o gradiente de pureza e ruidoso e o sistema colapsa no atrator
        # alinhado (pureza sobe). O modo sharp funciona nas duas formas. A
        # generalizacao para outras chaves e medida no piso de ruido em [2].
        loss, grads = value_and_grad(_loss)(
            params, FIXED_ENSEMBLE_KEY, n_samples, pert, mode, eig_jitter)
        grads, gnorm = _clip(grads, clip_norm)      # gnorm = norma REAL do grad
        params, opt_state = opt_update(params, grads, opt_state, lr)

        B, W1, b1, W2, b2, Lambda = params
        newI, newJ = sample_pair(Lambda, sub, pert)  # chave variavel -> F varia
        st = state.replace(B=B, W1=W1, b1=b1, W2=W2, b2=b2,
                           Lambda=Lambda, I=newI, J=newJ)

        rho = density_matrix(Lambda, B, W1, b1, W2, b2,
                             FIXED_ENSEMBLE_KEY, n_samples, pert)
        M = membrane(newI, newJ, B, W1, b1, W2, b2)
        hist["S"].append(float(von_neumann_entropy(rho, eig_jitter)))
        hist["P"].append(float(purity(rho)))
        hist["eff_rank"].append(float(effective_rank(rho)))
        hist["F"].append(float(fidelity(newI, newJ)))
        hist["V"].append(float(visibility(M)))
        hist["M_norm"].append(float(jnp.linalg.norm(M)))
        hist["loss"].append(float(loss))
        hist["grad_norm"].append(float(gnorm))
        hist["states"].append(st)

    return hist

# ==============================================================================
# VISUALIZACAO
# ==============================================================================


def _precompute_spirals(states):
    """Pre-computa (x,y) de cada espiral em NumPy puro (zero JAX no callback)."""
    theta = np.linspace(0, 4 * np.pi, 500)
    r = np.linspace(0.1, 5, 500)
    out = []
    for st in states:
        M = np.asarray(membrane(st.I, st.J, st.B, st.W1, st.b1, st.W2, st.b2))
        amp = 1 + 0.3 * np.sin(theta * 3 + np.linalg.norm(M[:2]) * theta)
        out.append((r * np.cos(theta) * amp, r * np.sin(theta) * amp))
    return out


def plot_static(hist, out_path, mode):
    steps = len(hist["S"])
    x, y = _precompute_spirals(hist["states"][-1:])[0]
    fig, ax = plt.subplots(2, 3, figsize=(18, 10))
    ax[0, 0].plot(x, y, lw=0.8, color="#00d4ff")
    ax[0, 0].set_title("Arkhe Spiral (final)"); ax[0, 0].axis("off")
    ax[0, 0].set_aspect("equal")
    ax[0, 1].plot(hist["S"], "o-", color="#ff6b6b", ms=3)
    ax[0, 1].set_title(f"S(rho) — {mode}"); ax[0, 1].grid(True, alpha=0.3)
    ax[0, 2].plot(hist["P"], "o-", color="#4ecdc4", ms=3)
    ax[0, 2].set_title("Pureza P = Tr(rho^2)"); ax[0, 2].grid(True, alpha=0.3)
    ax[1, 0].plot(hist["eff_rank"], "o-", color="#a29bfe", ms=3)
    ax[1, 0].set_title("Rank efetivo 1/P"); ax[1, 0].grid(True, alpha=0.3)
    ax[1, 1].plot(hist["F"], "^-", color="#ffe66d", ms=3, label="F")
    ax[1, 1].plot(hist["V"], "d-", color="#ff8c42", ms=3, label="V")
    ax[1, 1].set_title("Fidelidade / Visibilidade"); ax[1, 1].legend()
    ax[1, 1].grid(True, alpha=0.3)
    sc = ax[1, 2].scatter(hist["F"], hist["S"], c=range(steps), cmap="viridis", s=40)
    ax[1, 2].set_title("Fidelidade vs S(rho)"); ax[1, 2].grid(True, alpha=0.3)
    fig.colorbar(sc, ax=ax[1, 2], label="passo")
    plt.tight_layout()
    plt.savefig(out_path, dpi=150, bbox_inches="tight", facecolor="#0a0a0a")
    plt.close(fig)
    print(f"    figura salva em {out_path}")


def _try_gui_backend():
    for backend in ("QtAgg", "Qt5Agg", "TkAgg"):
        try:
            matplotlib.use(backend, force=True)
            import matplotlib.pyplot as _p
            globals()["plt"] = _p
            return True
        except Exception:
            continue
    return False


def interactive_plot(hist, out_path=None, mode="sharp"):
    steps = len(hist["S"])
    if steps == 0:
        print("Sem dados."); return
    spirals = _precompute_spirals(hist["states"])  # pre-computa ANTES do GUI
    if not _try_gui_backend():
        print("    [!] Sem backend GUI (headless); gerando figura estatica.")
        if out_path:
            plot_static(hist, out_path, mode)
        return

    S, P, F, V = hist["S"], hist["P"], hist["F"], hist["V"]
    fig, axes = plt.subplots(2, 2, figsize=(14, 10))
    plt.subplots_adjust(left=0.08, bottom=0.25, right=0.95, top=0.93)
    ax_sp, ax_S, ax_P, ax_sc = axes[0, 0], axes[0, 1], axes[1, 0], axes[1, 1]

    slider = Slider(plt.axes([0.15, 0.10, 0.65, 0.03]), "Passo",
                    0, steps - 1, valinit=steps - 1, valfmt="%d", valstep=1)
    xx, yy = spirals[-1]
    sp_line, = ax_sp.plot(xx, yy, lw=0.8, color="#00d4ff")
    ax_sp.set_title(f"Arkhe Spiral — S={S[-1]:.4f}")
    ax_sp.set_aspect("equal"); ax_sp.axis("off")
    ax_S.plot(range(steps), S, "-", color="#ff6b6b", lw=1.5)
    ax_S.set_title("Entropia S(rho)"); ax_S.grid(True, alpha=0.3)
    ax_P.plot(range(steps), P, "-", color="#4ecdc4", lw=1.5, label="P")
    ax_P.plot(range(steps), V, "-", color="#ffe66d", lw=1.5, label="V")
    ax_P.set_title("Pureza / Visibilidade"); ax_P.legend(); ax_P.grid(True, alpha=0.3)
    sc = ax_sc.scatter(F, S, c=range(steps), cmap="viridis", s=40)
    ax_sc.set_title("Fidelidade vs Entropia"); ax_sc.grid(True, alpha=0.3)
    fig.colorbar(sc, ax=ax_sc, label="passo")
    vS = ax_S.axvline(steps - 1, color="gray", ls="--", alpha=0.5)
    vP = ax_P.axvline(steps - 1, color="gray", ls="--", alpha=0.5)
    dS, = ax_S.plot([steps - 1], [S[-1]], "ro", ms=8)
    dP, = ax_P.plot([steps - 1], [P[-1]], "ro", ms=8)

    def update(_):  # PURO NumPy — nenhuma chamada JAX aqui
        i = int(slider.val)
        x, y = spirals[i]
        sp_line.set_data(x, y)
        ax_sp.set_title(f"Arkhe Spiral — S={S[i]:.4f}")
        dS.set_data([i], [S[i]]); dP.set_data([i], [P[i]])
        vS.set_xdata([i]); vP.set_xdata([i])
        sizes = np.full(steps, 30); sizes[i] = 140
        sc.set_sizes(sizes)
        fig.canvas.draw_idle()

    slider.on_changed(update)
    btn = Button(plt.axes([0.85, 0.05, 0.08, 0.04]), "Reset")
    btn.on_clicked(lambda _e: slider.set_val(steps - 1))
    if out_path:
        plt.savefig(out_path, dpi=150, bbox_inches="tight", facecolor="#0a0a0a")
        print(f"    figura salva em {out_path}")
    plt.show()

# ==============================================================================
# EVENTO NOSTR (kind 39000) — apenas montagem local; NAO publica
# ==============================================================================


def prepare_nostr_event(state, key, n_samples=DEFAULT_ENSEMBLE_N, pert=DEFAULT_PERT,
                        mode="trained"):
    rho = density_matrix(state.Lambda, state.B, state.W1, state.b1,
                         state.W2, state.b2, key, n_samples, pert)
    M = membrane(state.I, state.J, state.B, state.W1, state.b1, state.W2, state.b2)
    return {"kind": 39000, "content": {
        "s_measure": float(von_neumann_entropy(rho)),
        "purity": float(purity(rho)),
        "effective_rank": float(effective_rank(rho)),
        "visibility": float(visibility(M)),
        "timestamp": datetime.datetime.now(datetime.timezone.utc)
                     .isoformat().replace("+00:00", "Z"),
        "version": "RSI-v3.3", "dim": int(state.dim), "mode": mode}}

# ==============================================================================
# MAIN
# ==============================================================================


def main():
    p = argparse.ArgumentParser(description="ARKHE-RSI-v3.3")
    p.add_argument("--mode", default=DEFAULT_MODE, choices=["sharp", "explore"])
    p.add_argument("--optimizer", default=DEFAULT_OPTIMIZER, choices=["adam", "sgd"])
    p.add_argument("--steps", type=int, default=DEFAULT_STEPS)
    p.add_argument("--lr", type=float, default=None,
                   help="taxa de aprendizagem (default: sharp=0.1, explore=0.02 "
                        "-- explore diverge com lr alto)")
    p.add_argument("--ensemble", type=int, default=DEFAULT_ENSEMBLE_N)
    p.add_argument("--dim", type=int, default=DEFAULT_DIM)
    p.add_argument("--seed", type=int, default=DEFAULT_SEED)
    p.add_argument("--eig-jitter", type=float, default=DEFAULT_EIG_JITTER,
                   help="eps p/ rho<-(1-eps)rho+eps I/dim antes de eigvalsh (0=off)")
    p.add_argument("--output", default=DEFAULT_OUTPUT_DIR)
    p.add_argument("--interactive", action="store_true")
    p.add_argument("--no-plot", action="store_true")
    p.add_argument("--nostr", action="store_true")
    args = p.parse_args()
    if args.lr is None:               # lr default depende do modo
        args.lr = 0.02 if args.mode == "explore" else 0.10

    os.makedirs(args.output, exist_ok=True)
    print("=" * 70)
    print(f"ARKHE-RSI-v3.3 — modo={args.mode}  opt={args.optimizer}  "
          f"eig_jitter={args.eig_jitter}")
    print("=" * 70)

    key = random.PRNGKey(args.seed)
    state = RSIState(args.dim, key)

    obj = "S(rho)" if args.mode == "sharp" else "rank efetivo (via pureza)"
    print(f"\n[1] Treinando ({args.mode}: {obj}, {args.steps} passos, "
          f"lr={args.lr})...")
    hist = train(state, key, steps=args.steps, lr=args.lr, mode=args.mode,
                 optimizer=args.optimizer, n_samples=args.ensemble,
                 eig_jitter=args.eig_jitter)

    print(f"    S(rho):    {hist['S'][0]:.4f} -> {hist['S'][-1]:.4f}  "
          f"(delta {hist['S'][-1]-hist['S'][0]:+.4f})")
    print(f"    P:         {hist['P'][0]:.4f} -> {hist['P'][-1]:.4f}")
    print(f"    eff_rank:  {hist['eff_rank'][0]:.2f} -> {hist['eff_rank'][-1]:.2f}"
          f"  (delta {hist['eff_rank'][-1]-hist['eff_rank'][0]:+.2f})")
    print(f"    ||grad||:  {hist['grad_norm'][0]:.4e} -> {hist['grad_norm'][-1]:.4e}")
    print(f"    F: [{min(hist['F']):.4f}, {max(hist['F']):.4f}]  "
          f"({'varia => sem bug de chave fixa' if max(hist['F'])-min(hist['F'])>1e-3 else 'CONSTANTE!'})")

    # piso de ruido no objetivo pertinente ao modo
    final = hist["states"][-1]
    key_series = key
    if args.mode == "sharp":
        vals = []
        for _ in range(20):
            key_series, k = random.split(key_series)
            rho_n = density_matrix(final.Lambda, final.B, final.W1, final.b1,
                                   final.W2, final.b2, k, args.ensemble)
            vals.append(float(von_neumann_entropy(rho_n, args.eig_jitter)))
        nstd = float(jnp.std(jnp.array(vals)))
        delta = abs(hist["S"][-1] - hist["S"][0])
        name = "S"
    else:
        vals = []
        for _ in range(20):
            key_series, k = random.split(key_series)
            rho_n = density_matrix(final.Lambda, final.B, final.W1, final.b1,
                                   final.W2, final.b2, k, args.ensemble)
            vals.append(float(effective_rank(rho_n)))
        nstd = float(jnp.std(jnp.array(vals)))
        delta = abs(hist["eff_rank"][-1] - hist["eff_rank"][0])
        name = "eff_rank"
    print(f"\n[2] Piso de ruido ({name}): std = {nstd:.4f}")
    print(f"    => delta {name} {'DOMINA' if delta > 3*nstd else 'e comparavel a'}"
          f" o ruido (3*std = {3*nstd:.4f})")

    if args.nostr:
        key, k = random.split(key)
        c = prepare_nostr_event(final, k, args.ensemble, mode=args.mode)["content"]
        print("\n[3] Evento Nostr (kind 39000) [montado localmente, NAO publicado]:")
        for kk in ("s_measure", "purity", "effective_rank", "visibility"):
            print(f"    {kk:14s}= {c[kk]:.6f}")

    if not args.no_plot:
        out_png = os.path.join(args.output,
                               f"arkhe_rsi_v3_3_{args.mode}_{args.optimizer}.png")
        if args.interactive:
            print("\n[4] Visualizacao interativa...")
            interactive_plot(hist, out_png, args.mode)
        else:
            print("\n[4] Figura estatica...")
            plot_static(hist, out_png, args.mode)

    sp = os.path.join(args.output,
                      f"arkhe_rsi_v3_3_{args.mode}_{args.optimizer}_state.npz")
    np.savez(sp, B=np.asarray(final.B), W1=np.asarray(final.W1),
             b1=np.asarray(final.b1), W2=np.asarray(final.W2),
             b2=np.asarray(final.b2), Lambda=np.asarray(final.Lambda),
             I=np.asarray(final.I), J=np.asarray(final.J), dim=final.dim)
    print(f"\n[5] Estado salvo em {sp}")
    print("\n" + "=" * 70)
    print("ARKHE-RSI-v3.3 — COMPLETO")
    print("=" * 70)


if __name__ == "__main__":
    main()
