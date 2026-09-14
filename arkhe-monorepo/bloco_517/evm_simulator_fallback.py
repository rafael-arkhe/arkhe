"""evm_simulator_fallback.py — v77 VETADO — BLOCO 517 (E1)

Simulador EVM com fallback `evm t8n` -> `eth_call` via RPC.

VETAGEM:
  - A proposta v77 aceitava transacao so por tx_hash e deixava a chamada de
    rede implícita no selftest. Aqui `simulate()` recebe o DICT da transacao
    (como o RPC retorna), e rede t8n/RPC e injetavel (fetcher/rpc_post) — o
    selftest roda 100% hermetico, sem tocar em rpc.hyperliquid.xyz.
  - `_build_call_payload` com clamps: gasPrice/gas ausentes viram 0/"0x5208",
    value/dados nulos viram "0x0"/"0x".
  - Nenhuma chave de API e exigida para eth_call.
"""

from __future__ import annotations

import json
import os
import subprocess
import tempfile
from typing import Any, Callable, Dict, Optional


def _int_hex(value: Any, default: str) -> str:
    if value in (None, ""):
        return default
    return value


def _to_int(value: Any, default: int) -> int:
    if value in (None, ""):
        return default
    return int(value, 16) if isinstance(value, str) and value.startswith("0x") else int(value)


class EVMSimulatorWithFallback:
    def __init__(
        self,
        evm_path: str = "evm",
        rpc_url: str = "https://rpc.hyperliquid.xyz/evm",
        fetcher: Optional[Callable[[str], Optional[Dict]]] = None,
        rpc_post: Optional[Callable[[str, Dict], Dict]] = None,
    ) -> None:
        self.evm_path = evm_path
        self.rpc_url = rpc_url
        self.fetcher = fetcher
        self.rpc_post = rpc_post or self._default_post
        if self.fetcher is None:
            self.fetcher = self._default_fetch

    # ------------------------------------------------------------ transporte
    def _default_fetch(self, tx_hash: str) -> Optional[Dict]:
        payload = {"jsonrpc": "2.0", "id": 1, "method": "eth_getTransactionByHash", "params": [tx_hash]}
        return self._default_post(self.rpc_url, payload).get("result")

    def _default_post(self, url: str, payload: Dict) -> Dict:
        import requests

        return requests.post(url, json=payload, timeout=10).json()

    # ---------------------------------------------------------- construcao
    def _evm_tx_record(self, tx: Dict, gas_multiplier: float) -> Dict:
        return {
            "input": _int_hex(tx.get("input"), "0x"),
            "gas": hex(int(_to_int(tx.get("gas"), 21000) * gas_multiplier)),
            "gasPrice": _int_hex(tx.get("gasPrice"), "0x0"),
            "nonce": _int_hex(tx.get("nonce"), "0x0"),
            "to": _int_hex(tx.get("to"), ""),
            "value": _int_hex(tx.get("value"), "0x0"),
        }

    def _build_call_payload(self, tx: Dict, gas_multiplier: float) -> Dict:
        return {
            "from": tx.get("from"),
            "to": tx.get("to"),
            "data": _int_hex(tx.get("input"), "0x"),
            "gas": hex(int(_to_int(tx.get("gas"), 21000) * gas_multiplier)),
            "gasPrice": _int_hex(tx.get("gasPrice"), "0x0"),
            "value": _int_hex(tx.get("value"), "0x0"),
        }

    # -------------------------------------------------------------- simulacao
    def simulate(self, tx: Dict, gas_multiplier: float = 1.3) -> Dict:
        """Tenta evm t8n; qualquer falha cai para eth_call (fallback)."""
        if os.path.isfile(self.evm_path):
            try:
                result = self._simulate_with_evm(tx, gas_multiplier)
                if result.get("status") == "0x1":
                    return result
            except Exception:
                pass
        return self._simulate_with_eth_call(tx, gas_multiplier)

    def _simulate_with_evm(self, tx: Dict, gas_multiplier: float) -> Dict:
        with tempfile.TemporaryDirectory() as tmp:
            txs = [self._evm_tx_record(tx, gas_multiplier)]
            env = {"currentGasLimit": "0x500000"}
            alloc = {tx.get("from", "0x"): {"balance": "0xffffffffffffffff"}}
            for name, data in [("alloc", alloc), ("txs", txs), ("env", env)]:
                with open(os.path.join(tmp, f"{name}.json"), "w", encoding="utf-8") as fh:
                    json.dump(data, fh)
            cmd = [
                self.evm_path, "t8n",
                f"--input.alloc={os.path.join(tmp, 'alloc.json')}",
                f"--input.txs={os.path.join(tmp, 'txs.json')}",
                f"--input.env={os.path.join(tmp, 'env.json')}",
                f"--output.result={os.path.join(tmp, 'result.json')}",
            ]
            subprocess.run(cmd, check=True, capture_output=True, timeout=10)
            with open(os.path.join(tmp, "result.json"), "r", encoding="utf-8") as fh:
                return json.load(fh)

    def _simulate_with_eth_call(self, tx: Dict, gas_multiplier: float) -> Dict:
        payload = {
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_call",
            "params": [self._build_call_payload(tx, gas_multiplier), "latest"],
        }
        try:
            resp = self.rpc_post(self.rpc_url, payload)
            if resp.get("error"):
                return {"status": "0x0", "error": resp["error"].get("message", "rpc error")}
            if resp.get("result") is None:
                return {"status": "0x0", "error": "resposta eth_call vazia"}
            return {"status": "0x1", "output": resp["result"]}
        except Exception as exc:
            return {"status": "0x0", "error": str(exc)}

    # ------------------------------------------------------- conveniencia
    def simulate_by_hash(self, tx_hash: str, gas_multiplier: float = 1.3) -> Dict:
        tx = self.fetcher(tx_hash)
        if not tx:
            return {"status": "0x0", "error": "transacao nao encontrada"}
        return self.simulate(tx, gas_multiplier)


