"""TemporalChain bridge local — ancoragem append-only com hash SHA3-256.

Garante Loopseal-1 (ancoragem temporal) e Loopseal-2 (prova imutável) sem
qualquer dependência de rede. O ledger é um arquivo JSONL local:
cada registro referencia o hash do anterior (cadeia) e assina o payload.
"""
from __future__ import annotations

import hashlib
import json
import threading
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Dict, List, Optional

GENESIS = "0" * 64


class TemporalBridge:
    """Ledger local derivado do TemporalChainAnchor (substrato 923)."""

    def __init__(self, ledger_path: Path) -> None:
        self.ledger_path = Path(ledger_path)
        self._lock = threading.Lock()
        self.ledger_path.parent.mkdir(parents=True, exist_ok=True)

    @staticmethod
    def _hash(payload: Dict[str, Any]) -> str:
        return hashlib.sha3_256(
            json.dumps(payload, sort_keys=True).encode("utf-8")
        ).hexdigest()

    @staticmethod
    def _now() -> str:
        return datetime.now(timezone.utc).isoformat()

    def _last_hash(self) -> str:
        if not self.ledger_path.exists() or self.ledger_path.stat().st_size == 0:
            return GENESIS
        line = self.ledger_path.read_text(encoding="utf-8").strip().splitlines()[-1]
        try:
            return json.loads(line).get("seal", GENESIS)
        except json.JSONDecodeError:
            return GENESIS

    def anchor(self, event: str, data: Dict[str, Any]) -> str:
        """Ancora um evento na cadeia local. Append-only, imutável e auditável."""
        with self._lock:
            previous = self._last_hash()
            record = {
                "event": event,
                "timestamp": self._now(),
                "data": data,
                "previous": previous,
            }
            seal = self._hash({"event": event, "timestamp": record["timestamp"],
                               "data": data, "previous": previous})
            record["seal"] = seal
            with self.ledger_path.open("a", encoding="utf-8") as fh:
                fh.write(json.dumps(record, sort_keys=True) + "\n")
        return seal

    def tail(self, n: int = 20) -> List[Dict[str, Any]]:
        if not self.ledger_path.exists():
            return []
        lines = self.ledger_path.read_text(encoding="utf-8").strip().splitlines()
        return [json.loads(l) for l in lines[-n:]]

    def verify(self) -> tuple[bool, int]:
        """Verifica integridade da cadeia do gênese até o último bloco."""
        if not self.ledger_path.exists():
            return False, 0
        previous = GENESIS
        count = 0
        for line in self.ledger_path.read_text(encoding="utf-8").strip().splitlines():
            rec = json.loads(line)
            if rec.get("previous") != previous:
                return False, count
            expected = self._hash({"event": rec["event"], "timestamp": rec["timestamp"],
                                   "data": rec["data"], "previous": rec["previous"]})
            if rec.get("seal") != expected:
                return False, count
            previous = rec["seal"]
            count += 1
        return True, count

    def count(self) -> int:
        if not self.ledger_path.exists():
            return 0
        return len(self.ledger_path.read_text(encoding="utf-8").strip().splitlines())

    def anchor_cycle(self, metrics: Dict[str, float]) -> str:
        """Ancora as métricas de um ciclo de agentes na TemporalChain."""
        return self.anchor("sophia_cycle", metrics)


def load_bridge(path: Optional[str] = None) -> TemporalBridge:
    import tempfile
    if path:
        return TemporalBridge(Path(path))
    return TemporalBridge(Path(tempfile.gettempdir()) / "sophia_ledger.jsonl")