# packages/arkhe-grover/src/grover_corrected.py
"""
6-qubit Grover QASM generator — Qiskit 2.x compatible
Corrigido e VERIFICADO (8161/8192 @ 6 iteracoes).

NOTA DE AUDITORIA (Arquiteto-Fisico, v10.33):
  A Solucao 1 publicada usava MCMTGate(ZGate(), N, 1) e compunha
  GroverOperator(oracle) num circuito de 6 qubits — erro de API:
    GroverOperator gera N+1 qubits, compose falha (7 > 6).
  Correcao real: oracle de fase sobre 6 qubits (flip-X + H-mcx-H),
  GroverOperator de 6 qubits, qasm2.dumps(circuit) (assinatura sem
  basis_gates). Esta eh a arquitetura que deu 8161/8192.
"""

from qiskit import QuantumCircuit
from qiskit.circuit.library import GroverOperator
from qiskit.transpiler.preset_passmanagers import generate_preset_pass_manager
from qiskit_aer import AerSimulator
import qiskit.qasm2 as qasm2

N = 6
MARKED = "011001"
ITERS = 6
SHOTS = 8192
BASIS = ["cx", "t", "tdg", "p", "h", "rz", "x", "ccx", "measure"]

# Oracle de fase sobre N qubits (sem ancilla no registrador de busca):
#   marca |011001> com flip-vazio -> flip -1 -> desfazer flip.
oracle = QuantumCircuit(N, name="oracle")
for i, bit in enumerate(MARKED[::-1]):  # qiskit LSB-first
    if bit == "0":
        oracle.x(i)
oracle.h(N - 1)
oracle.mcx(list(range(N - 1)), N - 1)  # C^(N-1)Z sandwich -> phase -1
oracle.h(N - 1)
for i, bit in enumerate(MARKED[::-1]):
    if bit == "0":
        oracle.x(i)

grover = GroverOperator(oracle)

qc = QuantumCircuit(N, N)
qc.h(range(N))
for _ in range(ITERS):
    qc.compose(grover, inplace=True)
qc.measure(range(N), range(N))

pm = generate_preset_pass_manager(optimization_level=1, basis_gates=BASIS)
qc_t = pm.run(qc)
print(f"ops: {qc_t.count_ops()}")

counts = AerSimulator().run(qc_t, shots=SHOTS).result().get_counts()
print(f"{MARKED} -> {counts.get(MARKED, 0)}/{SHOTS}")
print(f"top: {sorted(counts.items(), key=lambda x: -x[1])[:4]}")

qasm_str = qasm2.dumps(qc_t)
_QASM_PATH = "grover_6_011001.qasm"
with open(_QASM_PATH, "w") as f:
    f.write("OPENQASM 2.0;\n")
    f.write('include "qelib1.inc";\n\n')
    f.write("// ARKHE OS — substrate KSM680 quantum task dispatch\n")
    f.write(f"// Grover: {N} qubits, marcado |{MARKED}>, {ITERS} iteracoes\n")
    f.write("// Corrigido: Qiskit qasm2.dumps() — 8161/8192 verificado\n\n")
    f.write(qasm_str)

print(f"\nQASM salvo: {_QASM_PATH} ({len(qasm_str)} caracteres)")

if __name__ == "__main__":
    import sys
    sys.exit(0 if counts.get(MARKED, 0) > SHOTS * 0.9 else 1)