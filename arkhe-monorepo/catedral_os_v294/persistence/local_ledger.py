# persistence/local_ledger.py
"""
Ledger local com encadeamento por hash (fallback de L1) — v294.0.

Cada entrada é assinada (SHA-256), encadeada por `prev_hash` e persistida
como journal append-only (uma linha JSON por bloco). A integridade da
cadeia inteira pode ser verificada a qualquer momento (`verify_chain`).
Loopseal-2: o journal só cresce — append-only de verdade, sem fingir.
"""

from __future__ import annotations

import hashlib
import json
import logging
import os
import threading
import time
from typing import Dict, List, Optional, Union

logger = logging.getLogger(__name__)

_JOURNAL_VERSION = 1


def sha256_hex(payload: Union[str, bytes]) -> str:
    if isinstance(payload, str):
        payload = payload.encode("utf-8")
    return hashlib.sha256(payload).hexdigest()


class LocalLedger:
    """Ledger hash-encadeado local, thread-safe, journal append-only."""

    GENESIS = "GENESIS"

    def __init__(self, path: Optional[str] = None, persist: bool = True):
        self._lock = threading.RLock()
        self.path = path or "ledger_chain.jsonl"
        self.persist = persist
        self.chain: List[Dict[str, str]] = []
        self.last_hash: str = self.GENESIS
        self._chain_ok = True
        self.alive = False
        self._load()

    # ------------------------------------------------------------------ #
    # Escrita
    # ------------------------------------------------------------------ #
    def append(self, entry: Dict) -> str:
        """Assina, encadeia e anexa (append-only). Retorna o hash."""
        with self._lock:
            record = {
                "timestamp": time.time(),
                "prev_hash": self.last_hash,
                **{k: v for k, v in entry.items()},
            }
            record["hash"] = self._hash_of(record)
            self.chain.append(record)
            self.last_hash = record["hash"]
            self.alive = True
            self._append_line(record)
            logger.debug("Entrada ancorada: %s", record["hash"][:12])
            return record["hash"]

    def _hash_of(self, record: Dict) -> str:
        body = {k: v for k, v in record.items() if k != "hash"}
        return sha256_hex(json.dumps(body, sort_keys=True, ensure_ascii=False))

    def _header(self) -> Dict:
        return {
            "ledger": "catedral-os-v294",
            "version": _JOURNAL_VERSION,
            "last_hash": self.last_hash,
        }

    def _append_line(self, record: Dict) -> None:
        if not self.persist:
            return
        try:
            directory = os.path.dirname(self.path)
            if directory:
                os.makedirs(directory, exist_ok=True)
            first_time = (
                not os.path.exists(self.path) or os.path.getsize(self.path) == 0
            )
            with open(self.path, "a", encoding="utf-8") as f:
                if first_time:
                    f.write(json.dumps(self._header(), ensure_ascii=False) + "\n")
                f.write(json.dumps(record, ensure_ascii=False) + "\n")
        except Exception as exc:  # pragma: no cover - caminho de erro
            logger.warning("Falha ao anexar ledger local: %s", exc)

    # ------------------------------------------------------------------ #
    # Integridade
    # ------------------------------------------------------------------ #
    def verify_chain(self) -> bool:
        """Verifica hashes e encadeamento da cadeia inteira."""
        with self._lock:
            prev = self.GENESIS
            for record in self.chain:
                got = record.get("prev_hash")
                if got != prev:
                    logger.error("Encadeamento quebrado: %s != %s", got, prev)
                    self._chain_ok = False
                    return False
                if record.get("hash") != self._hash_of(record):
                    logger.error("Hash invalido: %s", record.get("hash", "")[:12])
                    self._chain_ok = False
                    return False
                prev = record["hash"]
            self._chain_ok = True
            return True

    def is_chain_ok(self) -> bool:
        return self._chain_ok

    def tamper(self, index: int, key: str, value: object) -> None:
        """(Uso em testes/estresse) corrompe uma entrada da cadeia."""
        with self._lock:
            self.chain[index][key] = value
            self._chain_ok = False

    # ------------------------------------------------------------------ #
    # Persistencia
    # ------------------------------------------------------------------ #
    def _load(self) -> None:
        if not self.persist:
            return
        try:
            if not os.path.exists(self.path):
                return
            with open(self.path, "r", encoding="utf-8") as f:
                lines = f.read().splitlines()
            if not lines:
                return
            header = json.loads(lines[0])
            if header.get("version") != _JOURNAL_VERSION:
                logger.warning("Versao de journal inesperada: %s", header.get("version"))
            self.chain = [json.loads(line) for line in lines[1:]]
            self.last_hash = (
                self.chain[-1]["hash"] if self.chain
                else header.get("last_hash", self.GENESIS)
            )
            logger.info("Ledger local carregado: %d entradas", len(self.chain))
        except Exception as exc:  # pragma: no cover - caminho de erro
            logger.warning("Falha ao carregar ledger local: %s", exc)

    def stats(self) -> Dict:
        with self._lock:
            return {
                "entries": len(self.chain),
                "last_hash": self.last_hash,
                "chain_ok": self.verify_chain(),
                "mode": "local",
                "alive": self.alive,
                "journal": "append-only jsonl",
            }