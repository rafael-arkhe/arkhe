"""
ARKHE-RSI-v3.1 — Recursao de Interferencia com Adam, Modos e Visualizacao
============================================================================
Modos:
  sharp   : minimiza S(rho) -> estado puro   -> coerencia maxima (default)
  explore : maximiza S(rho) -> estado misto  -> exploracao do espaco de fases

Otimizadores:
  adam : Adam (beta1=0.9, beta2=0.999, eps=1e-8) + gradient clipping global
  sgd  : SGD com momentum (mu=0.9)             + gradient clipping global

Medicao honesta:
  A loss e o historico de metricas usam uma chave de ensemble FIXA, de modo que
  a evolucao reflete DINAMICA pura; o piso de ruido de amostragem e medido
  separadamente ao final (re-amostrando com o estado treinado).

Integracao Nostr:
  prepare_nostr_event(...) gera um evento kind:39000 com as metricas finais.

Uso:
  python arkhe_rsi_v3_1.py --mode sharp   --steps 100 --interactive
  python arkhe_rsi_v3_1.py --mode explore --steps 200 --no-plot --output ./results
  python arkhe_rsi_v3_1.py --mode sharp   --optimizer sgd --lr 0.3 --nostr --no-plot
============================================================================

Notas de implementacao (correcoes sobre o rascunho v3.1):
  - jax.tree_map / jax.tree_flatten removidos no JAX>=0.10 -> jax.tree_util.*
  - Adam opera sobre a TUPLA de parametros (grad via argnums), nunca sobre o
    RSIState inteiro (que incluiria I,J e quebraria a estrutura do pytree).
  - AdamState/SgdState sao NamedTuple => pytrees validos e jit-compativeis.
  - @dataclass removido (nao era importado); SGD de fato implementado.
  - Modo interativo troca para backend GUI se houver display; senao, degrada
    para figura estatica sem travar.
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
        # CHAVES INDEPENDENTES -> I != J
        self.I = self.Lambda + random.normal(k_I, (dim,)) * DEFAULT_PERT
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
        obj.B = kw.get("B", self.B)
        obj.W1 = kw.get("W1", self.W1)
        obj.b1 = kw.get("b1", self.b1)
        obj.W2 = kw.get("W2", self.W2)
        obj.b2 = kw.get("b2", self.b2)
        obj.Lambda = kw.get("Lambda", self.Lambda)
        obj.I = kw.get("I", self.I)
        obj.J = kw.get("J", self.J)
        obj.dim = kw.get("dim", self.dim)
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


def von_neumann_entropy(rho):
    ev = jnp.clip(jnp.linalg.eigvalsh(rho), 1e-12, 1.0)
    return -jnp.sum(ev * jnp.log(ev))


def purity(rho):
    return jnp.trace(rho @ rho)


def fidelity(I, J):
    return jnp.abs(jnp.dot(I, J)) ** 2 / (jnp.dot(I, I) * jnp.dot(J, J) + 1e-10)


def visibility(M):
    M2 = M ** 2
    hi, lo = jnp.max(M2), jnp.min(M2)
    return (hi - lo) / (hi + lo + 1e-10)


def eigenvalue_ratio(rho):
    ev = jnp.sort(jnp.linalg.eigvalsh(rho))[::-1]
    return ev[0] / (ev[1] + 1e-10)


def effective_rank(rho):
    """Rank de participacao 1/Tr(rho^2) = 1/P (robusto, sem threshold)."""
    return 1.0 / (purity(rho) + 1e-10)

# ==============================================================================
# LOSS SOBRE A TUPLA DE PARAMETROS (grad estruturalmente consistente)
# ==============================================================================
# params = (B, W1, b1, W2, b2, Lambda)


def _entropy_from_params(params, key, n_samples, pert):
    B, W1, b1, W2, b2, Lambda = params
    rho = density_matrix(Lambda, B, W1, b1, W2, b2, key, n_samples, pert)
    return von_neumann_entropy(rho)


def _loss(params, key, n_samples, pert, sign):
    # sign=+1 -> minimiza S (sharp); sign=-1 -> minimiza -S == maximiza S (explore)
    return sign * _entropy_from_params(params, key, n_samples, pert)

# ==============================================================================
# OTIMIZADORES (NamedTuple => pytree valido e jit-compativel)
# ==============================================================================


class AdamState(NamedTuple):
    m: Any
    v: Any
    t: Any


class SgdState(NamedTuple):
    v: Any  # velocidade (momentum)


def adam_init(params):
    z = jax.tree_util.tree_map(jnp.zeros_like, params)
    return AdamState(m=z, v=jax.tree_util.tree_map(jnp.zeros_like, params),
                     t=jnp.asarray(0, dtype=jnp.int32))


def sgd_init(params):
    return SgdState(v=jax.tree_util.tree_map(jnp.zeros_like, params))


@partial(jit, static_argnames=("beta1", "beta2", "eps"))
def adam_update(params, grads, st, lr, beta1=0.9, beta2=0.999, eps=1e-8):
    t = st.t + 1
    new_m = jax.tree_util.tree_map(lambda m, g: beta1 * m + (1 - beta1) * g,
                                   st.m, grads)
    new_v = jax.tree_util.tree_map(lambda v, g: beta2 * v + (1 - beta2) * g * g,
                                   st.v, grads)
    bc1 = 1 - beta1 ** t
    bc2 = 1 - beta2 ** t
    new_params = jax.tree_util.tree_map(
        lambda p, m, v: p - lr * (m / bc1) / (jnp.sqrt(v / bc2) + eps),
        params, new_m, new_v)
    return new_params, AdamState(m=new_m, v=new_v, t=t)


@partial(jit, static_argnames=("mu",))
def sgd_update(params, grads, st, lr, mu=0.9):
    new_v = jax.tree_util.tree_map(lambda v, g: mu * v - lr * g, st.v, grads)
    new_params = jax.tree_util.tree_map(lambda p, v: p + v, params, new_v)
    return new_params, SgdState(v=new_v)


def _global_norm(grads):
    leaves = jax.tree_util.tree_leaves(grads)
    return jnp.sqrt(sum(jnp.sum(g * g) for g in leaves))


def _clip(grads, clip_norm):
    gnorm = _global_norm(grads)
    coeff = jnp.minimum(1.0, clip_norm / (gnorm + 1e-10))
    return jax.tree_util.tree_map(lambda g: g * coeff, grads), gnorm

# ==============================================================================
# TREINO
# ==============================================================================


def train(state, key, steps=DEFAULT_STEPS, lr=DEFAULT_LR, mode=DEFAULT_MODE,
          optimizer=DEFAULT_OPTIMIZER, n_samples=DEFAULT_ENSEMBLE_N,
          pert=DEFAULT_PERT, clip_norm=DEFAULT_CLIP_NORM):
    """Treina minimizando (sharp) ou maximizando (explore) S(rho)."""
    sign = 1.0 if mode == "sharp" else -1.0
    params = (state.B, state.W1, state.b1, state.W2, state.b2, state.Lambda)

    if optimizer == "adam":
        opt_state, opt_update = adam_init(params), adam_update
    else:
        opt_state, opt_update = sgd_init(params), sgd_update

    hist = {k: [] for k in ("S", "P", "V", "F", "lambda_ratio",
                            "eff_rank", "loss", "global_norm", "states")}

    for _ in range(steps):
        key, sub = random.split(key)
        loss, grads = value_and_grad(_loss)(params, sub, n_samples, pert, sign)
        grads, gnorm = _clip(grads, clip_norm)
        params, opt_state = opt_update(params, grads, opt_state, lr)

        B, W1, b1, W2, b2, Lambda = params
        newI, newJ = sample_pair(Lambda, sub, pert)
        st = state.replace(B=B, W1=W1, b1=b1, W2=W2, b2=b2,
                           Lambda=Lambda, I=newI, J=newJ)

        # metricas com chave FIXA -> dinamica pura
        rho = density_matrix(Lambda, B, W1, b1, W2, b2,
                             FIXED_ENSEMBLE_KEY, n_samples, pert)
        M = membrane(newI, newJ, B, W1, b1, W2, b2)
        hist["S"].append(float(von_neumann_entropy(rho)))
        hist["P"].append(float(purity(rho)))
        hist["V"].append(float(visibility(M)))
        hist["F"].append(float(fidelity(newI, newJ)))
        hist["lambda_ratio"].append(float(eigenvalue_ratio(rho)))
        hist["eff_rank"].append(float(effective_rank(rho)))
        hist["loss"].append(float(loss))
        hist["global_norm"].append(float(gnorm))
        hist["states"].append(st)

    return hist

# ==============================================================================
# VISUALIZACAO
# ==============================================================================


def _spiral_xy(M):
    theta = jnp.linspace(0, 4 * jnp.pi, 500)
    r = jnp.linspace(0.1, 5, 500)
    amp = 1 + 0.3 * jnp.sin(theta * 3 + jnp.linalg.norm(M[:2]) * theta)
    return r * jnp.cos(theta) * amp, r * jnp.sin(theta) * amp


def _spiral_M(st, n_samples, pert):
    return membrane(st.I, st.J, st.B, st.W1, st.b1, st.W2, st.b2)


def plot_static(hist, out_path, n_samples=DEFAULT_ENSEMBLE_N, pert=DEFAULT_PERT):
    steps = len(hist["S"])
    fig, ax = plt.subplots(2, 2, figsize=(14, 12))
    x, y = _spiral_xy(_spiral_M(hist["states"][-1], n_samples, pert))
    ax[0, 0].plot(x, y, lw=0.8, color="#00d4ff")
    ax[0, 0].set_title("Arkhe Spiral (final)"); ax[0, 0].axis("off")
    ax[0, 0].set_aspect("equal")

    ax[0, 1].plot(hist["S"], "o-", color="#ff6b6b", ms=3)
    ax[0, 1].set_title("Entropia S(rho)"); ax[0, 1].grid(True, alpha=0.3)
    ax[0, 1].set_xlabel("passo"); ax[0, 1].set_ylabel("S(rho)")

    ax[1, 0].plot(hist["P"], "o-", color="#4ecdc4", ms=3, label="P")
    ax[1, 0].plot(hist["V"], "s-", color="#ffe66d", ms=3, label="V")
    ax[1, 0].set_title("Pureza P e Visibilidade V"); ax[1, 0].legend()
    ax[1, 0].grid(True, alpha=0.3); ax[1, 0].set_xlabel("passo")

    sc = ax[1, 1].scatter(hist["F"], hist["S"], c=range(steps),
                          cmap="viridis", s=40)
    ax[1, 1].set_title("Fidelidade vs Entropia")
    ax[1, 1].set_xlabel("Fidelidade |<I|J>|^2"); ax[1, 1].set_ylabel("S(rho)")
    ax[1, 1].grid(True, alpha=0.3); fig.colorbar(sc, ax=ax[1, 1], label="passo")

    plt.tight_layout()
    plt.savefig(out_path, dpi=150, bbox_inches="tight", facecolor="#0a0a0a")
    plt.close(fig)
    print(f"    figura salva em {out_path}")


def _try_gui_backend():
    """Tenta um backend interativo; retorna True se conseguiu."""
    for backend in ("QtAgg", "Qt5Agg", "TkAgg"):
        try:
            matplotlib.use(backend, force=True)
            import matplotlib.pyplot as _p  # re-bind
            globals()["plt"] = _p
            return True
        except Exception:
            continue
    return False


def interactive_plot(hist, out_path=None, n_samples=DEFAULT_ENSEMBLE_N,
                     pert=DEFAULT_PERT):
    steps = len(hist["S"])
    if steps == 0:
        print("Sem dados para visualizar."); return
    if not _try_gui_backend():
        print("    [!] Nenhum backend GUI disponivel (headless); "
              "gerando figura estatica em vez da interativa.")
        if out_path:
            plot_static(hist, out_path, n_samples, pert)
        return

    fig, axes = plt.subplots(2, 2, figsize=(14, 10))
    plt.subplots_adjust(left=0.08, bottom=0.25, right=0.95, top=0.93)
    ax_sp, ax_S, ax_P, ax_sc = axes[0, 0], axes[0, 1], axes[1, 0], axes[1, 1]

    ax_slider = plt.axes([0.15, 0.10, 0.65, 0.03])
    slider = Slider(ax_slider, "Passo", 0, steps - 1, valinit=steps - 1,
                    valfmt="%d", valstep=1)

    ax_S.plot(range(steps), hist["S"], "-", color="#ff6b6b", lw=1.5)
    ax_S.set_title("Entropia S(rho)"); ax_S.grid(True, alpha=0.3)
    ax_P.plot(range(steps), hist["P"], "-", color="#4ecdc4", lw=1.5, label="P")
    ax_P.plot(range(steps), hist["V"], "-", color="#ffe66d", lw=1.5, label="V")
    ax_P.set_title("Pureza / Visibilidade"); ax_P.legend(); ax_P.grid(True, alpha=0.3)
    sc = ax_sc.scatter(hist["F"], hist["S"], c=range(steps), cmap="viridis", s=40)
    ax_sc.set_title("Fidelidade vs Entropia"); ax_sc.grid(True, alpha=0.3)
    fig.colorbar(sc, ax=ax_sc, label="passo")

    vS = ax_S.axvline(steps - 1, color="gray", ls="--", alpha=0.5)
    vP = ax_P.axvline(steps - 1, color="gray", ls="--", alpha=0.5)
    dS, = ax_S.plot([steps - 1], [hist["S"][-1]], "ro", ms=8)
    dP, = ax_P.plot([steps - 1], [hist["P"][-1]], "ro", ms=8)

    x, y = _spiral_xy(_spiral_M(hist["states"][-1], n_samples, pert))
    sp_line, = ax_sp.plot(x, y, lw=0.8, color="#00d4ff")
    ax_sp.set_title(f"Arkhe Spiral — S={hist['S'][-1]:.4f}")
    ax_sp.set_aspect("equal"); ax_sp.axis("off")

    def update(_):
        i = int(slider.val)
        xx, yy = _spiral_xy(_spiral_M(hist["states"][i], n_samples, pert))
        sp_line.set_data(xx, yy)
        ax_sp.set_title(f"Arkhe Spiral — S={hist['S'][i]:.4f}")
        dS.set_data([i], [hist["S"][i]]); dP.set_data([i], [hist["P"][i]])
        vS.set_xdata([i]); vP.set_xdata([i])
        sizes = np.full(steps, 30); sizes[i] = 140
        sc.set_sizes(sizes)
        fig.canvas.draw_idle()

    slider.on_changed(update)
    ax_reset = plt.axes([0.85, 0.05, 0.08, 0.04])
    btn = Button(ax_reset, "Reset")
    btn.on_clicked(lambda _e: slider.set_val(steps - 1))
    if out_path:
        plt.savefig(out_path, dpi=150, bbox_inches="tight", facecolor="#0a0a0a")
        print(f"    figura salva em {out_path}")
    plt.show()

# ==============================================================================
# EVENTO NOSTR (kind 39000) — apenas montagem local; NAO publica nada
# ==============================================================================


def prepare_nostr_event(state, key, n_samples=DEFAULT_ENSEMBLE_N,
                        pert=DEFAULT_PERT, mode="trained"):
    rho = density_matrix(state.Lambda, state.B, state.W1, state.b1,
                         state.W2, state.b2, key, n_samples, pert)
    M = membrane(state.I, state.J, state.B, state.W1, state.b1, state.W2, state.b2)
    return {
        "kind": 39000,
        "content": {
            "s_measure": float(von_neumann_entropy(rho)),
            "purity": float(purity(rho)),
            "visibility": float(visibility(M)),
            "lambda_ratio": float(eigenvalue_ratio(rho)),
            "effective_rank": float(effective_rank(rho)),
            "timestamp": datetime.datetime.now(datetime.timezone.utc)
                         .isoformat().replace("+00:00", "Z"),
            "version": "RSI-v3.1",
            "dim": int(state.dim),
            "mode": mode,
        },
    }

# ==============================================================================
# MAIN
# ==============================================================================


def main():
    p = argparse.ArgumentParser(description="ARKHE-RSI-v3.1")
    p.add_argument("--mode", default=DEFAULT_MODE, choices=["sharp", "explore"])
    p.add_argument("--steps", type=int, default=DEFAULT_STEPS)
    p.add_argument("--lr", type=float, default=DEFAULT_LR)
    p.add_argument("--optimizer", default=DEFAULT_OPTIMIZER, choices=["adam", "sgd"])
    p.add_argument("--ensemble", type=int, default=DEFAULT_ENSEMBLE_N)
    p.add_argument("--dim", type=int, default=DEFAULT_DIM)
    p.add_argument("--seed", type=int, default=DEFAULT_SEED)
    p.add_argument("--output", default=DEFAULT_OUTPUT_DIR)
    p.add_argument("--interactive", action="store_true")
    p.add_argument("--no-plot", action="store_true")
    p.add_argument("--nostr", action="store_true")
    args = p.parse_args()

    os.makedirs(args.output, exist_ok=True)
    print("=" * 70)
    print(f"ARKHE-RSI-v3.1 — modo={args.mode}  opt={args.optimizer}  "
          f"dim={args.dim}  N={args.ensemble}")
    print("=" * 70)

    key = random.PRNGKey(args.seed)
    state = RSIState(args.dim, key)

    print(f"\n[1] Treinando ({args.mode}, {args.steps} passos, lr={args.lr})...")
    hist = train(state, key, steps=args.steps, lr=args.lr, mode=args.mode,
                 optimizer=args.optimizer, n_samples=args.ensemble)
    S0, Sf = hist["S"][0], hist["S"][-1]
    print(f"    S(rho): {S0:.4f} -> {Sf:.4f}   (delta {Sf - S0:+.4f})")
    print(f"    P:      {hist['P'][0]:.4f} -> {hist['P'][-1]:.4f}")
    print(f"    V:      {hist['V'][0]:.4f} -> {hist['V'][-1]:.4f}")
    print(f"    eff_rank: {hist['eff_rank'][0]:.2f} -> {hist['eff_rank'][-1]:.2f}")

    # piso de ruido nos parametros treinados
    final = hist["states"][-1]
    noise = []
    for _ in range(20):
        key, k = random.split(key)
        rho_n = density_matrix(final.Lambda, final.B, final.W1, final.b1,
                               final.W2, final.b2, k, args.ensemble)
        noise.append(float(von_neumann_entropy(rho_n)))
    nstd = float(jnp.std(jnp.array(noise)))
    print(f"\n[2] Piso de ruido (params treinados): std(S) = {nstd:.4f}")
    dom = abs(Sf - S0) > 3 * nstd
    print(f"    => delta S {'DOMINA' if dom else 'e comparavel a'} "
          f"o ruido (3*std = {3 * nstd:.4f})")

    if args.nostr:
        key, k = random.split(key)
        ev = prepare_nostr_event(final, k, args.ensemble, mode=args.mode)
        c = ev["content"]
        print("\n[3] Evento Nostr (kind 39000) [montado localmente, NAO publicado]:")
        for kk in ("s_measure", "purity", "visibility", "lambda_ratio",
                   "effective_rank"):
            print(f"    {kk:14s}= {c[kk]:.6f}")
        print(f"    timestamp     = {c['timestamp']}")

    if not args.no_plot:
        out_png = os.path.join(args.output, f"arkhe_rsi_v3_1_{args.mode}.png")
        if args.interactive:
            print("\n[4] Visualizacao interativa...")
            interactive_plot(hist, out_png, args.ensemble)
        else:
            print("\n[4] Figura estatica...")
            plot_static(hist, out_png, args.ensemble)

    state_path = os.path.join(args.output,
                              f"arkhe_rsi_v3_1_{args.mode}_state.npz")
    np.savez(state_path, B=np.asarray(final.B), W1=np.asarray(final.W1),
             b1=np.asarray(final.b1), W2=np.asarray(final.W2),
             b2=np.asarray(final.b2), Lambda=np.asarray(final.Lambda),
             I=np.asarray(final.I), J=np.asarray(final.J), dim=final.dim)
    print(f"\n[5] Estado final salvo em {state_path}")
    print("\n" + "=" * 70)
    print("ARKHE-RSI-v3.1 — COMPLETO")
    print("=" * 70)


if __name__ == "__main__":
    main()
