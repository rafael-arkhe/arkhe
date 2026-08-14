#!/usr/bin/env python3
"""
emulator_s2_validation.py
Validacao do espectro S2 do codigo [[5,1,3]] em emulador ruidoso.

Correcao v2 (2026-08-11):
  - O [[5,1,3]] NAO e um estado de grafos C5. Estabilizadores locais do
    grafo C5 (K_i = X_i Z_{i-1} Z_{i+1}) nao coincidem com os estabilizadores
    nao-locais do codigo (<XZZXI, IXZZX, XIXZZ, ZXIXZ>).
  - |0_L> e preparado por PROJECAO sobre o subespaco +1 dos estabilizadores
    (Numpy puro), verificada: <g_i> = +1, <Z_L> = +1, <X_L> = 0.
  - Shadows coletados em Numpy puro (sem Qiskit): base de Pauli aleatoria por
    qubit + amostragem da distribuicao de outcomes do estado reduzido rho_A.
  - Purity via formula NAO-enviesada prod_l (9*T_l - 1) (Brydges/Elben),
    calculada em O(N*2^k) exato (validado bit-a-bit contra o estimador
    O(N^2) de estimator_s2_brydges_elben.py).
  - Erro via bootstrap sobre shadows.

Correcao v2.1 (bit-ordering, 2026-08-11):
  - A coleta de shadows construia U com basis[0] como fator MAIS significativo
    do kron, mas extraia o outcome bit-l = qubit-l (little-endian). Inversao
    que so afetava estados NAO simetricos sob permutacao de qubits:
      |00>+|11> misto c/ peso: 0.85 vs exato 0.5 (k=2);
      GHZ4 reduzido:           0.84 vs exato 0.5 (k=3).
  - Correcao: ordem little-endian (qubit 0 = ultimo fator do kron).
    Validado apos fix: 0.503 vs 0.5, 0.499 vs 0.5, espectro |0_L> 4/4 OK.

Espectro exato de |0_L> (estado puro do codigo perfeito [[5,1,3]]):
  k=1: S2 = 1.0   (purity 0.5,  rho_A max-misto)
  k=2: S2 = 2.0   (purity 0.25, rho_A max-misto)
  k=3: S2 = 2.0   (purity 0.25, rank 4)
  k=4: S2 = 1.0   (purity 0.5,  complemento de 1 qubit)

Uso:
  python emulator_s2_validation.py [--n4 25600] [--p-depol 0.0]
        [--n-boot 40] [--seed 0]
"""

import argparse
import json
import sys
import time
from itertools import combinations

import numpy as np

# =============================================================================
# 1. CODIGO [[5,1,3]]
# =============================================================================
# Geradores (0=I, 1=X, 2=Y, 3=Z)
STABILIZERS = [
    [1, 3, 3, 1, 0],  # X Z Z X I
    [0, 1, 3, 3, 1],  # I X Z Z X
    [1, 0, 1, 3, 3],  # X I X Z Z
    [3, 1, 0, 1, 3],  # Z X I X Z
]
Z_L = [3, 3, 3, 3, 3]
X_L = [1, 1, 1, 1, 1]
N_QUBITS = 5
DIM = 2 ** N_QUBITS

_PAULI2 = {
    0: np.eye(2, dtype=complex),
    1: np.array([[0.0, 1.0], [1.0, 0.0]], dtype=complex),
    2: np.array([[0.0, -1.0j], [1.0j, 0.0]], dtype=complex),
    3: np.array([[1.0, 0.0], [0.0, -1.0]], dtype=complex),
}


def pauli_string_matrix(paulis):
    """Produto tensorial de Paulis -> matriz 32x32 (e 1x1 para lista vazia)."""
    m = np.array([1.0 + 0.0j])
    for p in paulis:
        m = np.kron(m, _PAULI2[p])
    return m


# =============================================================================
# 2. PREPARACAO DE |0_L> POR PROJECAO
# =============================================================================
def logical_zero_state():
    """|0_L> = normalizacao de prod_i (I+g_i) |00000>, com <g_i>=+1, <Z_L>=+1."""
    proj = np.eye(DIM, dtype=complex) / (2 ** 4)
    for stab in STABILIZERS:
        proj = proj @ (np.eye(DIM, dtype=complex) + pauli_string_matrix(stab))

    psi = proj @ np.eye(DIM)[:, 0]
    norm = np.linalg.norm(psi)
    if norm < 1e-12:
        raise RuntimeError("Falha na projecao: |0_L> nulo")
    return psi / norm


