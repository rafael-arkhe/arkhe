# validation/lean_validator.py
"""
L3 — Validacao em tempo real dos invariantes (v280.0).

Mirror Python dos invariantes formalizados em `invariants/ConsolidationInvariants.lean`
com os mesmos limiares:

  * I238  Monotonicidade da estimativa de coerencia;
  * I239  Convergencia da estimativa para phi_star;
  * I240  Sincronia warp <-> phi_star;
  * I241  Recompensa limitada (|delta_reward| <= 0.5);
  * I255  Ledger imutavel e encadeado (integro);
  * I256  Amostragem priorizada com prioridades positivas;
  * I257  Dashboard leve (cpu < 5%, mem < 100 MB).

O verificador Lean (quando instalado) checa o arquivo formal real via
`verify_with_lean`; sem `lean`, o status e registrado como `unavailable` —
nunca falso-verde.
"""

from __future__ import annotations

import logging
import os
import subprocess
from dataclasses import dataclass, field
from enum import Enum
from typing import Dict, List, Tuple

logger = logging.getLogger(__name__)


class InvariantStatus(Enum):
    VERIFIED = "verified"
    VIOLATED = "violated"
    PENDING = "pending"
    UNAVAILABLE = "unavailable"


@dataclass
class VerificationReport:
    checks: List[Tuple[str, InvariantStatus, str]] = field(default_factory=list)

    @property
    def all_verified(self) -> bool:
        return all(s == InvariantStatus.VERIFIED for _, s, _ in self.checks)

    @property
    def any_violated(self) -> bool:
        return any(s == InvariantStatus.VIOLATED for _, s, _ in self.checks)

    def to_dict(self) -> Dict:
        return {
            label: {"status": status.value, "detail": detail}
            for label, status, detail in self.checks
        }


class MiniLeanValidator:
    """Checador de invariantes em tempo real (L3)."""

    REWARD_LIMIT = 0.5
    WARP_TOL = 0.05
    CONV_WINDOW = 20
    CONV_TOL = 0.01
    CPU_LIMIT_MB = 5.0
    MEM_LIMIT_MB = 100.0

    def __init__(self, theorem_file: str = "invariants/ConsolidationInvariants.lean"):
        self.theorem_file = theorem_file

    # ------------------------------------------------------------------ #
    def check_i238(self, history: List[float], tol: float = 1e-6) -> InvariantStatus:
        if len(history) < 2:
            return InvariantStatus.PENDING
        for i in range(len(history) - 1):
            if history[i + 1] < history[i] - tol:
                return InvariantStatus.VIOLATED
        return InvariantStatus.VERIFIED

    def check_i239(self, history: List[float], phi_star: float,
                   window: int = CONV_WINDOW, tol: float = CONV_TOL) -> InvariantStatus:
        if len(history) < window:
            return InvariantStatus.PENDING
        import numpy as np

        last = history[-window:]
        mean = float(np.mean(last))
        variance = float(np.var(last))
        if variance < tol and abs(mean - phi_star) < tol:
            return InvariantStatus.VERIFIED
        # Ainda convergindo — nao e violacao, mas tambem nao e claim.
        return InvariantStatus.PENDING

    def check_i240(self, warp: float, phi_star: float, tol: float = WARP_TOL) -> InvariantStatus:
        if abs(warp - phi_star) < tol:
            return InvariantStatus.VERIFIED
        return InvariantStatus.VIOLATED

    def check_i241(self, delta_reward: float, limit: float = REWARD_LIMIT) -> InvariantStatus:
        if abs(delta_reward) <= limit:
            return InvariantStatus.VERIFIED
        return InvariantStatus.VIOLATED

    def check_i255(self, cid: str, chain_ok: bool) -> InvariantStatus:
        valid_cid = chain_ok and bool(cid) and isinstance(cid, str) and cid != "GENESIS"
        if valid_cid and _valid_hash(cid):
            return InvariantStatus.VERIFIED
        if not chain_ok:
            return InvariantStatus.VIOLATED
        return InvariantStatus.PENDING

    def check_i256(self, replay_size: int, mean_priority: float) -> InvariantStatus:
        if replay_size <= 0:
            return InvariantStatus.PENDING
        if mean_priority > 0:
            return InvariantStatus.VERIFIED
        return InvariantStatus.VIOLATED

    def check_i257(self, cpu_usage: float, memory_usage: float,
                   cpu_limit: float = CPU_LIMIT_MB, mem_limit: float = MEM_LIMIT_MB) -> InvariantStatus:
        if cpu_usage < cpu_limit and memory_usage < mem_limit:
            return InvariantStatus.VERIFIED
        return InvariantStatus.VIOLATED

    # ------------------------------------------------------------------ #
    def verify(self, state: Dict[str, float]) -> VerificationReport:
        history = state.get("phi_history", [])
        phi_star = state.get("phi_star", 0.85)
        warp = state.get("warp", 0.0)
        delta_reward = state.get("delta_reward", 0.0)
        cid = state.get("ipfs_cid", "")
        chain_ok = bool(state.get("chain_ok", True))
        replay_size = int(state.get("replay_size", 0))
        mean_priority = float(state.get("replay_mean_priority", 0.0))
        cpu = float(state.get("cpu_usage", 0.0))
        mem = float(state.get("memory_usage", 0.0))

        report = VerificationReport(
            checks=[
                ("I238", self.check_i238(history), "phi_history"),
                ("I239", self.check_i239(history, phi_star), "convergencia"),
                ("I240", self.check_i240(warp, phi_star), "warp sync"),
                ("I241", self.check_i241(delta_reward), "reward bounded"),
                ("I255", self.check_i255(cid, chain_ok), "ledger immutavel"),
                ("I256", self.check_i256(replay_size, mean_priority), "PER bias"),
                ("I257", self.check_i257(cpu, mem), "dashboard leve"),
            ]
        )
        return report

    # ------------------------------------------------------------------ #
    def verify_with_lean(self, timeout: int = 60, lean_bin: str = "lean") -> Tuple[InvariantStatus, str]:
        """Checa o arquivo formal com o verificador Lean 4 (se instalado)."""
        path = self.theorem_file
        if not os.path.exists(path):
            return InvariantStatus.UNAVAILABLE, f"teorema ausente: {path}"
        try:
            proc = subprocess.run(
                [lean_bin, path],
                capture_output=True,
                text=True,
                timeout=timeout,
                cwd=os.path.dirname(path) or ".",
            )
        except FileNotFoundError:
            return InvariantStatus.UNAVAILABLE, "lean nao instalado no PATH"
        except subprocess.TimeoutExpired:
            return InvariantStatus.UNAVAILABLE, "timeout na verificacao"
        if proc.returncode == 0:
            return InvariantStatus.VERIFIED, "ConsolidationInvariants.lean compilado"
        summary = (proc.stderr or proc.stdout or "").strip().splitlines()
        return InvariantStatus.VIOLATED, "; ".join(summary[-3:])


def _valid_hash(cid: str) -> bool:
    """CID valido: ponto Qm... (IPFS) ou hex SHA-256 de 64 chars (local)."""
    if len(cid) == 46 and cid.startswith("Qm"):
        return True
    if len(cid) == 64 and all(c in "0123456789abcdef" for c in cid.lower()):
        return True
    return False