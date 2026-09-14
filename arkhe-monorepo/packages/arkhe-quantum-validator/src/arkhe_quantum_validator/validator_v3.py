# packages/arkhe-quantum-validator/src/arkhe_quantum_validator/validator_v3.py
"""
Validador quantico V3.0 — decompose correta de Grover.
Gera QASM (script corrigido ou ancilla), simula, certifica.   (v10.33)
"""

import hashlib
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Dict

try:
    from qiskit import QuantumCircuit
    from qiskit.circuit.library import GroverOperator
    from qiskit.transpiler.preset_passmanagers import generate_preset_pass_manager
    from qiskit_aer import AerSimulator
except ImportError as exc:  # pragma: no cover
    raise RuntimeError("validator_v3 requires qiskit + qiskit-aer") from exc


class QuantumResilienceValidatorV3:
    def __init__(self, use_ancilla: bool = False):
        self.use_ancilla = use_ancilla
        here = Path(__file__).parent
        self.script_path = here.parent.parent.parent / "arkhe-grover" / "src" / "grover_corrected.py"
        self.ancilla_qasm = (
            here.parent.parent.parent / "arkhe-grover" / "src" / "grover_6_011001_ancilla.qasm"
        )

    def _build_corrected_circuit(self, n: int = 6, marked: str = "011001", iters: int = 6):
        oracle = QuantumCircuit(n, name="oracle")
        for i, bit in enumerate(marked[::-1]):
            if bit == "0":
                oracle.x(i)
        oracle.h(n - 1)
        oracle.mcx(list(range(n - 1)), n - 1)
        oracle.h(n - 1)
        for i, bit in enumerate(marked[::-1]):
            if bit == "0":
                oracle.x(i)
        grover = GroverOperator(oracle)
        qc = QuantumCircuit(n, n)
        qc.h(range(n))
        for _ in range(iters):
            qc.compose(grover, inplace=True)
        qc.measure(range(n), range(n))
        return qc, n, marked

    def validate(self, latent_vector=None, shots: int = 8192, iters: int = 6) -> Dict:
        target = "011001"
        if latent_vector and len(latent_vector) == 6:
            target = "".join(str(1 if v > 0.5 else 0) for v in latent_vector)

        if self.use_ancilla:
            from qiskit.qasm2 import load as qasm2_load
            qc = qasm2_load(str(self.ancilla_qasm))
            # ancilla QASM ja e o circuito 6-qubit marcado 011001
            target = "011001"
        else:
            qc, _, _ = self._build_corrected_circuit(iters=iters)

        pm = generate_preset_pass_manager(
            optimization_level=1,
            basis_gates=["cx", "t", "tdg", "p", "h", "rz", "x", "ccx", "measure"],
        )
        qc_t = pm.run(qc)
        counts = AerSimulator().run(qc_t, shots=shots).result().get_counts()
        fidelity = counts.get(target, 0) / shots

        qasm_src = None
        if self.use_ancilla:
            qasm_src = self.ancilla_qasm.read_text()
        else:
            import qiskit.qasm2 as qasm2
            qasm_src = qasm2.dumps(qc_t)

        return {
            "marked_state": target,
            "fidelity": fidelity,
            "is_resilient": fidelity > 0.9,
            "circuit_hash": hashlib.sha3_256(qasm_src.encode()).hexdigest()[:16],
            "backend": "aer_simulator",
            "decomposition": "ancilla" if self.use_ancilla else "qiskit",
            "shots": shots,
            "counts_total": len(counts),
        }


if __name__ == "__main__":
    for anc in (False, True):
        r = QuantumResilienceValidatorV3(use_ancilla=anc).validate()
        print(f"decomposition={r['decomposition']:7s} "
              f"fidelity={r['fidelity']:.4f} "
              f"resilient={r['is_resilient']} "
              f"hash={r['circuit_hash']}")