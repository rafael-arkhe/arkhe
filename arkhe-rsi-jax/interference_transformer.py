"""
interference_transformer.py — Does the RSI "membrane" help a transformer?
============================================================================
Honest, param-matched ablation of the ONE genuinely valid idea in the
"transformers/tensors/LLM" essay: the second-order interference term

    M(a, b) = R(B a + B b) - R(B a) - R(B b)     [R = 2-layer tanh MLP]

which measures the NON-ADDITIVITY of R (how much the joint input differs from
the sum of parts). This is a real interaction feature -- but it is NOT
attention (it does no token mixing), so here it sits ON TOP of a standard
self-attention sublayer, in place of the usual first-order feed-forward.

We compare, with IDENTICAL parameter sets and identical initialization:

  baseline     : z = LN(x + Attn(x));  g = R(B z);                       out = LN(z + g)
  interference : z = LN(x + Attn(x));  g = R(Bz + Bx) - R(Bz) - R(Bx);   out = LN(z + g)

Only the functional form of g differs (interference costs 3 R-evals vs 1).
Same params B, W1, b1, W2, b2 in both.

Two synthetic tasks over sequences with exactly two "marked" tokens p, q whose
scalar features a_p, a_q ~ N(0,1):
  * MUL  (second-order): y = a_p * a_q     -> should favor interference
  * ADD  (first-order) : y = a_p + a_q     -> control; interference should NOT help

If interference wins on MUL but ties on ADD, the advantage is specifically the
non-additivity, not just "more compute". If it wins on both or neither, we say so.

Run:  python interference_transformer.py
============================================================================
"""

import argparse
from functools import partial

import jax
import jax.numpy as jnp
from jax import random, jit, value_and_grad, vmap
import numpy as np

# ---------------------------------------------------------------------------
# hyperparams
# ---------------------------------------------------------------------------
D_MODEL = 32
SEQ_LEN = 10
N_MARK = 2
STEPS = 1500
BATCH = 256
EVAL_N = 4096
LR = 2e-3
SEEDS = (0, 1, 2, 3, 4)

# ---------------------------------------------------------------------------
# data: sequences with two marked tokens; targets add / mul of their features
# ---------------------------------------------------------------------------


def make_batch(key, n, task, seq_len=SEQ_LEN, n_mark=N_MARK):
    """Returns feats (n, L, 2) = [a_i, marker_i], and target y (n,).

    Exactly n_mark positions are marked (marker=1); their a-values determine y.
    """
    k_a, k_m = random.split(key)
    a = random.normal(k_a, (n, seq_len))                     # scalar feature
    # choose n_mark distinct marked positions per row
    marks = jnp.argsort(random.uniform(k_m, (n, seq_len)), axis=1)[:, :n_mark]
    marker = jax.nn.one_hot(marks, seq_len).sum(axis=1)      # (n, L) 0/1
    a_marked = jnp.take_along_axis(a, marks, axis=1)         # (n, n_mark)
    if task == "mul":
        y = jnp.prod(a_marked, axis=1)
    else:  # add
        y = jnp.sum(a_marked, axis=1)
    feats = jnp.stack([a, marker], axis=-1)                  # (n, L, 2)
    return feats, y