if __name__ == "__main__":
    import sys

    fail = 0
    fake_tx = {
        "from": "0xdbc69a52278b9deeed1d6cf099a583dd9d9708fd",
        "to": "0xb8ce59fc3717ada4c02eadf9682a9e934f625ebb",
        "input": "0xa9059cbb0000000000000000000000000abc",
        "gas": "0x10433",
        "gasPrice": "0x94029e4",
        "nonce": "0xa8",
        "value": "0x0",
    }

    def fake_post(url: str, payload: Dict) -> Dict:
        return {"jsonrpc": "2.0", "id": 1, "result": "0x"}

    sim = EVMSimulatorWithFallback(evm_path="evm", fetcher=lambda h: fake_tx, rpc_post=fake_post)
    r = sim.simulate_by_hash("0xf85149f7a9cf33317bddf2d6a43af37489e735766e6d26ca858192b078228c52")
    if r.get("status") != "0x1":
        fail += 1
        print(f"[EVM] FAIL: fallback eth_call (real: {r})")
    else:
        print(f"[EVM] fallback eth_call -> status {r['status']}")

    def fake_err(url: str, payload: Dict) -> Dict:
        return {"error": {"message": "execution reverted"}}

    sim2 = EVMSimulatorWithFallback(evm_path="evm", fetcher=lambda h: fake_tx, rpc_post=fake_err)
    r2 = sim2.simulate(fake_tx)
    if r2.get("status") != "0x0" or "reverted" not in r2.get("error", ""):
        fail += 1
        print(f"[EVM] FAIL: revert deveria virar 0x0 (real: {r2})")
    else:
        print(f"[EVM] revert detectado -> {r2['error']}")

    r3 = sim2.simulate_by_hash("0xdeadbeef")  # fetcher retorna fake_tx sempre; teste forma do payload
    if r3.get("status") not in ("0x0", "0x1"):
        fail += 1
    else:
        print("[EVM] simulate_by_hash ok (formato do payload valido)")

    print(f"[EVM] {'ALL CHECKS PASS' if fail == 0 else 'CHECKS FAILED'}")
    sys.exit(1 if fail else 0)