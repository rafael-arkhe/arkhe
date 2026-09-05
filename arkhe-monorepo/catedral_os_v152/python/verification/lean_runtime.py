# python/verification/lean_runtime.py
"""
Verificacao Lean embarcada (I200) — checagem de invariantes em tempo real.
Mirror Python do verificador Rust com os mesmos limiares.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from enum import Enum
from typing import List, Tuple

from core.entropy import constructive_entropy


class InvariantStatus(Enum):
    VERIFIED = "verified"
    VIOLATED = "violated"
    PENDING = "pending"


@dataclass
class VerificationReport:
    checks: List[Tuple[str, InvariantStatus, float]] = field(default_factory=list)

    @property
    def all_verified(self) -> bool:
        return all(s == InvariantStatus.VERIFIED for _, s, _ in self.checks)


class LeanVerifier:
    """Checador de invariantes em tempo real (I200)."""

    def __init__(self) -> None:
        self._violations: List[Tuple[str, float]] = []

    # ------------------------------------------------------------------ #
    def check_i157(self, phi_irr: float) -> InvariantStatus:
        if not (0.0 <= phi_irr <= 1.0):
            self._violations.append(("I157", phi_irr))
            return InvariantStatus.VIOLATED
        return InvariantStatus.VERIFIED

    def check_i158(self, phi_crit: float) -> InvariantStatus:
        if not (0.0 <= phi_crit <= 1.0):
            self._violations.append(("I158", phi_crit))
            return InvariantStatus.VIOLATED
        return InvariantStatus.VERIFIED

    def check_i159(self, anti_flatness: float) -> InvariantStatus:
        if anti_flatness < 0.0:
            self._violations.append(("I159", anti_flatness))
            return InvariantStatus.VIOLATED
        return InvariantStatus.VERIFIED

    def check_i167(self, spectrum: List[float], n_total: int,
                   n_handovers: int) -> InvariantStatus:
        entropy = constructive_entropy(spectrum, n_total, n_handovers)
        if entropy < 0.0:
            self._violations.append(("I167", entropy))
            return InvariantStatus.VIOLATED
        return InvariantStatus.VERIFIED

    def check_i194(self, dt_us: int, ranging_us: int) -> InvariantStatus:
        if dt_us > ranging_us * 2:
            self._violations.append(("I194", float(dt_us)))
            return InvariantStatus.VIOLATED
        return InvariantStatus.VERIFIED

    def check_i193(self, phi: float, energy: float) -> InvariantStatus:
        if energy <= 0.0:
            return InvariantStatus.VERIFIED
        eff = phi / energy
        if eff < 1e14:
            self._violations.append(("I193", eff))
            return InvariantStatus.VIOLATED
        return InvariantStatus.VERIFIED

    def check_i201(self, phi_local: float, phi_remote: float) -> InvariantStatus:
        if abs(phi_local - phi_remote) >= 0.1:
            self._violations.append(("I201", abs(phi_local - phi_remote)))
            return InvariantStatus.VIOLATED
        return InvariantStatus.VERIFIED

    # ------------------------------------------------------------------ #
    def verify_all(self, spectrum: List[float], n_total: int, n_handovers: int,
                   phi_irr: float, phi_crit: float, anti_flatness: float,
                   dt_us: int, ranging_us: int,
                   phi_local: float, phi_remote: float,
                   energy: float = 1e-15) -> VerificationReport:
        checks = [
            ("I157", self.check_i157(phi_irr), 0.0),
            ("I158", self.check_i158(phi_crit), 0.0),
            ("I159", self.check_i159(anti_flatness), 0.0),
            ("I167", self.check_i167(spectrum, n_total, n_handovers), 0.0),
            ("I194", self.check_i194(dt_us, ranging_us), 0.0),
            ("I193", self.check_i193(phi_local, energy), 0.0),
            ("I201", self.check_i201(phi_local, phi_remote), 0.0),
        ]
        return VerificationReport(checks=checks)

    @property
    def violations(self) -> List[Tuple[str, float]]:
        return list(self._violations)