# python/adaptive/shell_selector.py
"""
Selecao adaptativa de cascas diadicas (I199) — escolhe o nivel de casca que
maximiza a entropia construtiva (I167), com penalidade de profundidade.
"""

from __future__ import annotations

from typing import List

from core.entropy import constructive_entropy
from core.spectral import organize_in_shells


class ShellSelector:
    """Seleciona o nivel de casca com maior entropia construtiva."""

    def __init__(self, depth_decay: float = 0.95, history_window: int = 20):
        self.depth_decay = depth_decay
        self.history_window = history_window
        self.last_selected: int = 0
        self._entropy_history: List[List[float]] = []

    def select_shell(self, spectrum: List[float],
                     n_handovers: int,
                     alpha: float = 1.5) -> int:
        """I199: retorna o nivel de casca com maior entropia ajustada."""
        if not spectrum:
            return 0

        shells = organize_in_shells(spectrum)
        best_level = 0
        best_score = float("-inf")
        n_total = len(spectrum)

        for level, values in shells.items():
            if not values:
                continue
            entropy = constructive_entropy(values, n_total, n_handovers, alpha)
            penalty = self.depth_decay ** level
            score = entropy * penalty
            if score > best_score:
                best_score = score
                best_level = level

        self._entropy_history.append([best_score])
        if len(self._entropy_history) > self.history_window:
            self._entropy_history.pop(0)
        self.last_selected = best_level
        return best_level