def pos_encoding(seq_len, d):
    pos = np.arange(seq_len)[:, None]
    i = np.arange(d)[None, :]
    ang = pos / np.power(10000.0, (2 * (i // 2)) / d)
    pe = np.where(i % 2 == 0, np.sin(ang), np.cos(ang))
    return jnp.asarray(pe, dtype=jnp.float32)                # (L, d)


# ---------------------------------------------------------------------------
# params (identical shape set for both variants)
# ---------------------------------------------------------------------------


def init_params(key, d=D_MODEL):
    ks = random.split(key, 12)
    g = lambda k, shp, s: random.normal(k, shp) * s
    return {
        "Win": g(ks[0], (2, d), 0.5),
        "Wq": g(ks[1], (d, d), 1 / jnp.sqrt(d)),
        "Wk": g(ks[2], (d, d), 1 / jnp.sqrt(d)),
        "Wv": g(ks[3], (d, d), 1 / jnp.sqrt(d)),
        "Wo": g(ks[4], (d, d), 1 / jnp.sqrt(d)),
        "B":  g(ks[5], (d, d), 1 / jnp.sqrt(d)),
        "W1": g(ks[6], (d, d), 1 / jnp.sqrt(d)),
        "b1": jnp.zeros(d),
        "W2": g(ks[7], (d, d), 1 / jnp.sqrt(d)),
        "b2": jnp.zeros(d),
        "g1": jnp.ones(d), "be1": jnp.zeros(d),   # LayerNorm 1
        "g2": jnp.ones(d), "be2": jnp.zeros(d),   # LayerNorm 2
        "w_out": g(ks[8], (d,), 1 / jnp.sqrt(d)), "b_out": jnp.zeros(()),
    }

# ---------------------------------------------------------------------------
# model (single example: x is (L, d)); vmap over batch
# ---------------------------------------------------------------------------


def layernorm(x, g, be, eps=1e-5):
    mu = x.mean(-1, keepdims=True)
    var = x.var(-1, keepdims=True)
    return g * (x - mu) / jnp.sqrt(var + eps) + be


def R(x, p):
    h = jnp.tanh(x @ p["W1"] + p["b1"])
    return jnp.tanh(h @ p["W2"] + p["b2"])


def attention(x, p):
    d = x.shape[-1]
    Q, K, V = x @ p["Wq"], x @ p["Wk"], x @ p["Wv"]
    scores = (Q @ K.T) / jnp.sqrt(d)                # (L, L)
    A = jax.nn.softmax(scores, axis=-1)
    return (A @ V) @ p["Wo"]                         # (L, d)


def forward_single(p, feats, interference, pe):
    x = feats @ p["Win"] + pe                         # (L, d)
    z = layernorm(x + attention(x, p), p["g1"], p["be1"])
    Bz, Bx = z @ p["B"], x @ p["B"]
    if interference:
        g = R(Bz + Bx, p) - R(Bz, p) - R(Bx, p)     # second-order
    else:
        g = R(Bz, p)                                 # first-order (same params)
    out = layernorm(z + g, p["g2"], p["be2"])
    pooled = out.mean(axis=0)                        # (d,)
    return pooled @ p["w_out"] + p["b_out"]          # scalar


def predict(p, feats, interference, pe):
    return vmap(lambda f: forward_single(p, f, interference, pe))(feats)


def mse_loss(p, feats, y, interference, pe):
    pred = predict(p, feats, interference, pe)
    return jnp.mean((pred - y) ** 2)


_ATTN_KEYS = ("Wq", "Wk", "Wv", "Wo")

# ---------------------------------------------------------------------------
# Adam
# ---------------------------------------------------------------------------


def adam_init(p):
    z = jax.tree_util.tree_map(jnp.zeros_like, p)
    return z, jax.tree_util.tree_map(jnp.zeros_like, p), jnp.asarray(0, jnp.int32)


@partial(jit, static_argnames=("interference", "freeze_attn"))
def train_step(p, m, v, t, feats, y, lr, pe, interference, freeze_attn):
    loss, grads = value_and_grad(mse_loss)(p, feats, y, interference, pe)
    if freeze_attn:                                  # attention held at init
        for k in _ATTN_KEYS:
            grads[k] = jnp.zeros_like(grads[k])
    t = t + 1
    b1, b2, eps = 0.9, 0.999, 1e-8
    m = jax.tree_util.tree_map(lambda mm, g: b1 * mm + (1 - b1) * g, m, grads)
    v = jax.tree_util.tree_map(lambda vv, g: b2 * vv + (1 - b2) * g * g, v, grads)
    mc = 1 - b1 ** t
    vc = 1 - b2 ** t
    p = jax.tree_util.tree_map(
        lambda pp, mm, vv: pp - lr * (mm / mc) / (jnp.sqrt(vv / vc) + eps),
        p, m, v)
    return p, m, v, t, loss

# ---------------------------------------------------------------------------
# train one model on one task
# ---------------------------------------------------------------------------


def run(seed, task, interference, steps=STEPS, dim=D_MODEL, seq_len=SEQ_LEN,
        n_mark=N_MARK, freeze_attn=False):
    key = random.PRNGKey(seed)
    key, ki = random.split(key)
    p = init_params(ki, dim)                          # SAME init for both variants
    pe = pos_encoding(seq_len, dim)
    m, v, t = adam_init(p)
    for _ in range(steps):
        key, kb = random.split(key)
        feats, y = make_batch(kb, BATCH, task, seq_len, n_mark)
        p, m, v, t, _ = train_step(p, m, v, t, feats, y, LR, pe,
                                   interference, freeze_attn)
    key, ke = random.split(key)
    fe, ye = make_batch(ke, EVAL_N, task, seq_len, n_mark)
    mse = float(mse_loss(p, fe, ye, interference, pe))
    r2 = 1.0 - mse / float(jnp.var(ye))
    return mse, r2

# ---------------------------------------------------------------------------
# main
# ---------------------------------------------------------------------------


def _regime(label, task, seeds, steps, dim, seq_len, n_mark, freeze_attn):
    print(f"--- {label} ---")
    n_params = sum(int(np.prod(x.shape)) for x in
                   jax.tree_util.tree_leaves(init_params(random.PRNGKey(0), dim)))
    print(f"    (d={dim}, markers={n_mark}, freeze_attn={freeze_attn}, "
          f"params={n_params})")
    rows = {}
    for interference in (False, True):
        mses, r2s = [], []
        for s in seeds:
            mse, r2 = run(s, task, interference, steps, dim, seq_len,
                          n_mark, freeze_attn)
            mses.append(mse); r2s.append(r2)
        name = "interference" if interference else "baseline    "
        ma, ms = np.mean(mses), np.std(mses)
        ra, rs = np.mean(r2s), np.std(r2s)
        rows[name.strip()] = (ma, ms)
        print(f"    {name}: test MSE = {ma:.4f} +/- {ms:.4f}   "
              f"R^2 = {ra:.3f} +/- {rs:.3f}")
    b, i = rows["baseline"], rows["interference"]
    rel = 100.0 * (b[0] - i[0]) / (b[0] + 1e-12)
    verdict = ("interference BETTER" if i[0] < b[0] - (b[1] + i[1]) / 2
               else "baseline BETTER" if b[0] < i[0] - (b[1] + i[1]) / 2
               else "TIE (within noise)")
    print(f"    => MSE change (interf vs base): {rel:+.1f}%   [{verdict}]\n")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--steps", type=int, default=STEPS)
    ap.add_argument("--seeds", type=int, nargs="+", default=[0, 1, 2, 3])
    args = ap.parse_args()
    S = args.seeds

    print("=" * 72)
    print("STRESS-TEST: does the interference term EVER separate from baseline?")
    print(f"param-matched, same init per seed | steps={args.steps} seeds={S}")
    print("=" * 72 + "\n")

    # Reference: full trained attention, 2-way product (from the base run)
    _regime("R0  MUL d=32, attention TRAINED (reference)",
            "mul", S, args.steps, 32, SEQ_LEN, 2, False)
    # (a) attention frozen at init -> interference is the only learnable combiner
    _regime("Ra  MUL d=32, attention FROZEN at init",
            "mul", S, args.steps, 32, SEQ_LEN, 2, True)
    # (b) higher-order: 3-way product
    _regime("Rb  MUL3 y=a_p*a_q*a_r, d=32, 3 markers, attention trained",
            "mul", S, args.steps, 32, SEQ_LEN, 3, False)
    # (c) tight capacity
    _regime("Rc  MUL d=8 (tight capacity), attention trained",
            "mul", S, args.steps, 8, SEQ_LEN, 2, False)

    print("=" * 72)
    print("Read: 'interference BETTER' on any regime = a real, specific win.")
    print("All TIE/baseline = the second-order term adds no measurable capability.")
    print("=" * 72)


if __name__ == "__main__":
    main()
