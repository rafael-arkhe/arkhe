# scheduler/multi_tunnel.py
"""
L4 — Multiplos tuneis paralelos (v280.0).

Cada tunel (worker) roda o proprio Decisor com seus proprios buffer de
replay, compartilhando um ledger (L1) e um validador (L3) comuns. O
`MultiTunnelScheduler.start_all()` dispara os workers em `ThreadPoolExecutor`;
`step_all()` permite stepping deterministico sincrono (usado pelo
orquestrador e pelos testes de integrracao).
"""

from __future__ import annotations

import logging
import threading
import time
from concurrent.futures import ThreadPoolExecutor
from typing import Dict, List, Optional

import numpy as np

logger = logging.getLogger(__name__)


class TunnelWorker:
    """Worker de um tunel de coerencia individual."""

    def __init__(self, tunnel_id: int, decider, state, name: str = "tunnel"):
        self.tunnel_id = tunnel_id
        self.decider = decider
        self.state = state
        self.name = name
        self.running = False
        self.lock = threading.RLock()

    def step_once(self) -> None:
        with self.lock:
            self.state = self.decider.step(self.state)

    def run_loop(self) -> None:
        """Loop do tunel: passa um passo por tick breve."""
        self.running = True
        logger.info("%s %s iniciado.", self.name, self.tunnel_id)
        try:
            while self.running:
                self.step_once()
                time.sleep(0.005)
        finally:
            self.running = False

    def stop(self) -> None:
        self.running = False
        logger.info("%s %s parado.", self.name, self.tunnel_id)


class MultiTunnelScheduler:
    """Gerencia N tuneis de coerencia em paralelo."""

    def __init__(self, num_tunnels: int = 4, seed: Optional[int] = None):
        self.num_tunnels = num_tunnels
        self.workers: List[TunnelWorker] = []
        self.executor: Optional[ThreadPoolExecutor] = None
        self.seed = seed

    def add_worker(self, decider, initial_state) -> int:
        worker = TunnelWorker(
            tunnel_id=len(self.workers),
            decider=decider,
            state=initial_state,
        )
        self.workers.append(worker)
        self.num_tunnels = len(self.workers)
        return worker.tunnel_id

    @property
    def count(self) -> int:
        return len(self.workers)

    # ------------------------------------------------------------------ #
    def start_all(self) -> None:
        """Dispara os workers em threads paralelas."""
        if not self.workers:
            logger.warning("Nenhum tunel para iniciar.")
            return
        self.executor = ThreadPoolExecutor(max_workers=self.count)
        self._futures = [self.executor.submit(w.run_loop) for w in self.workers]

    def stop_all(self) -> None:
        for worker in self.workers:
            worker.stop()
        if self.executor is not None:
            self.executor.shutdown(wait=True)
            self.executor = None

    def step_all(self, n: int = 1) -> None:
        """Stepping sincrono e deterministico (sem threads)."""
        for _ in range(n):
            for worker in self.workers:
                worker.step_once()

    # ------------------------------------------------------------------ #
    def get_states(self) -> List[Dict]:
        return [w.state for w in self.workers]

    def get_summary(self) -> Dict:
        avg_phi = float(np.mean([w.state.phi_total for w in self.workers])) if self.workers else 0.0
        return {
            "num_tunnels": self.count,
            "running": [w.running for w in self.workers],
            "avg_phi": avg_phi,
            "total_actions": sum(w.state.steps for w in self.workers),
            "chain_ok": all(w.state.chain_ok for w in self.workers),
        }