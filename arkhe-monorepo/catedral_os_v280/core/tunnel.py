# core/tunnel.py
"""
Tunel de coerencia 16x16 com renderizacao (I238).

O tunel e um gridworld de coerencia: um campo potencial `potential` fixo
(pico gaussiano de altura `peak`) define a coerencia de cada celula; o
atrator (posicao do agente) move-se pelo gride e coleta `phi_raw` = valor
do potencial na celula atual. O gride renderizado mostra o campo em torno
do atrator escalado pela coerencia — a politica do decisor aprende a
escalar o potencial (aprender a coerencia).

Acoes: 0=ficar, 1=cima, 2=baixo, 3=esquerda, 4=direita.
"""

from __future__ import annotations

from typing import Dict, Optional, Tuple

import numpy as np


class CoherenceTunnel:
    """Gridworld 16x16 com campo potencial de coerencia."""

    DEFAULT_SIZE = 16
    PEAK = 0.85

    def __init__(self, size: int = DEFAULT_SIZE, seed: Optional[int] = None,
                 attractor: Optional[Tuple[int, int]] = None,
                 peak: float = PEAK,
                 peak_pos: Optional[Tuple[int, int]] = None,
                 sigma: float = 5.0):
        self.size = size
        self.peak = min(max(float(peak), 0.0), 1.0)
        self.rng = np.random.default_rng(seed)
        self.peak_pos = peak_pos or (max(1, size // 5), max(1, size // 5))
        self.potential = self._potential_field(sigma)
        self.grid = np.zeros((size, size), dtype=np.float64)
        self._attractor = attractor or (size // 2, size // 2)
        self.raw_coherence = 0.0
        self.phi_raw_history = []
        self.reset()

    # ------------------------------------------------------------------ #
    # Campo
    # ------------------------------------------------------------------ #
    def _potential_field(self, sigma: float) -> np.ndarray:
        iy, ix = self.peak_pos
        rows, cols = np.mgrid[0 : self.size, 0 : self.size]
        d2 = (rows - iy) ** 2 + (cols - ix) ** 2
        field = self.peak * np.exp(-d2 / (2.0 * sigma**2))
        return field

    def _coherence_at(self, pos: Tuple[int, int]) -> float:
        iy, ix = pos
        return float(min(max(self.potential[iy, ix], 0.0), self.peak))

    def _gaussian_field(self, center: Tuple[int, int],
                        sigma: float = 2.2) -> np.ndarray:
        iy, ix = center
        rows, cols = np.mgrid[0 : self.size, 0 : self.size]
        d2 = (rows - iy) ** 2 + (cols - ix) ** 2
        field = np.exp(-d2 / (2.0 * sigma**2))
        return field

    def reset(self) -> None:
        """Coloca o atrator no centro; a coerencia bruta parte do potencial
        local e cresce com o aprendizado da politica."""
        self._attractor = (self.size // 2, self.size // 2)
        self.raw_coherence = self._coherence_at(self._attractor)
        self.grid = self._gaussian_field(self._attractor) * self.raw_coherence
        self.phi_raw_history = []

    def apply_action(self, action: int) -> Tuple[np.ndarray, Dict]:
        """Aplica uma acao do decisor e retorna (novo gride, info)."""
        iy, ix = self._attractor
        dy, dx = 0, 0
        if action == 1:
            dy = -1
        elif action == 2:
            dy = 1
        elif action == 3:
            dx = -1
        elif action == 4:
            dx = 1

        niy = min(max(iy + dy, 0), self.size - 1)
        nix = min(max(ix + dx, 0), self.size - 1)
        moved = (niy != iy) or (nix != ix)

        self._attractor = (niy, nix)
        self.raw_coherence = self._coherence_at(self._attractor)
        self.grid = self._gaussian_field(self._attractor) * self.raw_coherence
        self.phi_raw_history.append(self.raw_coherence)

        return self.grid, {
            "moved": moved,
            "attractor": self._attractor,
            "raw_coherence": self.raw_coherence,
        }

    def compute_coherence(self) -> float:
        """Coerencia atual (potencial na posicao do atrator)."""
        return self.raw_coherence

    # ------------------------------------------------------------------ #
    # Renderizacao
    # ------------------------------------------------------------------ #
    def render(self) -> str:
        """Renderiza o tunel em ASCII e retorna a string."""
        lines = []
        for row in self.grid:
            line = "".join(
                " .:-=+*#%@@"[min(int(v * (len(" .:-=+*#%@@") - 1)), len(" .:-=+*#%@@") - 1)]
                for v in row
            )
            lines.append(line)
        lines.append("-" * self.size)
        lines.append(
            f"TUNNEL 16x16 | attractor={self._attractor} | "
            f"peak_at={self.peak_pos} | raw_coherence={self.raw_coherence:.4f}"
        )
        return "\n".join(lines)

    def attractor(self) -> Tuple[int, int]:
        return self._attractor

    def coherence_hist(self) -> np.ndarray:
        return np.asarray(self.phi_raw_history, dtype=np.float64)