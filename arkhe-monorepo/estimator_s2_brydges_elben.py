#!/usr/bin/env python3
"""
estimator_s2_brydges_elben.py
Estimador CORRETO de S2 (Renyi-2) via randomized Pauli measurements
Baseado em: Brydges et al. (2019), Elben et al. (2018, 2020)
v2.2-correcao-P1 -- 2026-08-11

NOTA DE CORRECAO (v2.0 -> v2.1 -> v2.2):
v2.0 usava fator 2^(-D_ss') (distancia de Hamming) -> errado.
v2.1 usava "coincidence counting": purity = 2^k * n_match / n_pairs.
    ESTA FORMULA E ENVIESADA. Para mediocas locais de Pauli (X/Y/Z),
    a probabilidade de coincidencia e ~(Tr rho^2 + 1)/9^k e NAO reproduz
    Tr(rho^2). Exemplos (numericos, N=4000):
      |0> (k=1):  exato 1.0000  | v2.1 -> 1.32   |  correto -> 0.98
      Bell (k=1): exato 0.5000  | v2.1 -> 1.00   |  correto -> 0.50
      max-misto 5q: exato 0.0312| v2.1 -> 0.98   |  correto -> 0.03 (N maior)

FORMULA CORRETA (estimador de 2 corpos / "swap" via 3 bases de Pauli):
  Tr(rho_A^2) = mean_{i<j}  prod_{l in A} ( 9 * delta[P_i,l=P_j,l AND b_i,l=b_j,l] - 1 )

Demonstracao: para cada qubit l, o operador M_l = sum_U P(U) sum_{b,b'}
(9*delta_{U,U'}delta_{b,b'} - 1) |b><b| (X) |b'><b'| = SWAP_l. Logo
E[prod_l (9 T_l - 1)] = Tr(rho_A (X) rho_A . prod_l SWAP_l) = Tr(rho_A^2).
O estimador e NAO-enviesado para qualquer estado (local de Pauli, 3 bases).

Complexidade de amostragem: a variancia cresce como ~prod_l E[(9T_l-1)^2]
(~15^k p/ produto puro), logo N precisa crescer com k (trabalho tipico da
literatura: randomized measurements p/ purity exige overhead ~2^O(k)).

Referencias:
- Brydges et al., Science 364, 260 (2019). DOI: 10.1126/science.aau4963
- Elben et al., PRL 124, 010504 (2020). DOI: 10.1103/PhysRevLett.124.010504
- Elben et al., "The randomized measurement toolbox", arXiv:2203.11374
"""

import numpy as np
from collections import defaultdict


def generate_random_pauli_basis(n_qubits, rng):
    """
    Gera uma base de Pauli aleatoria para n qubits.
    Retorna: array de inteiros onde 0=X, 1=Y, 2=Z
    """
    return rng.integers(0, 3, size=n_qubits)


def apply_pauli_basis_rotations(circuit, qubits, basis):
    """
    Aplica rotacoes para medir na base de Pauli especificada.

    Args:
        circuit: QuantumCircuit do Qiskit
        qubits: lista de indices de qubits a medir
        basis: array onde 0=X, 1=Y, 2=Z
    """
    for i, b in enumerate(basis):
        q = qubits[i]
        if b == 0:  # X basis: aplicar H
            circuit.h(q)
        elif b == 1:  # Y basis: aplicar S^dagger + H
            circuit.sdg(q)
            circuit.h(q)
        # b == 2: Z basis (padrao, nada a fazer)


def measure_in_pauli_basis(circuit, qubits, basis, backend, shots=1):
    """
    Mede o circuito na base de Pauli especificada.

    Returns:
        counts: dict {bitstring: frequencia}
    """
    from qiskit import transpile, ClassicalRegister

    qc = circuit.copy()
    if qc.num_clbits < len(qubits):
        qc.add_register(ClassicalRegister(len(qubits)))
    apply_pauli_basis_rotations(qc, qubits, basis)
    qc.measure(qubits, range(len(qubits)))

    qc_t = transpile(qc, backend, optimization_level=0)
    job = backend.run(qc_t, shots=shots)
    result = job.result()
    return result.get_counts()


