#!/usr/bin/env python3
"""
estimator_s2_brydges.py
Estimador de S2 (Rényi-2) via randomized Pauli measurements.
Correção do roteiro v2.0 — P1 (Brydges et al. 2019, Elben et al. 2018)
"""

import numpy as np
from collections import defaultdict
from itertools import combinations

def estimate_purity_randomized(outcomes, bases, k):
    """
    Estima Tr(rho_A^2) a partir de shadows.

    Parâmetros:
        outcomes: lista de inteiros (bitstrings) para cada medição
        bases: lista de tuplas de inteiros 0..2 representando X,Y,Z por qubit
        k: número de qubits em A

    Retorna:
        purity, n_pairs_usados
    """
    N = len(outcomes)
    if N < 2:
        raise ValueError("Número insuficiente de shadows (N < 2)")

    # Agrupa por base de Pauli
    groups = defaultdict(list)
    for idx, (b, out) in enumerate(zip(bases, outcomes)):
        groups[tuple(b)].append((idx, out))

    purity_sum = 0.0
    total_pairs = 0

    for base, items in groups.items():
        n_p = len(items)
        if n_p < 2:
            continue

        # Conta pares com outcomes iguais dentro da mesma base
        match_count = 0
        for (_, out_i), (_, out_j) in combinations(items, 2):
            if out_i == out_j:
                match_count += 1

        # Contribuição da base
        if match_count > 0:
            n_pairs_base = n_p * (n_p - 1) // 2
            purity_sum += (2**k) * (match_count / n_pairs_base)
            total_pairs += 1

    if total_pairs == 0:
        raise ValueError(
            f"Nenhum par com mesma base encontrado. "
            f"N_shadows={N}, grupos={len(groups)}. Aumente N."
        )

    purity = purity_sum / total_pairs
    return purity, total_pairs

def estimate_s2(outcomes, bases, k):
    """S2 = -log2(purity) com clipping para evitar log(<=0)."""
    purity, _ = estimate_purity_randomized(outcomes, bases, k)
    purity = np.clip(purity, 1e-12, 1.0)
    return -np.log2(purity)

def bootstrap_s2(outcomes, bases, k, n_bootstrap=200, seed=42):
    """Bootstrap para estimar erro padrão de S2."""
    rng = np.random.default_rng(seed)
    N = len(outcomes)
    s2_samples = []
    for _ in range(n_bootstrap):
        idx = rng.integers(0, N, size=N)
        try:
            s2 = estimate_s2([outcomes[i] for i in idx],
                             [bases[i] for i in idx], k)
            s2_samples.append(s2)
        except ValueError:
            continue
    if len(s2_samples) < 10:
        raise ValueError("Bootstrap falhou: poucas amostras válidas.")
    return np.mean(s2_samples), np.std(s2_samples)

# ===== Exemplo de uso =====
if __name__ == "__main__":
    # Simulação para k=5, estado maximamente misto (purity = 1/2^k)
    k = 5
    N = 20000
    rng = np.random.default_rng(42)

    bases = [tuple(rng.integers(0, 3, size=k)) for _ in range(N)]
    outcomes = [rng.integers(0, 2**k) for _ in range(N)]

    s2 = estimate_s2(outcomes, bases, k)
    s2_mean, s2_std = bootstrap_s2(outcomes, bases, k)

    print(f"k={k}, N={N}")
    print(f"S2 estimado = {s2:.4f} (teórico para maximamente misto: {k:.4f})")
    print(f"S2 (bootstrap) = {s2_mean:.4f} ± {s2_std:.4f}")
