# python/adaptive/governance_adaptive.py
"""
Governanca adaptativa (I199) — ajusta parametros da triade com base em
metricas de performance coletadas a cada ciclo de governanca.

  - Se a coerencia total fica abaixo de 80% do alvo, acelera a inferencia.
  - Se a eficiencia energetica cai, aumenta o decaimento (esquece mais).
"""

from __future__ import annotations

import threading
import time
from collections import deque
from dataclasses import dataclass
from typing import Deque, Optional


@dataclass
class GovernanceMetrics:
    perception_rate: float
    inference_rate: float
    proof_rate: float
    undulator_rate: float
    total_coherence: float
    energy_efficiency: float


class AdaptiveGovernance:
    """Ajusta parametros da triade com base nas metricas coletadas."""

    def __init__(self, chain, integrator) -> None:
        self.chain = chain
        self.integrator = integrator
        self._history: Deque[GovernanceMetrics] = deque(maxlen=100)
        self._lock = threading.RLock()
        self._running = False
        self._thread: Optional[threading.Thread] = None

        # Parametros ajustaveis
        self.target_phi = 0.95
        self.decay_rate = 0.001
        self.inference_interval = 0.1
        self.proof_interval = 1.0

    def start(self, period: float = 5.0) -> None:
        if self._running:
            return
        self._running = True
        self._thread = threading.Thread(target=self._loop, args=(period,), daemon=True)
        self._thread.start()

    def stop(self) -> None:
        self._running = False
        if self._thread is not None:
            self._thread.join(timeout=1.0)

    # ------------------------------------------------------------------ #
    def _loop(self, period: float) -> None:
        while self._running:
            time.sleep(period)
            with self._lock:
                metrics = self._collect_metrics()
                self._adjust_parameters(metrics)
                self._history.append(metrics)

    def _collect_metrics(self) -> GovernanceMetrics:
        proof = self.chain.to_proof()
        return GovernanceMetrics(
            perception_rate=self.integrator.stats["perception_count"] / 10.0,
            inference_rate=self.integrator.stats["inference_count"] / 10.0,
            proof_rate=self.integrator.stats["proof_count"] / 10.0,
            undulator_rate=self.integrator.stats["undulator_count"] / 10.0,
            total_coherence=proof.get("total", 0.0),
            energy_efficiency=proof.get("total", 0.0) / 1e-15,
        )

    def _adjust_parameters(self, metrics: GovernanceMetrics) -> None:
        if metrics.total_coherence < self.target_phi * 0.8:
            self.inference_interval = max(0.05, self.inference_interval * 0.9)
        elif metrics.total_coherence > self.target_phi * 1.1:
            self.inference_interval = min(0.5, self.inference_interval * 1.1)

        if metrics.energy_efficiency < 1e14:
            self.decay_rate = min(0.01, self.decay_rate * 1.05)
        else:
            self.decay_rate = max(0.0001, self.decay_rate * 0.95)

    def report(self) -> dict:
        with self._lock:
            return {
                "target_phi": self.target_phi,
                "decay_rate": self.decay_rate,
                "inference_interval": self.inference_interval,
                "proof_interval": self.proof_interval,
                "samples": len(self._history),
            }