# learning/dqn_agent.py
"""
L2 — Agente Double DQN (v280.0).

Duas redes MLP (online + target) treinadas em numpy puro, aprendizado por
gradiente descendente para minimizar o erro ao quadrado contra o alvo de
Double DQN:

    a* = argmax_a Q_online(s')
    target = r + gamma * Q_target(s', a*)

Replay com PrioritizedExperienceReplay (L2) e atualizacao suave (soft update)
da rede target (`tau`). Sem dependencia de framework externo.
"""

from __future__ import annotations

import logging
from typing import Optional, Sequence, Tuple

import numpy as np

from learning.prioritized_replay import PrioritizedReplayBuffer

logger = logging.getLogger(__name__)


def relu(x: np.ndarray) -> np.ndarray:
    return np.maximum(x, 0.0)


class DQNetwork:
    """MLP numpy: obs -> [relu -> relu -> linear] -> acoes."""

    def __init__(self, in_dim: int, hidden: int, out_dim: int,
                 seed: Optional[int] = None):
        rng = np.random.default_rng(seed)
        self.w1 = rng.standard_normal((in_dim, hidden)) * np.sqrt(1.0 / in_dim)
        self.b1 = np.zeros(hidden, dtype=np.float64)
        self.w2 = rng.standard_normal((hidden, hidden)) * np.sqrt(1.0 / hidden)
        self.b2 = np.zeros(hidden, dtype=np.float64)
        self.w3 = rng.standard_normal((hidden, out_dim)) * np.sqrt(1.0 / hidden)
        self.b3 = np.zeros(out_dim, dtype=np.float64)

    def forward(self, x: np.ndarray) -> Tuple[np.ndarray, tuple]:
        z1 = x @ self.w1 + self.b1
        a1 = relu(z1)
        z2 = a1 @ self.w2 + self.b2
        a2 = relu(z2)
        q = a2 @ self.w3 + self.b3
        return q, (x, z1, a1, z2, a2)

    def train_step(self, x: np.ndarray, y_target: np.ndarray,
                   lr: float = 1e-3) -> float:
        """Um passo de gradiente descendente; retorna a perda media."""
        batch = x.shape[0]
        q, (xs, z1, a1, z2, a2) = self.forward(x)
        loss = float(np.mean((q - y_target) ** 2))

        dv = 2.0 * (q - y_target) / batch

        gw3 = a2.T @ dv
        gb3 = dv.sum(axis=0)
        da2 = dv @ self.w3.T
        dz2 = da2 * (z2 > 0)
        gw2 = a1.T @ dz2
        gb2 = dz2.sum(axis=0)
        da1 = dz2 @ self.w2.T
        dz1 = da1 * (z1 > 0)
        gw1 = xs.T @ dz1
        gb1 = dz1.sum(axis=0)

        for w, g in ((self.w1, gw1), (self.b1, gb1),
                     (self.w2, gw2), (self.b2, gb2),
                     (self.w3, gw3), (self.b3, gb3)):
            w -= lr * g
        return loss

    def copy_from(self, other: "DQNetwork") -> None:
        self.w1 = other.w1.copy()
        self.b1 = other.b1.copy()
        self.w2 = other.w2.copy()
        self.b2 = other.b2.copy()
        self.w3 = other.w3.copy()
        self.b3 = other.b3.copy()


