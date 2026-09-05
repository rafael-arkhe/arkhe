# python/hardware/undulator.py
"""
Nó Undulator — decaimento físico com Zeno Veto (I194.I197) e adaptativo (I198).
Mirror Python do crate Rust `arkhe-catedral-v152`.

  I194  Zeno Veto: dt <= 2 * ranging
  I195  Ranging Delta (microssegundos da linha)
  I196  Decaimento exponencial: Phi(t) = Phi0 * exp(-lambda * t)
  I198  lambda(t) = lambda_0 + beta * (1 - f_handover), clamp em [min,max]
"""

from __future__ import annotations

import math
import struct
import time
from dataclasses import dataclass
from typing import List, Optional

from core.spectral import irreducible_coherence


@dataclass(frozen=True)
class RangingDelta:
    """I195: ranging da linha em microssegundos."""
    us: int

    def as_secs(self) -> float:
        return self.us * 1e-6

    def zeno_threshold(self) -> int:
        """I194: limiar de Zeno = 2 * ranging."""
        return self.us * 2


@dataclass(frozen=True)
class DecayRate:
    """I196: taxa de decaimento (1/tau)."""
    value: float

    @classmethod
    def from_ranging(cls, ranging: RangingDelta) -> "DecayRate":
        tau = ranging.as_secs()
        return cls(1.0 / tau if tau > 0.0 else 1.0)

    def apply(self, phi: float, dt_secs: float) -> float:
        return phi * math.exp(-self.value * dt_secs)


@dataclass
class UndulatorHandover:
    phi: float
    source: str
    timestamp: float
    ranging_delta_us: int
    zeno_passed: bool
    decayed_phi: float


@dataclass
class AdaptiveParams:
    """I198: parametros de adaptacao do decaimento."""
    initial_decay: float = 0.001
    adaptation_rate: float = 0.0005
    min_decay: float = 0.0001
    max_decay: float = 0.01


class UndulatorNode:
    """Nó Undulator com Zeno Veto (I194) e decaimento adaptativo (I198)."""

    def __init__(self, ranging: RangingDelta,
                 adaptive: Optional[AdaptiveParams] = None):
        self.ranging = ranging
        self.adaptive = adaptive or AdaptiveParams()
        self.decay_rate = DecayRate.from_ranging(ranging) if adaptive is None \
            else DecayRate(self.adaptive.initial_decay)
        self._last_handover_ts = time.monotonic()
        self._handover_times: List[float] = []
        self._count = 0
        self._phi_received = 0.0
        self._phi_accepted = 0.0

    # ------------------------------------------------------------------ #
    # Frequencia / adaptacao (I198)
    # ------------------------------------------------------------------ #
    def _update_decay_rate(self) -> None:
        if self.adaptive.adaptation_rate <= 0.0:
            return
        now = time.monotonic()
        window = 10.0  # segundos
        self._handover_times = [t for t in self._handover_times if t >= now - window]
        freq = len(self._handover_times) / window
        raw = (self.adaptive.initial_decay
               + self.adaptive.adaptation_rate * (1.0 - min(freq, 1.0)))
        self.decay_rate = DecayRate(
            max(self.adaptive.min_decay, min(self.adaptive.max_decay, raw)))

    # ------------------------------------------------------------------ #
    # Handover (I194)
    # ------------------------------------------------------------------ #
    def receive_handover(self, phi: float, source: str = "Z1T") -> Optional[UndulatorHandover]:
        now = time.monotonic()
        dt = now - self._last_handover_ts
        dt_us = int(dt * 1e6)
        dt_secs = dt

        zeno_ok = dt_us <= self.ranging.zeno_threshold()
        if not zeno_ok:
            return None

        self._handover_times.append(now)
        self._update_decay_rate()

        decayed = self.decay_rate.apply(phi, dt_secs)
        self._last_handover_ts = now
        self._count += 1
        self._phi_received += phi
        self._phi_accepted += decayed

        return UndulatorHandover(
            phi=phi,
            source=source,
            timestamp=time.time(),
            ranging_delta_us=self.ranging.us,
            zeno_passed=True,
            decayed_phi=decayed,
        )

    # ------------------------------------------------------------------ #
    # Estadisticas
    # ------------------------------------------------------------------ #
    def stats(self) -> dict:
        return {
            "handover_count": self._count,
            "total_phi_received": self._phi_received,
            "total_phi_accepted": self._phi_accepted,
            "acceptance_rate": (self._phi_accepted / self._phi_received
                                if self._phi_received > 0 else 0.0),
            "ranging_us": self.ranging.us,
            "decay_rate": self.decay_rate.value,
        }

    def encode_packet(self, phi: float, source: int = 0) -> bytes:
        """Empacota um handover para o barramento UART (payload G1)."""
        ts_us = int(time.time() * 1e6)
        return struct.pack("<Bfq", source & 0xFF, phi, ts_us)

    @staticmethod
    def phi_reference() -> float:
        """I157 aplicado ao nó: referencia quasi-universal (N=13)."""
        return irreducible_coherence(13)