def verify_state(psi):
    """Verifica estabilizadores e logicos de |0_L>."""
    checks = {}
    for i, s in enumerate(STABILIZERS, start=1):
        exp = float(np.real(psi.conj() @ pauli_string_matrix(s) @ psi))
        checks[f'g{i}'] = (exp, 1.0)
    checks['Z_L'] = (float(np.real(psi.conj() @ pauli_string_matrix(Z_L) @ psi)), 1.0)
    checks['X_L'] = (float(np.real(psi.conj() @ pauli_string_matrix(X_L) @ psi)), 0.0)
    return checks


# =============================================================================
# 3. RUIDO DEPOLARIZANTE (KRAUS, por qubit)
# =============================================================================
def apply_depolarizing(rho, p_depol):
    """Canal depolarizante por qubit: rho -> (1-p)rho + (p/3) sum_a Pa rho Pa.

    Implementado como soma sobre 4^5 Paulis tensoriais (1024 termos) com
    pesos independentes por qubit. p_depol = 1 - fidelity_por_qubit.
    """
    if p_depol <= 0.0:
        return rho
    paulis = [_PAULI2[0], _PAULI2[1], _PAULI2[2], _PAULI2[3]]
    w = np.array([1.0 - p_depol, p_depol / 3, p_depol / 3, p_depol / 3])

    rho_out = np.zeros_like(rho)
    for combo in np.ndindex(*(4,) * N_QUBITS):
        weight = 1.0
        for c in combo:
            weight *= w[c]
        K = paulis[combo[0]]
        for c in combo[1:]:
            K = np.kron(K, paulis[c])
        rho_out += weight * (K @ rho @ K.conj().T)
    return rho_out


def reduced_density(rho, A):
    """Traco parcial sobre A^c. A: lista de indices dos qubits mantidos."""
    k = len(A)
    notA = [q for q in range(N_QUBITS) if q not in A]
    dim_a = 2 ** k
    dim_na = 2 ** (N_QUBITS - k)
    order = A + notA

    rho_r = np.zeros_like(rho)
    for i in range(DIM):
        bits = [(i >> q) & 1 for q in range(N_QUBITS)]
        ni = sum(bits[order[pos]] << pos for pos in range(N_QUBITS))
        for j in range(DIM):
            bitsj = [(j >> q) & 1 for q in range(N_QUBITS)]
            nj = sum(bitsj[order[pos]] << pos for pos in range(N_QUBITS))
            rho_r[ni, nj] = rho[i, j]

    rho_r = rho_r.reshape(dim_a, dim_na, dim_a, dim_na)
    return np.trace(rho_r, axis1=1, axis2=3)


# =============================================================================
# 4. COLETA DE SHADOWS (NUMPY PURO)
# =============================================================================
def collect_shadows_numpy(rho_A, k, N_shadows, rng):
    """Shadows de rho_A sem Qiskit.

    Para cada shadow: base de Pauli aleatoria (X/Y/Z) por qubit + um outcome
    amostrado de p(m) = diag(U_b^dag rho_A U_b)_m, onde U_b e a base de
    autovetores dos operadores de Pauli escolhidos.

    Returns: lista de (basis_tuple, outcome_int).
    """
    # Autovetores de cada base de Pauli para um qubit (colunas = outcomes 0/1)
    EV = {
        0: np.array([[1, 1], [1, -1]], dtype=complex) / np.sqrt(2),   # X
        1: np.array([[1, 1], [1j, -1j]], dtype=complex) / np.sqrt(2), # Y
        2: np.array([[1, 0], [0, 1]], dtype=complex),                 # Z
    }
    shadows = []
    for _ in range(N_shadows):
        basis = tuple(rng.integers(0, 3, size=k).tolist())
        # Ordem little-endian: qubit 0 (bit 0) e o ULTIMO fator do kron.
        U = EV[basis[-1]]
        for b in basis[-2::-1]:
            U = np.kron(U, EV[b])
        diag = np.real(np.diag(U.conj().T @ rho_A @ U))
        diag = np.maximum(diag, 0.0)
        s = diag.sum()
        if s > 0:
            diag = diag / s
        outcome = int(rng.choice(2 ** k, p=diag))
        shadows.append((basis, outcome))
    return shadows