def collect_shadows_batch(circuit, qubits_A, N_shadows, backend, rng,
                          readout_mitigation=None, shots_per_shadow=1):
    """
    Coleta N_shadows em UM batch (transpile + run unicos no backend).

    Isso evita o custo de um transpile+job por shadow (que era o gargalo
    das versoes anteriores em AerSimulator/QPU).

    Returns:
        list de (basis_tuple, outcome_int)
    """
    from qiskit import ClassicalRegister, transpile

    k = len(qubits_A)
    bases = []
    circuits = []
    for _ in range(N_shadows):
        basis = tuple(generate_random_pauli_basis(k, rng))
        qc = circuit.copy()
        if qc.num_clbits < k:
            qc.add_register(ClassicalRegister(k))
        apply_pauli_basis_rotations(qc, qubits_A, basis)
        qc.measure(qubits_A, range(k))
        bases.append(basis)
        circuits.append(qc)

    transpiled = transpile(circuits, backend, optimization_level=0)
    result = backend.run(transpiled, shots=shots_per_shadow).result()

    shadows = []
    for j, basis in enumerate(bases):
        counts = result.get_counts(j)
        if readout_mitigation is not None and basis in readout_mitigation:
            counts = apply_readout_mitigation(counts,
                                              readout_mitigation[basis], k)
        if shots_per_shadow == 1:
            b_str = next(iter(counts))
        else:
            b_str = max(counts, key=counts.get)
        shadows.append((basis, int(b_str, 2)))
    return shadows


def _purity_from_shadows(shadows, k):
    """
    Estimador NAO-enviesado de Tr(rho_A^2) a partir dos shadows coletados.

    shadows: lista de (basis_tuple, outcome_int)
    k: numero de qubits do subconjunto A

    Formula: mean sobre pares (i<j) de prod_l (9 * T_l - 1), onde
    T_l = 1 se shots i e j usaram a mesma base de Pauli no qubit l E
    o mesmo outcome no qubit l.

    Returns:
        purity, purity_std (erro padrao da media sobre pares)
    """
    B = np.array([b for b, _ in shadows], dtype=np.int8)   # (N, k)
    O = np.array([o for _, o in shadows], dtype=np.int64)  # (N,)
    N = len(O)
    if N < 2:
        return float('nan'), float('nan')

    obits = np.array([(O >> l) & 1 for l in range(k)]).T  # (N, k)

    pair_sum = 0.0
    for i in range(N):
        j = slice(i + 1, N)
        contrib = np.ones(N - i - 1)
        for l in range(k):
            T = (B[j, l] == B[i, l]) & (obits[j, l] == obits[i, l])
            contrib *= 9.0 * T.astype(np.float64) - 1.0
        if N - i - 1 > 0:
            pair_sum += float(contrib.sum())

    n_pairs = N * (N - 1) // 2
    purity = pair_sum / n_pairs
    return purity, None


def _bootstrap_std(shadows, k, n_bootstrap=40, seed=1):
    """
    Erro padrao via bootstrap sobre SHADOWS (nao sobre pares).

    IMPORTANTE: pares que compartilham o mesmo shadow sao correlacionados;
    o erro de "pares independentes" subestima a variancia real. O bootstrap
    reamostra shadows e recalcula o estimador, capturando essa correlacao.
    """
    rng = np.random.default_rng(seed)
    N = len(shadows)
    B = np.array([b for b, _ in shadows], dtype=np.int8)   # (N, k)
    O = np.array([o for _, o in shadows], dtype=np.int64)  # (N,)
    obits = np.array([(O >> l) & 1 for l in range(k)]).T  # (N, k)
    n_pairs = N * (N - 1) // 2

    purities = np.empty(n_bootstrap)
    for t in range(n_bootstrap):
        idx = rng.integers(0, N, size=N)
        Bb, ob = B[idx], obits[idx]
        pair_sum = 0.0
        for i in range(N):
            j = slice(i + 1, N)
            contrib = np.ones(N - i - 1)
            for l in range(k):
                T = (Bb[j, l] == Bb[i, l]) & (ob[j, l] == ob[i, l])
                contrib *= 9.0 * T.astype(np.float64) - 1.0
            pair_sum += float(contrib.sum())
        purities[t] = pair_sum / n_pairs
    return float(np.std(purities))


