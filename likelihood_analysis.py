""" ============================================================================
    likelihood_analysis.py — BLOCO 501 v61 — Catedral OS
    Espelho Python (lado host) do likelihood_kernel.c (AURIX/TriCore):
      perfil de verossimilhança -> chi2 (Wilks, SPEC) -> p-value local ->
      correção look-elsewhere (Gross-Vitells) via toy MC determinístico.

    REVISÃO VETADA (vs. proposta v61):
      * Sem dependência de tpr_kernel/aqec_kernel/TrustGraphs (não existem no
        repositório — ground-truth verificado). A coerência entra como escalar.
      * Toy MC SORTEIA da hipótese nula (banda ER) — corrige o vazamento de
        sinal para os toys presente na proposta (que somava ruído ao observado).
      * PRNG determinístico (xorshift64*), não srand(time(NULL)).
      * Mistura normalizada mu + (1-mu) = 1 (a proposta somava +0.01 e não
        normalizava).
      * Wilks/Gross-Vitells: assintóticos -> SPEC (axiomas no contrato Lean).

    Hermético: stdlib apenas. Selftest default.
    ============================================================================
"""
from __future__ import annotations

import math
import random
import sys
from dataclasses import dataclass

ER_LOG10S2_MEAN0 = 3.80
ER_LOG10S2_SIGMA0 = 0.05
ER_S1_GAIN = 0.002
NR_LOG10S2_MEAN0 = 3.60
NR_LOG10S2_SIGMA0 = 0.08
NR_S1_GAIN = 0.0015

GRID_STEPS = 101
LEE_TOYS = 256


@dataclass(frozen=True)
class Event:
    S1c: float
    log10_S2c: float


@dataclass
class Result:
    mu_hat: float
    chi2: float
    p_value_local: float
    p_global: float
    significance_local: float
    significance_global: float
    is_signal: bool
    num_events: int


def pdf_nr(s1: float, log_s2: float, phi_c: float) -> float:
    mean = NR_LOG10S2_MEAN0 + NR_S1_GAIN * s1 + 0.05 * (phi_c - 0.85)
    sigma = NR_LOG10S2_SIGMA0 * (0.9 if phi_c > 0.9 else 1.0)
    z = (log_s2 - mean) / (sigma + 1e-12)
    return math.exp(-0.5 * z * z) / (sigma * math.sqrt(2.0 * math.pi))


def pdf_er(s1: float, log_s2: float, phi_c: float) -> float:
    mean = ER_LOG10S2_MEAN0 + ER_S1_GAIN * max(0.0, s1) - 0.1 * (1.0 - phi_c)
    sigma = ER_LOG10S2_SIGMA0 + 0.02 * (1.0 - phi_c)
    z = (log_s2 - mean) / (sigma + 1e-12)
    return math.exp(-0.5 * z * z) / (sigma * math.sqrt(2.0 * math.pi))


def loglik(events: list[Event], mu: float, phi_c: float) -> float:
    total = 0.0
    for ev in events:
        prob = mu * pdf_nr(ev.S1c, ev.log10_S2c, phi_c) + (1.0 - mu) * pdf_er(
            ev.S1c, ev.log10_S2c, phi_c
        )
        total += math.log(max(prob, 1e-300))
    return total


def profile(events: list[Event], phi_c: float) -> tuple[float, float]:
    best_ll, best_mu = -math.inf, 0.0
    for k in range(GRID_STEPS):
        mu = k / (GRID_STEPS - 1)
        ll = loglik(events, mu, phi_c)
        if ll > best_ll:
            best_ll, best_mu = ll, mu
    return best_mu, best_ll


def chi2_pvalue_1dof(ts: float) -> float:
    if ts < 0:
        return 1.0
    return 1.0 - math.erf(math.sqrt(ts / 2.0))


def norm_ppf(p: float) -> float:
    """Quantil da normal padrão (Abramowitz & Stegun 26.2.23), clamps HEP."""
    if p <= 0.0:
        return 8.0
    if p >= 1.0:
        return -8.0
    a = (2.50662823884, -18.61500062529, 41.39119773534, -25.44106049637)
    b = (-8.47351093090, 23.08336743743, -21.06224101826, 3.13082909833)
    c = (0.3374754822726147, 0.9761690190917186, 0.1607979714918209, 0.0276438810333863)
    d = (0.0032915721918257, 0.0002198789931803, 0.0000199943176875, 0.0000002752296141)
    q = p - 0.5
    if abs(q) < 0.42:
        r = q * q
        return q * (((a[3] * r + a[2]) * r + a[1]) * r + a[0]) / (
            (((b[3] * r + b[2]) * r + b[1]) * r + b[0]) * r + 1.0
        )
    rv = (1.0 - p) if p > 0.5 else p
    if rv < 1e-300:
        return 8.0 if p > 0.5 else -8.0
    rv = math.sqrt(-2.0 * math.log(rv))
    z = (((c[3] * rv + c[2]) * rv + c[1]) * rv + c[0]) / (
        (((d[3] * rv + d[2]) * rv + d[1]) * rv + d[0]) * rv + 1.0
    )
    return z if p > 0.5 else -z