# =============================================================================
# 5. ESTIMADOR DE PURITY (O(N*2^k) EXATO)
# =============================================================================
def purity_from_shadows_fast(shadows, k):
    """Tr(rho_A^2) = mean_{i<j} prod_l (9*T_l - 1), em O(N*2^k).

    Expansao: prod_l (9T_l-1) = sum_{S subset [k]} 9^|S| (-1)^(k-|S|) prod_{l in S} T_l.
    prod_{l in S} T_l e 1 iff os dois shadows concordam em base E outcome nas
    colunas de S. Soma de pares via agrupamento (np.unique) -> O(N*2^k).
    Validado bit-a-bit contra _purity_from_shadows (O(N^2)).
    """
    N = len(shadows)
    if N < 2:
        return float('nan')
    B = np.array([b for b, _ in shadows], dtype=np.int8)   # (N, k)
    O = np.array([o for _, o in shadows], dtype=np.int64)  # (N,)
    obits = np.array([(O >> l) & 1 for l in range(k)]).T  # (N, k)
    sig = np.stack([B, obits], axis=-1)  # (N, k, 2)
    n_pairs = N * (N - 1) // 2

    total = 0.0
    for S in range(1 << k):
        cols = [l for l in range(k) if (S >> l) & 1]
        w = (9.0 ** len(cols)) * ((-1.0) ** (k - len(cols)))
        if not cols:
            total += w * n_pairs
            continue
        key = sig[:, cols, :].reshape(N, -1)
        _, cnts = np.unique(key, axis=0, return_counts=True)
        total += w * float((cnts * (cnts - 1) // 2).sum())
    return total / n_pairs


def bootstrap_std_fast(shadows, k, n_bootstrap=40, seed=1):
    """Erro padrao via bootstrap sobre shadows (correlacao entre pares)."""
    rng = np.random.default_rng(seed)
    N = len(shadows)
    B = np.array([b for b, _ in shadows], dtype=np.int8)
    O = np.array([o for _, o in shadows], dtype=np.int64)
    obits = np.array([(O >> l) & 1 for l in range(k)]).T
    sig = np.stack([B, obits], axis=-1)
    n_pairs = N * (N - 1) // 2

    purities = np.empty(n_bootstrap)
    for t in range(n_bootstrap):
        idx = rng.integers(0, N, size=N)
        sigb = sig[idx]
        total = 0.0
        for S in range(1 << k):
            cols = [l for l in range(k) if (S >> l) & 1]
            w = (9.0 ** len(cols)) * ((-1.0) ** (k - len(cols)))
            if not cols:
                total += w * n_pairs
                continue
            key = sigb[:, cols, :].reshape(N, -1)
            _, cnts = np.unique(key, axis=0, return_counts=True)
            total += w * float((cnts * (cnts - 1) // 2).sum())
        purities[t] = total / n_pairs
    return float(np.std(purities))


def estimate_S2(rho_A, k, N_shadows, rng, n_boot=40):
    """Purity + S2 + erro a partir de shadows de rho_A."""
    shadows = collect_shadows_numpy(rho_A, k, N_shadows, rng)
    purity = purity_from_shadows_fast(shadows, k)
    purity = float(np.clip(purity, 1.0 / (2 ** k), 1.0))
    purity_std = bootstrap_std_fast(shadows, k, n_boot)
    S2 = -np.log2(purity)
    S2_std = purity_std / (purity * np.log(2)) if purity > 0 else float('nan')
    return purity, purity_std, S2, S2_std, shadows


# =============================================================================
# 6. MAIN
# =============================================================================
def main(argv=None):
    ap = argparse.ArgumentParser(
        description="Espectro S2 do [[5,1,3]] em emulador (shadows Numpy).")
    ap.add_argument('--n2', type=int, default=3200,
                    help='shadows p/ k=2 (default 3200)')
    ap.add_argument('--n3', type=int, default=9600,
                    help='shadows p/ k=3 (default 9600)')
    ap.add_argument('--n4', type=int, default=25600,
                    help='shadows p/ k=4 (default 25600)')
    ap.add_argument('--p-depol', type=float, default=0.0,
                    help='erro depolarizante por qubit (0.0 = estado puro)')
    ap.add_argument('--n-boot', type=int, default=40)
    ap.add_argument('--seed', type=int, default=0)
    ap.add_argument('--verify', action='store_true',
                    help='imprime verificacao de estabilizadores')
    ap.add_argument('--json', default=None,
                    help='caminho p/ relatorio JSON maquina-legivel')
    args = ap.parse_args(argv)

    print("=" * 66)
    print("EMULADOR S2 — CODIGO [[5,1,3]] (shadows Numpy, sem Qiskit)")
    print("=" * 66)

    # --- Preparacao e verificacao do estado ---
    psi = logical_zero_state()
    if args.verify:
        print("\n--- Verificacao de |0_L> ---")
        for label, (val, exp) in verify_state(psi).items():
            ok = abs(val - exp) < 1e-8
            print(f"  {label:4s}: <psi|{label}|psi> = {val:+.6f} "
                  f"(esperado {exp:+.1f}) [{'PASS' if ok else 'FAIL'}]")

    # --- Ruido ---
    t0 = time.time()
    rho = np.outer(psi, psi.conj())
    if args.p_depol > 0:
        rho = apply_depolarizing(rho, args.p_depol)
    print(f"\nRuido depolarizante por qubit: p={args.p_depol:.4f} "
          f"(fidelity 1q = {1 - args.p_depol:.4f}) | preparo+ruido: "
          f"{time.time() - t0:.2f}s")

    rng = np.random.default_rng(args.seed)

    print("\n--- Espectro S2 ---")
    print(f"{'k':>3} {'subset':>8} {'N':>7} {'purity_est':>12} "
          f"{'purity_exato':>13} {'S2_est':>8} {'S2_exato':>8} "
          f"{'S2_std':>8} {'status':>8}")
    all_ok = True
    rows = []
    for k, N in ((1, 600), (2, args.n2), (3, args.n3), (4, args.n4)):
        # exato do estado puro (referencia teorica) E do estado ruidoso
        A = list(range(k))
        if k == 4:
            A = [0, 1, 2, 3]
        rho_A = reduced_density(rho, A)
        # referencia exata do estado (com ou sem ruido) via traco
        pur_exact = float(np.real(np.trace(rho_A @ rho_A)))
        # referencia teorica do codigo perfeito puro
        s2_theory = float(np.log2(2 ** min(k, N_QUBITS - k)))
        pur_theory = 2.0 ** (-min(k, N_QUBITS - k))

        pur, pur_std, S2, S2_std, _ = estimate_S2(rho_A, k, N, rng,
                                                  args.n_boot)
        # 1-sigma em torno do exato (ruidoso); puro: em torno do teorico
        ref_pur = pur_theory if args.p_depol == 0 else pur_exact
        within = abs(pur - ref_pur) <= 2 * pur_std
        all_ok = all_ok and within
        status = "OK" if within else "OFF"
        print(f"{k:>3} {str(A):>8} {N:>7} {pur:>12.4f} {pur_exact:>13.4f} "
              f"{S2:>8.3f} {s2_theory:>8.3f} {S2_std:>8.4f} {status:>8}")
        rows.append({
            'k': k,
            'subset': A,
            'n_shadows': N,
            'purity_est': pur,
            'purity_exact': pur_exact,
            'S2_est': S2,
            'S2_theory': s2_theory,
            'S2_std': S2_std,
            'status': status,
        })

    print("\n" + ("== Todos os estimadores dentro de 2-sigma =="
                  if all_ok else "== ATENCAO: algum estimador fora de 2-sigma =="))

    if args.json:
        report = {
            'p_depol': args.p_depol,
            'fidelity_1q': 1 - args.p_depol,
            'seed': args.seed,
            'all_ok': all_ok,
            'rows': rows,
        }
        with open(args.json, 'w', encoding='utf-8') as fh:
            json.dump(report, fh, indent=2)
        print(f"JSON salvo: {args.json}")

    return 0 if all_ok else 1


if __name__ == '__main__':
    sys.exit(main())
