# core/state.py
"""
Estado do sistema (I238, I239, I241) — o contrato compartilhado entre
decider, ledger, validator e scheduler. Um `SystemState` carrega:

  * phi_total        — coerencia consolidada (estimativa monotona, I238);
  * phi_star         — alvo de coerencia;
  * warp             — sincronia do warp com phi_star (I240);
  * phi_history      — serie temporal usada por I239 (convergencia);
  * delta_reward     — ultima recompensa do decisor (I241);
  * tunnel           — gride 16x16 de coerencia;
  * ledger_entries   — contagem de entradas ancoradas no ledger (I255);
  * ipfs_cid         — ultimo CID/hash ancorado;
  * validation       — ultimo relatorio do validador em tempo real.

Nenhuma dependencia externa além da biblioteca padrao.
"""

from __future__ import annotations

import copy
import time
from dataclasses import dataclass, field
from typing import Any, Dict, List

import numpy as np

from core.tunnel import CoherenceTunnel


@dataclass
class SystemState:
    """Contrato de estado do sistema Catedral OS v280.0."""

    phi_star: float = 0.85
    phi_total: float = 0.5
    warp: float = 0.85
    delta_reward: float = 0.0
    steps: int = 0
    tunnel: CoherenceTunnel = field(default_factory=CoherenceTunnel)
    phi_history: List[float] = field(default_factory=list)
    handovers: List[Dict[str, Any]] = field(default_factory=list)
    ledger_entries: int = 0
    ipfs_cid: str = "GENESIS"
    chain_ok: bool = True
    validation: Dict[str, Any] = field(default_factory=dict)
    replay_size: int = 0
    replay_mean_priority: float = 0.0
    cpu_usage: float = 1.0
    memory_usage: float = 24.0

    # ------------------------------------------------------------------ #
    # Atualizacao
    # ------------------------------------------------------------------ #
    def record_estimate(self, phi_raw: float) -> float:
        """Projecao monotona da estimativa de coerencia (I238).

        A estimativa consolidada nunca decresce: o sistema re-ancora o piso
        sempre que a coerencia observada cair (decaimento/reinicio). Isso
        preserva I238 (monotonicidade da estimativa) sem esconder a serie
        bruta — a serie bruta continua disponivel em `tunnel.phi_raw_history`.
        """
        phi = min(max(phi_raw, 0.0), 1.0)
        floor = self.phi_history[-1] if self.phi_history else self.phi_total
        estimate = max(floor, phi)
        self.phi_total = estimate
        self.phi_history.append(estimate)
        return estimate

    def add_handover(self, source: str, metadata: Dict[str, Any]) -> None:
        self.handovers.append(
            {
                "source": source,
                "timestamp": time.time(),
                "metadata": metadata,
            }
        )

    def to_verification(self) -> Dict[str, Any]:
        """Estado reduzido consumido pelo validador Lean em tempo real."""
        window = 20
        history = self.phi_history[-window:]
        return {
            "phi_history": history,
            "phi_star": self.phi_star,
            "warp": self.warp,
            "delta_reward": self.delta_reward,
            "ipfs_cid": self.ipfs_cid,
            "chain_ok": self.chain_ok,
            "replay_size": self.replay_size,
            "replay_mean_priority": self.replay_mean_priority,
            "cpu_usage": self.cpu_usage,
            "memory_usage": self.memory_usage,
        }

    def to_dict(self) -> Dict[str, Any]:
        data = copy.deepcopy(self.__dict__)
        data["tunnel"] = self.tunnel.grid.tolist()
        return data

    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> "SystemState":
        state = cls()
        for key in state.__dict__.keys():
            if key in data:
                setattr(state, key, data[key])
        grid = data.get("tunnel")
        if grid is not None:
            state.tunnel = CoherenceTunnel()
            state.tunnel.grid = np.asarray(grid, dtype=np.float64)
        return state