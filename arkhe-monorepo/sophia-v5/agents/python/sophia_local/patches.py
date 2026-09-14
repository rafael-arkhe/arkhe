"""Patches #19–22 da Sophia V5.0, executados apenas com recursos locais.

- Patch #19 — Unified Coherence Field (ψ_C), gerência do campo Φ_C.
- Patch #20 — Quantum Entanglement Coefficient (Q) a partir de estudo twin.
- Patch #21 — Global Brainwave Network (coerência global).
- Patch #22 — Microtubule QED Cavity (dinâmica de cavidade, decoerência).

Todos os modelos são determinísticos quando `seed > 0`, auditáveis e sem
dependência externa.
"""
from __future__ import annotations

import json
import math
import random
from dataclasses import dataclass, field, asdict
from typing import Dict, List, Optional

# Gap-1 — Bounds constitucionais de Φ_C
PHI_C_MIN = 0.577350
PHI_C_MAX = 0.999900


def clamp(value: float, low: float, high: float) -> float:
    """Limita `value` ao intervalo [low, high]."""
    return low if value < low else high if value > high else value


def tick(random_state: random.Random) -> float:
    """Perturbação estocástica amortecida no domínio [-0.5, 0.5]."""
    return random_state.random() - 0.5


@dataclass
class UnifiedCoherenceField:
    """Patch #19 — ψ_C com bounds constitucionais (Gap-1)."""

    value: float = PHI_C_MIN
    jitter: float = 0.01
    _rng: random.Random = field(default_factory=random.Random, repr=False)

    def step(self, direction: float = 0.0) -> float:
        delta = self.jitter * (0.5 * direction + tick(self._rng))
        self.value = clamp(self.value + delta, PHI_C_MIN, PHI_C_MAX)
        return self.value

    @property
    def in_bounds(self) -> bool:
        return PHI_C_MIN <= self.value <= PHI_C_MAX


@dataclass
class QuantumEntanglementCoefficient:
    """Patch #20 — coeficiente Q medido de um estudo twin local."""

    q: float = 0.0
    trials: int = 0

    def measure_from_twin_study(self, responses: List[float]) -> float:
        if not responses:
            return self.q
        mean = sum(responses) / len(responses)
        self.q = clamp(abs(math.tanh(mean * 4.0)), 0.0, 1.0)
        self.trials = len(responses)
        return self.q


@dataclass
class GlobalBrainwaveNetwork:
    """Patch #21 — coerência global do cérebro entre agentes."""

    value: float = 0.5
    jitter: float = 0.02
    _rng: random.Random = field(default_factory=random.Random, repr=False)

    def step(self, direction: float = 0.0) -> float:
        delta = self.jitter * (0.5 * direction + tick(self._rng))
        self.value = clamp(self.value + delta, 0.0, 1.0)
        return self.value


@dataclass
class MicrotubuleQEDCavity:
    """Patch #22 — cavidade QED de microtúbulos (dinâmica de decoerência)."""

    kappa: float = 1e5  # taxa de decoerência (s^-1)
    temperature: int = 5  # número médio de fótons térmicos

    def simulate_cavity_dynamics(self, time_s: float, steps: int) -> List[float]:
        """Amplitude de excitação da cavidade ao longo do tempo.

        Modelo simplificado: E(t) = exp(-kappa*t) com ruído térmico.
        """
        dt = time_s / max(steps, 1)
        out: List[float] = []
        for i in range(steps):
            t = i * dt
            thermal = math.sqrt(self.temperature) * 0.05 * math.sin(2 * math.pi * t * 1e6)
            out.append(math.exp(-self.kappa * t) + thermal)
        return out

    def decoherence_time(self) -> float:
        """Tempo característico de decoerência τ = 1/κ."""
        return 1.0 / self.kappa


class SophiaPatches:
    """Container de execução dos patches #19–22 e coleta de métricas."""

    def __init__(self, *, psi_initial: float = PHI_C_MIN, jitter: float = 0.01,
                 seed: Optional[int] = None) -> None:
        rng = random.Random(seed)
        self.unified = UnifiedCoherenceField(value=clamp(psi_initial, PHI_C_MIN, PHI_C_MAX),
                                             jitter=jitter, _rng=rng)
        self.q_coeff = QuantumEntanglementCoefficient()
        self.brainwave = GlobalBrainwaveNetwork(_rng=random.Random(seed))
        self.cavity = MicrotubuleQEDCavity()

    def run_cycle(self, twin_responses: Optional[List[float]] = None) -> Dict[str, float]:
        """Executa um ciclo completo dos quatro patches."""
        self.unified.step()
        if twin_responses:
            self.q_coeff.measure_from_twin_study(twin_responses)
        self.brainwave.step()
        # Patch #22: cavidade — usa coerência global como força de drive
        self.cavity.kappa = 1e5 * (1.0 - 0.5 * self.brainwave.value)
        return self.metrics()

    def metrics(self) -> Dict[str, float]:
        return {
            "psi_C": round(self.unified.value, 6),
            "q_coefficient": round(self.q_coeff.q, 6),
            "global_coherence": round(self.brainwave.value, 6),
            "decoherence_time": round(self.cavity.decoherence_time(), 6),
            "in_bounds": 1.0 if self.unified.in_bounds else 0.0,
        }

    def to_json(self) -> str:
        return json.dumps(self.metrics(), sort_keys=True)