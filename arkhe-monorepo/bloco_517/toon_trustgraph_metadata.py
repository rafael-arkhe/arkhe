"""toon_trustgraph_metadata.py — v77 VETADO — BLOCO 517 (E2)

TOON (cadeia de hashes, bloco 509/toon_ledger) com campo
`trustgraph_proof` embutido no metadata.

Em cada TOON emitido, o metadata passa a carregar:
    trustgraph_proof = { proof_id, root, data_hash, ts }
onde data_hash = keccak(JSON canonico do payload). A verificacao recalcula
os dois hashes e compara — hermetica, sem rede.
"""

from __future__ import annotations

import json
from typing import Any, Dict

from web3 import Web3


def _canonical(data: Dict[str, Any]) -> str:
    return json.dumps(data, sort_keys=True, separators=(",", ":"))


def embed_trustgraph_proof(toon_meta: Dict[str, Any], proof: Dict[str, Any]) -> Dict[str, Any]:
    """Insere o bloco trustgraph_proof no metadata do TOON."""
    out = dict(toon_meta)
    payload = {k: v for k, v in toon_meta.items() if k != "trustgraph_proof"}
    out["trustgraph_proof"] = {
        "proof_id": proof["proof_id"],
        "root": proof.get("root", ""),
        "data_hash": Web3.keccak(text=_canonical(payload)).hex(),
        "ts": proof.get("ts", 0),
    }
    return out


def verify_toon_metadata(toon_meta: Dict[str, Any]) -> bool:
    """Recalcula data_hash sobre o payload (excluindo o proprio campo)."""
    proof_block = toon_meta.get("trustgraph_proof")
    if not proof_block or "data_hash" not in proof_block:
        return False
    payload = {k: v for k, v in toon_meta.items() if k != "trustgraph_proof"}
    return _data_hash(payload) == proof_block["data_hash"]


def _data_hash(payload: Dict[str, Any]) -> str:
    return Web3.keccak(text=_canonical(payload)).hex()


if __name__ == "__main__":
    import sys
    from trustgraphs_real import ProofRegistry

    fail = 0
    base = {"toon": "CATEDRAL-517", "from": "0xdbc69a52278b9deeed1d6cf099a583dd9d9708fd", "nonce": 7}
    reg = ProofRegistry()
    pid = reg.commit(base)
    entry = reg.get(pid)
    proof = {"proof_id": pid, "root": entry["root"], "ts": entry["ts"]}

    meta = embed_trustgraph_proof(base, proof)
    if not verify_toon_metadata(meta):
        fail += 1
        print("[TOON] FAIL: metadata invalido")
    else:
        print(f"[TOON] metadata valido (proof_id={pid})")

    forged = dict(meta)
    forged["nonce"] = 8
    if verify_toon_metadata(forged):
        fail += 1
        print("[TOON] FAIL: payload adulterado aceito")
    else:
        print("[TOON] payload adulterado rejeitado")

    if "trustgraph_proof" not in base and not verify_toon_metadata(base):
        print("[TOON] TOON sem prova corretamente rejeitado")

    print(f"[TOON] {'ALL CHECKS PASS' if fail == 0 else 'CHECKS FAILED'}")
    sys.exit(1 if fail else 0)