class DoubleDQNAgent:
    """Agente Double DQN com replay priorizado e exploracao epsilon-greedy."""

    def __init__(self, obs_dim: int, act_dim: int, hidden: int = 48,
                 gamma: float = 0.95, lr: float = 1e-3, tau: float = 0.02,
                 eps_start: float = 0.9, eps_min: float = 0.05,
                 eps_decay: float = 0.995, batch_size: int = 32,
                 buffer_capacity: int = 10000, alpha: float = 0.6,
                 beta: float = 0.4, seed: Optional[int] = None):
        self.obs_dim = obs_dim
        self.act_dim = act_dim
        self.eps_start = eps_start
        self.eps_min = eps_min
        self.eps_decay = eps_decay
        self.gamma = gamma
        self.lr = lr
        self.tau = tau
        self.batch_size = batch_size
        self.rng = np.random.default_rng(seed)

        self.online = DQNetwork(obs_dim, hidden, act_dim, seed=seed)
        self.target = DQNetwork(obs_dim, hidden, act_dim, seed=(seed or 0) + 1)
        self.target.copy_from(self.online)

        self.buffer = PrioritizedReplayBuffer(
            capacity=buffer_capacity, alpha=alpha, beta=beta, seed=seed
        )
        self._eps = eps_start
        self.steps = 0

    # ------------------------------------------------------------------ #
    def select_action(self, obs: np.ndarray) -> Tuple[int, np.ndarray]:
        """Greedy (Double DQN) com exploracao epsilon-greedy."""
        q = self.online.forward(obs.reshape(1, -1))[0][0]
        if self.rng.random() < self._eps:
            action = int(self.rng.integers(self.act_dim))
        else:
            action = int(np.argmax(q))
        return action, q

    def store(self, obs: np.ndarray, action: int, reward: float,
              next_obs: np.ndarray, done: bool, td_error: float = 0.0) -> None:
        self.buffer.add((obs.copy(), action, reward, next_obs.copy(), done),
                        td_error=td_error)

    # ------------------------------------------------------------------ #
    def train_step(self) -> float:
        """Amostra do PER, constroi o alvo Double DQN e atualiza. Retorna o
        TD medio absoluto."""
        indices, samples, weights = self.buffer.sample(self.batch_size)
        del weights
        if len(samples) == 0:
            return 0.0

        obs_b = np.asarray([s[0] for s in samples])
        act_b = np.asarray([s[1] for s in samples], dtype=np.int64)
        rew_b = np.asarray([s[2] for s in samples], dtype=np.float64)
        nxt_b = np.asarray([s[3] for s in samples])
        done_b = np.asarray([s[4] for s in samples], dtype=np.float64)

        q_next_online = self.online.forward(nxt_b)[0]
        a_star = np.argmax(q_next_online, axis=1)
        q_next_target = self.target.forward(nxt_b)[0]
        target_vals = q_next_target[np.arange(len(samples)), a_star]
        targets = rew_b + self.gamma * (1.0 - done_b) * target_vals

        q_now = self.online.forward(obs_b)[0]
        td_errors = targets - q_now[np.arange(len(samples)), act_b]

        # Double DQN: o alvo (batch, act) aplica a correcao apenas na acao
        # tomada; as demais entradas mantem o valor online para a perda.
        y_target = q_now.copy()
        y_target[np.arange(len(samples)), act_b] = targets

        self.buffer.update_priorities(indices, td_errors)
        self.online.train_step(obs_b, y_target, lr=self.lr)
        self.steps += 1
        self._eps = max(self.eps_min, self._eps * self.eps_decay)
        return float(np.mean(np.abs(td_errors)))

    def soft_update(self) -> None:
        for online_w, target_w in (
            (self.online.w1, self.target.w1),
            (self.online.b1, self.target.b1),
            (self.online.w2, self.target.w2),
            (self.online.b2, self.target.b2),
            (self.online.w3, self.target.w3),
            (self.online.b3, self.target.b3),
        ):
            target_w[:] = self.tau * online_w + (1.0 - self.tau) * target_w

    def decay_epsilon(self, factor: Optional[float] = None) -> None:
        factor = factor or self.eps_decay
        self._eps = max(self.eps_min, self._eps * factor)

    # ------------------------------------------------------------------ #
    def save(self, path: str) -> None:
        np.savez(
            path,
            w1=self.online.w1, b1=self.online.b1,
            w2=self.online.w2, b2=self.online.b2,
            w3=self.online.w3, b3=self.online.b3,
            tw1=self.target.w1, tb1=self.target.b1,
            tw2=self.target.w2, tb2=self.target.b2,
            tw3=self.target.w3, tb3=self.target.b3,
            eps=self._eps,
        )

    def load(self, path: str) -> bool:
        try:
            data = np.load(path)
            for key in ("w1", "b1", "w2", "b2", "w3", "b3"):
                setattr(self.online, key, data[key])
            for key in ("tw1", "tb1", "tw2", "tb2", "tw3", "tb3"):
                setattr(self.target, key, data[key])
            self._eps = float(data["eps"])
            return True
        except Exception as exc:  # pragma: no cover - arquivo ausente
            logger.error("Falha ao carregar pesos: %s", exc)
            return False