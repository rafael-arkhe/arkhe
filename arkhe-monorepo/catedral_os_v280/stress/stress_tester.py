# stress/stress_tester.py
"""
L6 — Teste de estresse continuo com injecao de falhas (v280.0).

Prova a resiliencia do sistema: N workers disparam a funcao alvo sob carga,
com injecao de falhas (excecoes aleatorias e dados malformados) e relatorio
aggregado por worker. `target` e tipicamente um passo do orquestrador; o
`report` retorna taxa de sucesso global e por worker.
"""

from __future__ import annotations

import logging
import random
import time
from concurrent.futures import ThreadPoolExecutor
from typing import Callable, Dict, List, Optional

logger = logging.getLogger(__name__)


class StressTester:
    """Injeta falhas e carga pesada na funcao alvo."""

    def __init__(self, target_function: Callable[[], bool],
                 num_workers: int = 10, seed: Optional[int] = None,
                 fault_probability: float = 0.05):
        self.target = target_function
        self.num_workers = num_workers
        self.rng = random.Random(seed)
        self.base_fault_probability = fault_probability
        self.fault_probability = fault_probability
        self.fault_injection_enabled = True if fault_probability > 0 else False
        self.running = False
        self.results: List[Dict] = []

    def inject_faults(self, enable: bool = True) -> None:
        self.fault_injection_enabled = enable
        self.fault_probability = self.base_fault_probability if enable else 0.0
        logger.info("Injecao de falhas %s", "ativada" if enable else "desativada")

    # ------------------------------------------------------------------ #
    def start(self, duration: int = 10, calls_per_worker: Optional[int] = None) -> Dict:
        """Executa o estresse (por segundos ou por numero de chamadas)."""
        self.running = True
        self.results = []
        logger.info(
            "Teste de estresse: %d workers, falhas=%.0f%%",
            self.num_workers,
            self.fault_probability * 100,
        )

        def _worker_loop(worker_id: int) -> Dict:
            successes = 0
            failures = 0
            start = time.time()
            while self.running:
                if calls_per_worker is not None and successes + failures >= calls_per_worker:
                    break
                if duration and time.time() - start >= duration:
                    break
                try:
                    if self.fault_probability > 0 and self.rng.random() < self.fault_probability:
                        raise RuntimeError(f"falha injetada no worker {worker_id}")
                    if self.target():
                        successes += 1
                except Exception:
                    failures += 1
            return {
                "worker_id": worker_id,
                "successes": successes,
                "failures": failures,
                "success_rate": successes / (successes + failures)
                if (successes + failures) else 0.0,
            }

        with ThreadPoolExecutor(max_workers=self.num_workers) as executor:
            self.results = list(executor.map(_worker_loop, range(self.num_workers)))
        self.running = False
        logger.info("Teste de estresse concluido.")
        return self.get_report()

    # ------------------------------------------------------------------ #
    def get_report(self) -> Dict:
        total_successes = sum(r["successes"] for r in self.results)
        total_failures = sum(r["failures"] for r in self.results)
        total = total_successes + total_failures
        return {
            "total_workers": self.num_workers,
            "total_successes": total_successes,
            "total_failures": total_failures,
            "overall_success_rate": total_successes / total if total > 0 else 0.0,
            "workers_report": self.results,
        }