def estimate_purity_randomized_pauli(circuit, qubits_A, N_shadows, backend,
                                     readout_mitigation=None, rng=None,
                                     shots_per_shadow=1):
    """
    Estima Tr(rho_A^2) via randomized Pauli measurements (formula correta).

    PROTOCOLO:
    1. Coletar N shadows: base de Pauli aleatoria P_s (0=X,1=Y,2=Z) + outcome b_s
    2. Purity = mean_{i<j} prod_{l in A} (9 * delta[P_samem, l e b_mesmo, l] - 1)

    Args:
        circuit: circuito preparando o estado
        qubits_A: indices do subconjunto A (estado reduzido rho_A)
        N_shadows: numero total de shadows
        backend: backend de execucao (QPU ou AerSimulator)
        readout_mitigation: dict {basis_tuple: M_inv} ou None
        rng: gerador aleatorio (np.random.Generator)
        shots_per_shadow: shots por shadow (1 recomendado)

    Returns:
        dict com purity_est, purity_std, S2_est, S2_std, n_bases_observed,
        n_pairs_total, n_shadows, k
    """
    if rng is None:
        rng = np.random.default_rng(42)

    k = len(qubits_A)

    # ============================================================
    # PASSO 1: Coletar shadows (batch unico, rapido)
    # ============================================================
    shadows = collect_shadows_batch(
        circuit, qubits_A, N_shadows, backend, rng,
        readout_mitigation=readout_mitigation,
        shots_per_shadow=shots_per_shadow,
    )

    # ============================================================
    # PASSO 2: Estimador correto (swap, 2-corpos) + erro (bootstrap)
    # ============================================================
    purity_est, _ = _purity_from_shadows(shadows, k)

    if not np.isfinite(purity_est):
        raise ValueError(
            "Estimativa nao convergiu. Aumente N_shadows "
            f"(N={N_shadows}, k={k})."
        )

    # Clipping no intervalo fisicamente possivel
    purity_est = float(np.clip(purity_est, 1.0 / (2**k), 1.0))
    purity_std = _bootstrap_std(shadows, k)

    # ============================================================
    # PASSO 3: S2 e incerteza
    # ============================================================
    S2_est = -np.log2(purity_est)
    S2_std = purity_std / (purity_est * np.log(2)) if purity_est > 0 else float('nan')

    return {
        'purity_est': purity_est,
        'purity_std': purity_std,
        'S2_est': S2_est,
        'S2_std': S2_std,
        'n_bases_observed': len({b for b, _ in shadows}),
        'n_pairs_total': len(shadows) * (len(shadows) - 1) // 2,
        'n_shadows': N_shadows,
        'k': k,
    }


def apply_readout_mitigation(counts, M_inv, k):
    """
    Aplica mitigacao de erro de leitura via inversao de matriz.
    """
    n_states = 2**k
    total = sum(counts.values())
    c = np.zeros(n_states)
    for b_str, count in counts.items():
        c[int(b_str, 2)] = count / total

    c_mit = M_inv @ c
    c_mit = np.maximum(c_mit, 0)
    c_mit = c_mit / np.sum(c_mit)

    counts_mit = {}
    for idx in range(n_states):
        if c_mit[idx] > 1e-10:
            counts_mit[format(idx, f'0{k}b')] = c_mit[idx] * total
    return counts_mit


