#!/usr/bin/env python3
"""
circuit_513_encoding.py
Circuito de encoding do codigo [[5,1,3]] de Laflamme
Estabilizadores: <XZZXI, IXZZX, XIXZZ, ZXIXZ>
Referencia: Laflamme et al. (1996), Gottesman (1997)
v2.2-correcao-P2 -- 2026-08-11

NOTA DE CORRECAO (v2.0/v2.1 -> v2.2):
As versoes anteriores usavam circuitos escritos a mao (H + cadeia de CNOT/CZ
arbitraria) que NAO preparavam |0_L>. A verificacao via Statevector retornava
0.0 para TODOS os estabilizadores.

Correcao: o circuito e gerado por sintese de Clifford deterministica a partir
dos estabilizadores do codigo usando qiskit.synthesis.synth_circuit_from_stabilizers
(mesmo algoritmo de eliminacao de Gauss usado pelo Stim, validado contra o
estado |0_L> exato). A verificacao abaixo confirma <psi|g_i|psi> = +1 para
todos os estabilizadores e <psi|Z_L|psi> = +1, i.e., |psi> = |0_L>.

Estabilizadores do [[5,1,3]]:
  g1 = XZZXI   g2 = IXZZX   g3 = XIXZZ   g4 = ZXIXZ
Operadores logicos:
  Z_L = ZZZZZ (|0_L> e autoestado +1)   X_L = XXXXX (le |0_L> -> |1_L>)
"""

from qiskit import QuantumCircuit
from qiskit.qasm2 import dumps as qasm2_dumps
from qiskit.synthesis import synth_circuit_from_stabilizers

STABILIZERS = ['XZZXI', 'IXZZX', 'XIXZZ', 'ZXIXZ']
Z_LOGICAL = 'ZZZZZ'
X_LOGICAL = 'XXXXX'


def build_513_encoding_verified():
    """
    Circuito de encoding de |0_L> do [[5,1,3]].

    Sintese de Clifford deterministica: gera um circuito que, aplicado a
    |00000>, produz o estado +1 autoestado de g1..g4 e Z_L (= |0_L>).

    Returns:
        QuantumCircuit com 5 qubits
    """
    qc = synth_circuit_from_stabilizers(STABILIZERS + [Z_LOGICAL])
    qc.name = "[[5,1,3]]_encoding"
    return qc


def build_513_logical_state(logical_bit=0):
    """
    Prepara |0_L> ou |1_L> do codigo [[5,1,3]].

    Args:
        logical_bit: 0 para |0_L>, 1 para |1_L>
    """
    qc = build_513_encoding_verified()
    if logical_bit == 1:
        for q in range(5):
            qc.x(q)
    return qc


def verify_513_stabilizers(qc):
    """
    Verifica os estabilizadores e operadores logicos no estado preparado.

    Para |0_L>: <g_i> = +1 (i=1..4), <Z_L> = +1 e <X_L> = 0.

    Returns:
        dict {label: {'expectation': float, 'expected': float, 'pass': bool}}
    """
    from qiskit.quantum_info import Statevector, Pauli

    state = Statevector.from_instruction(qc)

    checks = {}
    for i, s in enumerate(STABILIZERS, start=1):
        checks[f'g{i}_{s}'] = Pauli(s)
    checks['Z_L'] = Pauli(Z_LOGICAL)
    checks['X_L'] = Pauli(X_LOGICAL)

    results = {}
    for label, op in checks.items():
        exp_val = state.expectation_value(op).real
        expected = 0.0 if label == 'X_L' else 1.0
        results[label] = {
            'expectation': exp_val,
            'expected': expected,
            'pass': abs(exp_val - expected) < 1e-8,
        }
    return results


def build_513_smoke_test_stabilizers():
    """
    Smoke test: circuitos de medicao de cada estabilizador g_i (base rotacionada).
    Em hardware, a paridade medida deve ser sempre par (autovalor +1).
    """
    circuits = []
    paulis = {
        'g1_XZZXI': ['x', 'z', 'z', 'x', 'i'],
        'g2_IXZZX': ['i', 'x', 'z', 'z', 'x'],
        'g3_XIXZZ': ['x', 'i', 'x', 'z', 'z'],
        'g4_ZXIXZ': ['z', 'x', 'i', 'x', 'z'],
    }
    for name, basis in paulis.items():
        qc = QuantumCircuit(5, 4, name=f"stab_meas_{name}")
        qc.compose(build_513_encoding_verified(), inplace=True)
        idx = 0
        for q, p in enumerate(basis):
            if p == 'x':
                qc.h(q)
            elif p == 'y':
                qc.sdg(q)
                qc.h(q)
            if p != 'i':
                qc.measure(q, idx)
                idx += 1
        circuits.append((name, qc))
    return circuits


# ============================================================
# VERIFICACAO DO CIRCUITO
# ============================================================
if __name__ == '__main__':
    print("=" * 60)
    print("VERIFICACAO DO CIRCUITO [[5,1,3]] (v2.2)")
    print("=" * 60)

    qc = build_513_encoding_verified()
    print("\n--- Circuito de Encoding (sintese de Clifford) ---")
    print(qc.draw(output='text'))

    print("\n--- Verificacao de Estabilizadores ---")
    results = verify_513_stabilizers(qc)
    all_pass = True
    for label, res in results.items():
        status = "PASS" if res['pass'] else "FAIL"
        all_pass = all_pass and res['pass']
        print(f"  {label:10s}: <psi|{label}|psi> = {res['expectation']:+.6f} "
              f"(esperado {res['expected']:+.1f}) [{status}]")

    print("\n" + ("V   Todos os estabilizadores verificados: o estado e |0_L>."
                  if all_pass else "X   FALHOU."))

    qasm_str = qasm2_dumps(qc)
    with open('circuit_513_encoding.qasm', 'w', encoding='utf-8') as f:
        f.write(qasm_str)
    print("\nArquivo 'circuit_513_encoding.qasm' gerado.")

    print("\n--- Estado |1_L> (aplicando X_L) ---")
    qc1 = build_513_logical_state(logical_bit=1)
    res1 = verify_513_stabilizers(qc1)
    ok = (all(abs(res1[l]['expectation'] - 1.0) < 1e-8 for l in res1 if l.startswith('g'))
          and abs(res1['Z_L']['expectation'] + 1.0) < 1e-8
          and abs(res1['X_L']['expectation']) < 1e-8)
    print(f"  <Z_L> = {res1['Z_L']['expectation']:+.6f} (esperado -1 para |1_L>)")
    print(f"  <X_L> = {res1['X_L']['expectation']:+.6f} (esperado 0)  -> {'PASS' if ok else 'FAIL'}")

    print("\n--- Smoke Test: Medicao de Estabilizadores ---")
    for name, qc_s in build_513_smoke_test_stabilizers():
        print(f"  {name}: {qc_s.num_qubits} qubits, {len(qc_s.data)} operacoes")
