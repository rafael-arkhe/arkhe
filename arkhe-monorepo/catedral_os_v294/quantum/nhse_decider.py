# quantum/nhse_decider.py
"""
Decisor CQFI — Double DQN com camada de atenção para o fator de
aprimoramento NHSE (v294.0).

A política aprende a maximizar a recompensa R = phi * E(N), onde phi é a
coerência pós-NHSE e E(N) é o fator de aprimoramento topológico
(I319). A ação escolhida regula kappa (taxa de decaimento não-Bloch,
raio da GBZ) e/ou phi, guiando o sistema para o regime de amplificação
mais alto sob o limite de Cramér-Rao (I320).

Ações (4):
  0  increase_kappa  — aumenta kappa (raio da GBZ e aprimoramento);
  1  decrease_kappa  — reduz kappa (estabilização);
  2  increase_phi    — aumenta a coerência basal;
  3  decrease_phi    — reduz a coerência basal.
"""

from __future__ import annotations

import logging
import random
from collections import deque
from typing import Deque, Optional, Tuple

import numpy as np

try:
    import torch
    import torch.nn as nn

    TORCH_AVAILABLE = True
except ImportError:  # pragma: no cover - ambiente sem torch
    TORCH_AVAILABLE = False
    torch = None
    nn = None

from quantum.nhse_engine import NHSEngine

logger = logging.getLogger(__name__)


class NHSE_DQN(nn.Module):
    """Rede Q com gate de atenção sobre o estado NHSE."""

    def __init__(self, state_dim: int = 6, hidden_dim: int = 64,
                 output_dim: int = 4):
        super().__init__()
        self.net = nn.Sequential(
            nn.Linear(state_dim, hidden_dim),
            nn.ReLU(),
            nn.Linear(hidden_dim, hidden_dim),
            nn.ReLU(),
            nn.Linear(hidden_dim, output_dim),
        )
        self.attention = nn.Linear(state_dim, 1)

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        att = torch.sigmoid(self.attention(x))
        return self.net(x) * att


class NHSEDecider:
    """Decisor que otimiza a amplificação topológica da coerência (CQFI)."""

    def __init__(self, nhse_engine: Optional[NHSEngine] = None,
                 state_dim: int = 6, action_dim: int = 4, lr: float = 1e-3,
                 gamma: float = 0.99, buffer_size: int = 10000,
                 batch_size: int = 64, epsilon: float = 0.1,
                 update_target_every: int = 100, seed: Optional[int] = None):
        self.nhse = nhse_engine or NHSEngine()
        self.device = torch.device("cuda" if torch.cuda.is_available() else "cpu")

        self.q_net = NHSE_DQN(state_dim, 64, action_dim).to(self.device)
        self.target_net = NHSE_DQN(state_dim, 64, action_dim).to(self.device)
        self.target_net.load_state_dict(self.q_net.state_dict())
        self.target_net.eval()

        self.optimizer = torch.optim.Adam(self.q_net.parameters(), lr=lr)
        self.loss_fn = nn.MSELoss()
        self.gamma = gamma
        self.batch_size = batch_size
        self.buffer: Deque[Tuple] = deque(maxlen=buffer_size)
        self.steps = 0
        self.update_target_every = update_target_every
        self.epsilon = epsilon
        self.rng = random.Random(seed)

    # ------------------------------------------------------------------ #
    def act(self, state: np.ndarray, epsilon: Optional[float] = None) -> int:
        """Seleção epsilon-greedy da ação (0..3)."""
        eps = self.epsilon if epsilon is None else epsilon
        if self.rng.random() < eps:
            return self.rng.randrange(0, 4)
        state_t = torch.FloatTensor(np.asarray(state, dtype=np.float32)).unsqueeze(0).to(self.device)
        with torch.no_grad():
            q_values = self.q_net(state_t)
        return int(q_values.argmax(dim=1).item())

    def remember(self, state: np.ndarray, action: int, reward: float,
                 next_state: np.ndarray, done: bool) -> None:
        self.buffer.append(
            (np.asarray(state, dtype=np.float32), int(action), float(reward),
             np.asarray(next_state, dtype=np.float32), bool(done))
        )

    def learn(self) -> float:
        """Um passo de Double DQN; retorna o TD médio absoluto."""
        if len(self.buffer) < self.batch_size:
            return 0.0
        batch = self.rng.sample(self.buffer, self.batch_size)
        states, actions, rewards, next_states, dones = zip(*batch)

        states = torch.FloatTensor(np.array(states)).to(self.device)
        actions = torch.LongTensor(np.array(actions)).to(self.device)
        rewards = torch.FloatTensor(np.array(rewards)).to(self.device)
        next_states = torch.FloatTensor(np.array(next_states)).to(self.device)
        dones = torch.FloatTensor(np.array(dones, dtype=np.float32)).to(self.device)

        with torch.no_grad():
            next_actions = self.q_net(next_states).argmax(dim=1)
            next_q = self.target_net(next_states).gather(1, next_actions.unsqueeze(1)).squeeze(1)
        target = rewards + self.gamma * next_q * (1.0 - dones)

        current_q = self.q_net(states).gather(1, actions.unsqueeze(1)).squeeze(1)
        loss = self.loss_fn(current_q, target.detach())

        self.optimizer.zero_grad()
        loss.backward()
        self.optimizer.step()
        self.steps += 1

        if self.steps % self.update_target_every == 0:
            self.target_net.load_state_dict(self.q_net.state_dict())

        return float(loss.item())

    def get_reward(self, phi: float, enhancement: float) -> float:
        """I319: R = Φ * E(N) — maximiza a amplificação topológica."""
        return float(phi * enhancement)


if not TORCH_AVAILABLE:  # pragma: no cover - declara tipo antes do runtime
    logger.warning("torch ausente — NHSEDecider indisponível até a instalação do requisito.")