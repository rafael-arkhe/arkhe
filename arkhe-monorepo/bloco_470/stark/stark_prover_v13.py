# =============================================================================
# BLOCO 470 v13 — zk-STARKs com Cairo + stone-cli (O4)
# stark_prover_v13.py
#
# Prover/verifier para zk-STARKs usando Cairo (Scarb) e stone-cli.
#
# Requer: scarb (instalado), stone-cli (binário no PATH),
#         pip install web3 (apenas p/ verify_on_ethereum)
# Env:    ETH_RPC_URL (apenas p/ verify_on_ethereum)
# =============================================================================
import subprocess
import json
import tempfile
import os
from pathlib import Path
from typing import Dict, Optional
import hashlib
from datetime import datetime


class StarkProver:
    """
    Prover para zk-STARKs usando Cairo e stone-cli.
    Baseado no fluxo: scatter build -> stone-cli prove/verify.
    """

    CAIRO_DIR = Path(__file__).parent / "cairo"
    PROOF_DIR = Path(__file__).parent / "proofs"

    # Programas Cairo disponíveis no crate governance_stark
    PROGRAMS = ("governance_consensus", "policy_evaluation")

    @classmethod
    def compile_cairo(cls, program_name: str) -> Dict:
        """
        Compila o projeto Cairo usando Scarb.
        Retorna caminhos para sierra.json e casm.json.
        """
        if program_name not in cls.PROGRAMS:
            raise ValueError(f"Programa desconhecido: {program_name}. Use {cls.PROGRAMS}")

        cmd = ["scarb", "build", "--manifest-path", str(cls.CAIRO_DIR / "Scarb.toml")]
        result = subprocess.run(cmd, capture_output=True, text=True)
        if result.returncode != 0:
            raise RuntimeError(f"Compilação Cairo falhou: {result.stderr}")

        module_name = program_name.replace("_", "")
        return {
            "sierra": cls.CAIRO_DIR / "target" / "release" / f"{module_name}.sierra.json",
            "casm": cls.CAIRO_DIR / "target" / "release" / f"{module_name}.casm.json",
        }

    @classmethod
    def prove(cls, program_name: str, inputs: Dict) -> Dict:
        """
        Gera uma prova STARK usando stone-cli.
        """
        # 1. Compila o programa
        compiled = cls.compile_cairo(program_name)

        # 2. Prepara os inputs (stone-cli espera felt252 escalados como strings)
        input_file = cls._prepare_inputs(inputs)

        # 3. Executa stone-cli prove
        cls.PROOF_DIR.mkdir(parents=True, exist_ok=True)
        proof_path = cls.PROOF_DIR / f"{program_name}_proof.json"
        public_path = cls.PROOF_DIR / f"{program_name}_public.json"
        trace_path = cls.PROOF_DIR / f"{program_name}_trace.bin"

        cmd = [
            "stone-cli", "prove",
            "--program", str(compiled["casm"]),
            "--program-input", str(input_file),
            "--output", str(proof_path),
            "--public-output", str(public_path),
            "--trace", str(trace_path),
        ]

        result = subprocess.run(cmd, capture_output=True, text=True)
        if result.returncode != 0:
            raise RuntimeError(f"stone-cli prove falhou: {result.stderr}")

        proof_data = json.loads(proof_path.read_text())
        public_data = json.loads(public_path.read_text())

        return {
            "proof_type": "stark",
            "proof": proof_data,
            "public_inputs": public_data,
            "program": program_name,
            "curve": "ed25519",
            "timestamp": datetime.utcnow().isoformat(),
        }

    @classmethod
    def verify(cls, proof_data: Dict) -> bool:
        """
        Verifica uma prova STARK com stone-cli (local, off-chain).
        """
        cls.PROOF_DIR.mkdir(parents=True, exist_ok=True)
        proof_path = cls.PROOF_DIR / "verify_input.json"
        proof_path.write_text(json.dumps(proof_data.get("proof")))

        cmd = [
            "stone-cli", "verify",
            "--proof", str(proof_path),
            "--public-output", str(proof_path),
        ]

        result = subprocess.run(cmd, capture_output=True, text=True)
        return result.returncode == 0

    @classmethod
    def verify_on_ethereum(cls, proof_data: Dict, verifier_address: str) -> bool:
        """
        Verifica uma prova STARK no Ethereum (EVM adapter).
        Baseado no stark-evm-adapter da zksecurity.
        """
        from web3 import Web3

        w3 = Web3(Web3.HTTPProvider(os.environ.get("ETH_RPC_URL", "https://rpc.sepolia.org")))

        # ABI mínima do verified contract (Stark EVM Adapter)
        abi = [
            {
                "inputs": [{"type": "bytes", "name": "proof"}],
                "name": "verify",
                "outputs": [{"type": "bool"}],
                "stateMutability": "view",
                "type": "function",
            }
        ]

        contract = w3.eth.contract(address=verifier_address, abi=abi)

        try:
            proof_bytes = json.dumps(proof_data.get("proof")).encode()
            return bool(contract.functions.verify(proof_bytes).call())
        except Exception as e:
            print(f"Ethereum verification failed: {e}")
            return False

    @classmethod
    def _prepare_inputs(cls, inputs: Dict) -> Path:
        """Prepara o arquivo de inputs JSON para stone-cli."""
        with tempfile.NamedTemporaryFile(
            mode="w", suffix=".json", prefix="stone_inputs_", delete=False
        ) as f:
            json.dump(inputs, f)
            path = Path(f.name)
        return path


if __name__ == "__main__":
    import sys

    program = sys.argv[1] if len(sys.argv) > 1 else "governance_consensus"
    sample = {
        "governance_consensus": {
            "phi_values": [90, 85, 78],
            "weights": [3, 2, 1],
            "threshold": 85,
        },
        "policy_evaluation": {"phi_scaled": 92, "threshold": 85, "critical_violations": 0},
    }[program]
    proof = StarkProver.prove(program, sample)
    print(json.dumps(proof, indent=2, default=str)[:500])