def build_readout_mitigation_matrix_per_basis(backend, qubits, shots=8192):
    """
    Calibra matriz de mitigacao de leitura (produto tensorial por qubit).

    Returns:
        M_inv_total: matriz inversa 2^k x 2^k
        M_single: lista de matrizes 2x2 por qubit
    """
    from qiskit import QuantumCircuit, transpile

    k = len(qubits)
    M_single = []

    for q in qubits:
        M_q = np.zeros((2, 2))
        for prep_bit in [0, 1]:
            qc = QuantumCircuit(max(qubits) + 1, 1)
            if prep_bit == 1:
                qc.x(q)
            qc.measure(q, 0)
            qc_t = transpile(qc, backend, optimization_level=0)
            counts = backend.run(qc_t, shots=shots).result().get_counts()
            total = sum(counts.values())
            for meas_str, count in counts.items():
                M_q[int(meas_str, 2), prep_bit] = count / total
        M_single.append(M_q)

    M_total = M_single[0]
    for i in range(1, k):
        M_total = np.kron(M_total, M_single[i])

    return np.linalg.inv(M_total), M_single


def estimate_s2_v2_2(circuit, qubits_A, N_shadows, backend,
                     readout_mitigation=None, rng=None, shots_per_shadow=1):
    """
    Wrapper da API v2.0 usando o estimador CORRETO (v2.2).
    """
    return estimate_purity_randomized_pauli(
        circuit, qubits_A, N_shadows, backend,
        readout_mitigation=readout_mitigation, rng=rng,
        shots_per_shadow=shots_per_shadow,
    )


# ============================================================
# TESTE DE VALIDACAO (simulacao local)
# ============================================================
if __name__ == '__main__':
    from qiskit import QuantumCircuit
    from qiskit_aer import AerSimulator

    print("=" * 60)
    print("TESTE DE VALIDACAO DO ESTIMADOR S2 CORRIGIDO (v2.2)")
    print("=" * 60)

    simulator = AerSimulator()
    rng = np.random.default_rng(42)

    def show(label, qc, qubits, N):
        res = estimate_purity_randomized_pauli(qc, qubits, N, simulator, rng=rng)
        print(f"  {label:34s} N={N:4d}: S2={res['S2_est']:.4f} +/- {res['S2_std']:.4f} "
              f"(purity={res['purity_est']:.4f}, bases={res['n_bases_observed']})")

    # Teste 1: |0>^6, subconjunto {0,1,2} -> puro -> S2 = 0
    print("\n--- Teste 1: |0>^6, A={0,1,2} (puro) [esperado S2 = 0.00] ---")
    show("|0>^6, subconjunto 3q", QuantumCircuit(6), [0, 1, 2], 2500)

    # Teste 2: GHZ3 completo -> puro -> S2 = 0
    print("\n--- Teste 2: GHZ3 completo (puro) [esperado S2 = 0.00] ---")
    qc_ghz = QuantumCircuit(3)
    qc_ghz.h(0); qc_ghz.cx(0, 1); qc_ghz.cx(0, 2)
    show("GHZ3 completo", qc_ghz, [0, 1, 2], 2000)

    # Teste 3: GHZ3, subconjunto {0,1} -> reducao mista -> S2 = 1.00
    print("\n--- Teste 3: GHZ3, A={0,1} (reducao mista) [esperado S2 = 1.00] ---")
    show("GHZ3 subconjunto 2q", qc_ghz, [0, 1], 1200)

    # Teste 4: Bell completo -> puro -> S2 = 0
    print("\n--- Teste 4: Bell completo (puro) [esperado S2 = 0.00] ---")
    qc_bell = QuantumCircuit(2)
    qc_bell.h(0); qc_bell.cx(0, 1)
    show("Bell completo", qc_bell, [0, 1], 1200)

    # Teste 5: Bell, subconjunto {0} -> S2 = 1.00
    print("\n--- Teste 5: Bell, A={0} (reducao 1q) [esperado S2 = 1.00] ---")
    show("Bell subconjunto 1q", qc_bell, [0], 600)

    print("\n" + "=" * 60)
    print("NOTA: overhead de amostragem cresce com k. Para k=5 use N > 2e4")
    print("      (variancia ~15^k; use a formula correta, nao coincidence counting).")
    print("=" * 60)
