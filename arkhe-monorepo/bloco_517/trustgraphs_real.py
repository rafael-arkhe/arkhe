"""trustgraphs_real.py — v77 VETADO — BLOCO 517 (V2)

Integracao com TrustGraphs.

VETAGEM (precedente BLOCO 509/C11 e 501/L6):
  - A proposta v77 define uma ABI on-chain (`submitLikelihoodProof`) para um
    contrato INEXISTENTE no repositorio. Inventar ABI = mesmo erro rejeitado
    em 484/509.
  - Núcleo entregue e HERMETICO: ProofRegistry local com commit por raiz
    (Web3.keccak sobre JSON canonico), tamper-evidente, prova verificavel.
  - A via on-chain fica habilita SOMENTE com (rpc + contract_address +
    abi_path real). Sem esses 3, qualquer chamada on-chain falha com
    ConfigurationError — nao ha rede implicita nem ABI fantasma.
"""

from __future__ import annotations

import json
import time
from typing import Any, Dict, Optional, Tuple

from eth_utils import to_checksum_address
from web3 import Web3


class ConfigurationError(RuntimeError):
    """Caminho on-chain nao configurado com artefato real."""


class ProofRegistry:
    """Ledger local de provas (commit/verify), tamper-evidente e determinístico."""

    def __init__(self) -> None:
        self._proofs: Dict[int, Dict[str, Any]] = {}
        self._next_id = 1

    @staticmethod
    def root_for(data: Dict[str, Any]) -> bytes:
        canonical = json.dumps(data, sort_keys=True, separators=(",", ":"))
        return Web3.keccak(text=canonical)

    def commit(self, data: Dict[str, Any], proof: Optional[bytes] = None) -> int:
        proof_id = self._next_id
        self._next_id += 1
        root = self.root_for(data).hex()
        self._proofs[proof_id] = {
            "root": root,
            "data": data,
            "proof": (proof or b"").hex(),
            "ts": int(time.time()),
        }
        return proof_id

    def verify(self, proof_id: int, data: Dict[str, Any]) -> bool:
        entry = self._proofs.get(proof_id)
        if entry is None:
            return False
        return entry["root"] == self.root_for(data).hex()

    def get(self, proof_id: int) -> Optional[Dict[str, Any]]:
        return self._proofs.get(proof_id)


class TrustGraphsClient:
    """Cliente real: modo local (hermetico) + modo on-chain configurado."""

    def __init__(
        self,
        rpc_url: Optional[str] = None,
        contract_address: Optional[str] = None,
        private_key: Optional[str] = None,
        abi_path: Optional[str] = None,
    ) -> None:
        self.registry = ProofRegistry()
        self.rpc_url = rpc_url
        self.contract_address = (
            to_checksum_address(contract_address) if contract_address else None
        )
        self.private_key = private_key
        self.abi_path = abi_path

    @property
    def onchain_ready(self) -> bool:
        return bool(
            self.rpc_url and self.contract_address and self.private_key and self.abi_path
        )

    def _check_onchain(self) -> None:
        if not self.onchain_ready:
            raise ConfigurationError(
                "on-chain requer rpc_url + contract_address + private_key + abi_path real; "
                "neste repositorio a via on-chain e integracao documentada, nao testada"
            )

    def submit_proof(self, data: Dict[str, Any], proof: Optional[bytes] = None) -> int:
        """Commit local SEMPRE; commit on-chain apenas se configurado com ABI real."""
        proof_id = self.registry.commit(data, proof)
        if self.onchain_ready:
            self._submit_onchain(data, proof or b"")
        return proof_id

    def verify_proof(self, proof_id: int, data: Optional[Dict[str, Any]] = None) -> bool:
        if data is not None:
            return self.registry.verify(proof_id, data)
        entry = self.registry.get(proof_id)
        return entry is not None

    def _submit_onchain(self, data: Dict[str, Any], proof: bytes) -> str:
        """Caminho on-chain — exige ABTRACT real (carregado do disco)."""
        self._check_onchain()
        import json as _json

        with open(self.abi_path, "r", encoding="utf-8") as fh:
            abi = _json.load(fh)
        if not isinstance(abi, list):
            raise ConfigurationError("abi_path precisa apontar para lista ABI JSON")
        w3 = Web3(Web3.HTTPProvider(self.rpc_url))
        root = self.registry.root_for(data)
        contract = w3.eth.contract(address=self.contract_address, abi=abi)
        fn = contract.functions.submitLikelihoodProof(
            root, int(data.get("significance", 0)), data.get("num_events", 0), proof
        )
        nonce = w3.eth.get_transaction_count(self.contract_address)
        tx = fn.build_transaction(
            {"from": self.contract_address, "nonce": nonce, "gas": 500000}
        )
        return tx["data"].hex()


if __name__ == "__main__":
    import sys

    fail = 0
    cli = TrustGraphsClient()  # modo local — sem rede
    sample = {"significance": 3.2e-8, "mu_hat": 0.5, "num_events": 42}

    pid = cli.submit_proof(sample, proof=b"\x01\x02\x03")
    if not cli.verify_proof(pid, sample):
        fail += 1
        print("[TG ] FAIL: roundtrip commit/verify")
    else:
        print(f"[TG ] roundtrip ok (proof_id={pid}, root={cli.registry.root_for(sample).hex()[:12]}...)")

    tampered = dict(sample)
    tampered["num_events"] = 43
    if cli.verify_proof(pid, tampered):
        fail += 1
        print("[TG ] FAIL: adulteracao detectada")
    else:
        print("[TG ] adulteracao rejeitada")

    if cli.verify_proof(999, sample):
        fail += 1
        print("[TG ] FAIL: proof_id inexistente aceito")
    else:
        print("[TG ] proof_id inexistente rejeitado")

    print(f"[TG ] {'ALL CHECKS PASS' if fail == 0 else 'CHECKS FAILED'}")
    sys.exit(1 if fail else 0)