def look_elsewhere_toy(events: list[Event], phi_c: float, ts_obs: float, seed: int) -> float:
    rng = random.Random(seed)
    n_extreme = 0
    for _ in range(LEE_TOYS):
        toy = []
        for ev in events:
            sigma = ER_LOG10S2_SIGMA0 + 0.02 * (1.0 - phi_c)
            mean = ER_LOG10S2_MEAN0 + ER_S1_GAIN * ev.S1c - 0.1 * (1.0 - phi_c)
            toy.append(Event(ev.S1c, mean + sigma * rng.gauss(0.0, 1.0)))
        _, ll_max = profile(toy, phi_c)
        ts_t = max(0.0, -2.0 * (loglik(toy, 0.0, phi_c) - ll_max))
        if ts_t >= ts_obs:
            n_extreme += 1
    return (n_extreme + 1.0) / (LEE_TOYS + 1.0)


def analyze(events: list[Event], phi_c: float, toy_seed: int = 0xCACA) -> Result:
    if not events:
        return Result(0.0, 0.0, 1.0, 1.0, 0.0, 0.0, False, 0)
    mu_hat, ll_max = profile(events, phi_c)
    ll_0 = loglik(events, 0.0, phi_c)
    chi2 = max(0.0, -2.0 * (ll_0 - ll_max))
    p_local = chi2_pvalue_1dof(chi2)
    z_local = norm_ppf(1.0 - p_local)
    p_global = look_elsewhere_toy(events, phi_c, chi2, toy_seed)
    z_global = norm_ppf(1.0 - p_global)
    if p_local > 0.5:
        z_local = 0.0
    if p_global > 0.5:
        z_global = 0.0
    return Result(mu_hat, chi2, p_local, p_global, z_local, z_global,
                  z_global > 2.5 and mu_hat > 0.1, len(events))


def _fill(with_signal: bool) -> list[Event]:
    s1 = [12, 15, 18, 21, 24, 27, 30, 34]
    phi_c = 0.85
    events = []
    for _ in range(5):
        for j in s1:
            events.append(Event(j, ER_LOG10S2_MEAN0 + ER_S1_GAIN * j - 0.1 * (1.0 - phi_c)))
    if with_signal:
        for j in s1:
            events.append(Event(j, NR_LOG10S2_MEAN0 + NR_S1_GAIN * j + 0.05 * (phi_c - 0.85)))
    return events


def _selftest() -> int:
    print("=== LIKELIHOOD SELFTEST (BLOCO 501 v61) ===")
    seed = 0xCACA

    r_bg = analyze(_fill(False), 0.85, toy_seed=seed)
    assert not r_bg.is_signal
    assert r_bg.chi2 == 0.0
    assert r_bg.p_global >= r_bg.p_value_local - 1e-12
    assert r_bg.significance_global == 0.0

    r_sig = analyze(_fill(True), 0.85, toy_seed=seed)
    assert 0.0 <= r_sig.mu_hat <= 1.0
    assert r_sig.p_value_local <= r_sig.p_global + 1e-12
    assert r_sig.p_global >= 1.0 / (LEE_TOYS + 1)
    assert r_sig.chi2 > r_bg.chi2

    r_sig2 = analyze(_fill(True), 0.85, toy_seed=seed)
    assert r_sig2 == r_sig or (
        r_sig2.mu_hat == r_sig.mu_hat and r_sig2.p_global == r_sig.p_global
    )

    print(
        f"[LIK] bg:  Z_g={r_bg.significance_global:.2f}σ ts=0.00  "
        f"(sacado da banda ER)"
    )
    print(
        f"[LIK] sig: mu_hat={r_sig.mu_hat:.3f} chi2={r_sig.chi2:.2f} "
        f"p_local={r_sig.p_value_local:.3e} p_global={r_sig.p_global:.3e} "
        f"Z_l={r_sig.significance_local:.2f}σ Z_g={r_sig.significance_global:.2f}σ "
        f"{'SINAL' if r_sig.is_signal else 'fundo'}"
    )
    print("=== LIKELIHOOD SELFTEST PASSED ===")
    return 0


if __name__ == "__main__":
    raise SystemExit(_selftest())