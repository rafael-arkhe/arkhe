# learning/prioritized_replay.py
"""
L2 — Prioritized Experience Replay (PER) (v280.0).

Prioridade = (|TD error| + eps)^alpha, amostragem proporcional a prioridade e
pesos de importancia (IS) com exponente beta para correcao de vies. O metodo
`update_priorities` recebe os indices devolvidos por `sample` — a API completa
de PER (indices + amostras + pesos), sem fingir re-priorizacao.
"""

from __future__ import annotations

import logging
from typing import Any, List, Optional, Tuple

import numpy as np

logger = logging.getLogger(__name__)


class PrioritizedReplayBuffer:
    """Buffer de replay com amostragem priorizada (Schaul et al., 2016)."""

    def __init__(self, capacity: int = 10000, alpha: float = 0.6,
                 beta: float = 0.4, seed: Optional[int] = None):
        self.capacity = max(1, int(capacity))
        self.alpha = alpha
        self.beta = beta
        self.rng = np.random.default_rng(seed)
        self._buffer: List[Any] = []
        self._priorities = np.zeros(self.capacity, dtype=np.float64)
        self._pos = 0
        self._size = 0
        self._last_weights: Optional[np.ndarray] = None

    # ------------------------------------------------------------------ #
    def add(self, transition: Any, td_error: float = 0.0) -> None:
        priority = (abs(float(td_error)) + 1e-6) ** self.alpha
        if self._size < self.capacity:
            self._buffer.append(transition)
            self._priorities[self._size] = priority
            self._size += 1
        else:
            self._buffer[self._pos] = transition
            self._priorities[self._pos] = priority
        self._pos = (self._pos + 1) % self.capacity

    def sample(self, batch_size: int) -> Tuple[np.ndarray, List[Any], np.ndarray]:
        """Retorna (indices, amostras, pesos_is)."""
        if self._size == 0 or batch_size <= 0:
            return np.asarray([], dtype=np.int64), [], np.asarray([])

        bs = min(batch_size, self._size)
        priorities = self._priorities[: self._size]
        probs = priorities / priorities.sum()
        indices = self.rng.choice(self._size, size=bs, replace=False, p=probs)
        samples = [self._buffer[int(i)] for i in indices]

        weights = (self._size * probs[indices]) ** (-self.beta)
        weights /= weights.max()
        self._last_weights = weights
        return indices.astype(np.int64), samples, weights

    def update_priorities(self, indices: np.ndarray,
                          td_errors: np.ndarray) -> None:
        for idx, td in zip(indices, td_errors):
            idx = int(idx)
            if 0 <= idx < self._size:
                self._priorities[idx] = (abs(float(td)) + 1e-6) ** self.alpha

    # ------------------------------------------------------------------ #
    def stats(self) -> dict:
        priorities = self._priorities[: self._size]
        return {
            "size": self._size,
            "mean_priority": float(priorities.mean()) if self._size else 0.0,
            "max_priority": float(priorities.max()) if self._size else 0.0,
        }

    def __len__(self) -> int:
        return self._size