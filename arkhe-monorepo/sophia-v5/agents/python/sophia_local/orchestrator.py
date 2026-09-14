"""Orquestrador central de agentes locais (Python).

Executa os patches #19–22 em loop (thread), faz a ancoragem na TemporalChain,
aciona o agente linguístico e expõe um snapshot de estado consumido pelo
dashboard via socket local.
"""
from __future__ import annotations

import json
import threading
import time
from dataclasses import dataclass, field
from pathlib import Path
from typing import Dict, List, Optional

from .patches import SophiaPatches
from .temporal_bridge import TemporalBridge
from .linguistic_agent import LinguisticAgent

SAMPLE_TWIN_RESPONSES: List[float] = [0.5, 0.6, 0.44, 0.51, 0.57, 0.49, 0.53, 0.5]


@dataclass
class LocalOrchestrator:
    """Orquestra agentes locais sem comunicação externa."""

    loop_hz: int = 10
    state_path: Path = field(default_factory=lambda: Path("state.json"))
    ledger_path: Path = field(default_factory=lambda: Path("ledger.jsonl"))
    twin_responses: List[float] = field(default_factory=list)

    def __post_init__(self) -> None:
        self.patches = SophiaPatches(seed=42)
        self.bridge = TemporalBridge(self.ledger_path)
        self.linguistic = LinguisticAgent()
        self.running = False
        self._thread: Optional[threading.Thread] = None
        self._lock = threading.Lock()
        self.reset_state_metrics()

    def reset_state_metrics(self) -> Dict[str, float]:
        self.metrics = self.patches.metrics()
        self.last_cycle_seal = ""
        return self.metrics

    def run_full_cycle(self, *,
                       detections: Optional[List[str]] = None,
                       anchor: bool = True) -> str:
        """Executa um ciclo completo de todos os agentes (patches + linguística)."""
        self.patches.run_cycle(self.twin_responses or SAMPLE_TWIN_RESPONSES)
        self.metrics = self.patches.metrics()

        seen = sorted({d for d in (detections or []) if d})
        if seen:
            self.linguistic.run_iteration(psi_c=self.metrics["psi_C"],
                                          q=self.metrics["q_coefficient"],
                                          detections=seen)

        seal = ""
        if anchor:
            seal = self.bridge.anchor_cycle(self.metrics)
            self.last_cycle_seal = seal
        self._write_state()
        return seal

    def _write_state(self) -> None:
        with self._lock:
            self.state_path.write_text(json.dumps({
                "metrics": self.metrics,
                "last_cycle_seal": self.last_cycle_seal,
                "ledger_entries": self.bridge.count(),
                "ledger_verified": self.bridge.verify()[0],
                "ts": time.time(),
            }, sort_keys=True), encoding="utf-8")

    def start(self) -> None:
        if self.running:
            return
        self.running = True

        def loop() -> None:
            while self.running:
                self.run_full_cycle()
                time.sleep(1.0 / self.loop_hz)

        self._thread = threading.Thread(target=loop, daemon=True)
        self._thread.start()

    def stop(self) -> None:
        self.running = False
        if self._thread:
            self._thread.join(timeout=2.0)
            self._thread = None

    def status(self) -> Dict[str, object]:
        with self._lock:
            return {
                "orchestrator": {**self.metrics,
                                 "running": self.running,
                                 "loop_hz": self.loop_hz},
                "temporal": {
                    "ledger_entries": self.bridge.count(),
                    "ledger_verified": self.bridge.verify()[0],
                    "last_seal": self.last_cycle_seal,
                },
            }


def run_cli() -> None:
    import argparse
    import signal
    import time

    parser = argparse.ArgumentParser(prog="sophia_orchestrator")
    parser.add_argument("--daemon", action="store_true", help="roda continuamente (systemd)")
    parser.add_argument("--cycles", type=int, default=1, help="ciclos antes de parar (test)")
    parser.add_argument("--hz", type=int, default=10)
    args = parser.parse_args()

    orch = LocalOrchestrator(loop_hz=args.hz)

    def _stop(_sig, _frame) -> None:
        orch.stop()
        raise SystemExit(0)

    if args.daemon:
        signal.signal(signal.SIGTERM, _stop)
        signal.signal(signal.SIGINT, _stop)
        print(f"[Sophia] orquestrador local (10 Hz) — ledger={orch.bridge.ledger_path}")
        orch.start()
        while True:
            time.sleep(1)
    else:
        for _ in range(max(1, args.cycles)):
            orch.run_full_cycle()
        print(orch.status())


if __name__ == "__main__":
    run_cli()