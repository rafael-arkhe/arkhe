# -*- coding: utf-8 -*-
"""toon_ledger.py — Ledger de TOONs por cadeia de hashes (append-only, tamper-evidente).

BLOCO 509 v69 (vetado) — C10/C11/C12:
  * O "ledger Rust via FFI" e o "on-chain TrustGraphs" nao existem no repo —
    REJEITADOS. O nucleo honesto (cadeia de hashes, imutabilidade,
    deteccao de adulteracao) e implementado aqui, em Python puro.
  * Linhagem: replicando o modelo blockchain-style ja presente em
    nexus_agi/immutable_ledger_system.py (referencia cruzada, sem copia).
  * Persistencia JSON (C12) com round-trip verificavel.
"""

import hashlib
import json
import os
import time
from typing import Dict, List, Optional


def sha256text(s: str) -> str:
    return hashlib.sha256(s.encode("utf-8")).hexdigest()


class ToonLedger:
    def __init__(self) -> None:
        self.chain: List[Dict] = []
        genesis_record = {"type": "genesis", "id": "GENESIS"}
        payload = json.dumps(genesis_record, sort_keys=True)
        genesis_hash = sha256text(f"0|{'0' * 64}|{payload}")
        self.chain.append({
            "index": 0,
            "record": genesis_record,
            "prev_hash": "0" * 64,
            "block_hash": genesis_hash,
            "created_at": 0.0,
        })

    def _append(self, record: Dict, prev_hash: str) -> Dict:
        index = len(self.chain)
        payload = json.dumps(record, sort_keys=True)
        block_hash = sha256text(f"{index}|{prev_hash}|{payload}")
        block = {
            "index": index,
            "record": record,
            "prev_hash": prev_hash,
            "block_hash": block_hash,
            "created_at": time.time(),
        }
        self.chain.append(block)
        return block

    def append(self, record: Dict) -> str:
        prev = self.chain[-1]["block_hash"] if self.chain else "0" * 64
        block = self._append(record, prev)
        return block["block_hash"]

    def verify_chain(self) -> bool:
        for i, block in enumerate(self.chain):
            payload = json.dumps(block["record"], sort_keys=True)
            expected = sha256text(f"{i}|{block['prev_hash']}|{payload}")
            if block["block_hash"] != expected:
                return False
            if i > 0 and block["prev_hash"] != self.chain[i - 1]["block_hash"]:
                return False
        return True

    def verify_block(self, i: int) -> bool:
        if i < 0 or i >= len(self.chain):
            return False
        block = self.chain[i]
        payload = json.dumps(block["record"], sort_keys=True)
        return block["block_hash"] == sha256text(f"{i}|{block['prev_hash']}|{payload}")

    def save(self, path: str) -> None:
        with open(path, "w", encoding="utf-8") as f:
            json.dump({"chain": self.chain}, f)

    def load(self, path: str) -> "ToonLedger":
        with open(path, "r", encoding="utf-8") as f:
            data = json.load(f)
        ledger = ToonLedger.__new__(ToonLedger)
        ledger.chain = data["chain"]
        return ledger


def selftest() -> None:
    print("=== TOON LEDGER SELFTEST (hash chain, tamper-evident) ===")
    ledger = ToonLedger()
    h1 = ledger.append({"type": "change", "target": "algorithm", "phi_c": 0.90})
    h2 = ledger.append({"type": "change", "target": "ontology", "phi_c": 0.84})
    h3 = ledger.append({"type": "rollback", "target": "algorithm", "phi_c": 0.77})

    assert ledger.verify_chain() is True
    assert len(ledger.chain) == 4
    assert h1 != h2 and h2 != h3
    assert ledger.chain[3]["prev_hash"] == ledger.chain[2]["block_hash"]
    print(f"[LED] chain of {len(ledger.chain)} blocks verified (no tamper)")

    ledger.chain[2]["record"]["phi_c"] = 0.99
    assert ledger.verify_chain() is False
    assert ledger.verify_block(2) is False
    assert ledger.verify_block(3) is True
    print("[LED] tamper at block 2 detected; blocks 3 hash lookup unchanged")

    tmp = os.path.join(os.environ.get("TMP", os.environ.get("TEMP", ".")), "catedral_block509.json")
    clean = ToonLedger()
    clean.append({"type": "change", "target": "rng", "phi_c": 0.81})
    clean.save(tmp)
    loaded = ToonLedger().load(tmp)
    assert loaded.verify_chain() is True
    assert [b["block_hash"] for b in clean.chain] == [b["block_hash"] for b in loaded.chain]
    os.remove(tmp)
    print("[LED] persistence round-trip verified (C12)")
    print("=== TOON LEDGER SELFTEST PASSED ===")


if __name__ == "__main__":
    selftest()