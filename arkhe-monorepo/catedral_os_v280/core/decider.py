# core/decider.py
"""
Decisor com Double DQN + Atencao (v280.0).

O decisor transforma o estado em uma observacao composta:

  * gride 16x16 (256 celulas);
  * contexto de atencao sobre o gride (8 dims, posicoes sinusoidais,
    valores modulados pela coerencia da celula);
  * 3 escalares (phi_total, phi_star, warp).

A policy e Double DQN (ver `learning.dqn_agent`). A cada passo o decisor:

  1. escolhe a acao (epsilon-greedy + rede target);
  2. aplica no tunel (L4 constroi multi-tuneis sobre isto);
  3. ancora a acao no ledger (L1) — loopseal-1 TemporalChain;
  4. registra a experiencia no replay priorizado (L2);
  5. valida os invariantes em tempo real (L3).

O retorno e um novo `SystemState` com `validation`, `ipfs_cid`,
`phi_history` e `delta_reward` atualizados.
"""

from __future__ import annotations

import logging
from typing import Dict, Optional, Tuple

import numpy as np

from core.state import SystemState
from core.tunnel import CoherenceTunnel
from learning.dqn_agent import DoubleDQNAgent
from persistence.ipfs_ledger import IPFSLedger
from validation.lean_validator import MiniLeanValidator

logger = logging.getLogger(__name__)

GRID_FLAT = 256
SCALAR_DIM = 3
CONTEXT_DIM = 8
ACTION_DIM = 5  # 0=stay, 1=up, 2=down, 3=left, 4=right


class AttentionHead:
    """Atencao simples sobre o gride (posicoes sinusoidais como keys)."""

    def __init__(self, size: int, context_dim: int = CONTEXT_DIM,
                 seed: Optional[int] = None):
        self.size = size
        self.context_dim = context_dim
        rng = np.random.default_rng(seed)
        positions = np.arange(size, dtype=np.float64)
        div = np.power(10000.0, np.arange(0, context_dim) / context_dim)
        self.positions = np.stack(
            [np.sin(positions / d) if i % 2 == 0 else np.cos(positions / d)
             for i, d in enumerate(div)],
            axis=1,
        )  # (size, context_dim)
        self.q_proj = rng.standard_normal((context_dim, GRID_FLAT + SCALAR_DIM)) * 0.05

    def forward(self, grid_flat: np.ndarray,
                scalars: np.ndarray) -> Tuple[np.ndarray, np.ndarray]:
        """Retorna (contexto, pesos de atencao)."""
        x = np.concatenate([grid_flat, scalars])
        q = self.q_proj @ x  # (context_dim,)
        scores = q @ self.positions.T / np.sqrt(self.context_dim)  # (size,)
        scores = scores - scores.max()
        exp = np.exp(scores)
        weights = exp / exp.sum()
        values = grid_flat.reshape(-1, 1) * self.positions  # (size, ctx)
        context = weights @ values  # (context_dim,)
        return context, weights


class Decider:
    """Decisor Double DQN + Atencao sobre o tunel de coerencia."""

    def __init__(self, tunnel: Optional[CoherenceTunnel] = None,
                 ledger: Optional[IPFSLedger] = None,
                 validator: Optional[MiniLeanValidator] = None,
                 phi_star: float = 0.85, seed: Optional[int] = None,
                 episode: int = 0):
        self.tunnel = tunnel or CoherenceTunnel(seed=seed, peak=phi_star)
        self.tunnel.reset()
        self.ledger = ledger or IPFSLedger(use_ipfs=False)
        self.validator = validator or MiniLeanValidator()
        self.phi_star = phi_star
        self.episode = episode
        self.seed = seed
        self.rng = np.random.default_rng(seed)

        obs_dim = GRID_FLAT + CONTEXT_DIM + SCALAR_DIM
        self.agent = DoubleDQNAgent(
            obs_dim=obs_dim, act_dim=ACTION_DIM, hidden=32, lr=3e-3,
            batch_size=16, buffer_capacity=5000, seed=seed,
        )
        self.attention = AttentionHead(GRID_FLAT, seed=(seed or 0))

    # ------------------------------------------------------------------ #
    def _observe(self, state: SystemState) -> np.ndarray:
        grid_flat = state.tunnel.grid.flatten()
        scalars = np.asarray(
            [state.phi_total, state.phi_star, state.warp], dtype=np.float64
        )
        context, _ = self.attention.forward(grid_flat, scalars)
        return np.concatenate([grid_flat, context, scalars])

    def _reward(self, phi_old: float, phi_new: float) -> float:
        convergence = -abs(phi_new - self.phi_star) * 0.05
        delta = (phi_new - phi_old) * 10.0
        reward = delta + convergence
        return float(np.clip(reward, -0.5, 0.5))  # I241

    # ------------------------------------------------------------------ #
    def step(self, state: SystemState) -> SystemState:
        obs = self._observe(state)
        action, _q = self.agent.select_action(obs)

        phi_old = state.phi_total
        _grid, info = state.tunnel.apply_action(action)
        phi_raw = min(info["raw_coherence"], self.phi_star)  # dissipa excedente

        # Estimativa monotona da coerencia consolidada (I238).
        phi_consolidated = state.record_estimate(phi_raw)
        reward = self._reward(phi_old, phi_consolidated)
        done = abs(phi_consolidated - self.phi_star) < 0.05

        state.delta_reward = reward
        state.steps += 1
        state.warp = self.phi_star + float(self.rng.normal(0.0, 0.005))

        # Memoria priorizada (L2).
        self.agent.store(obs, action, reward, self._observe(state), done)
        self.agent.train_step()
        self.agent.soft_update()
        stats = self.agent.buffer.stats()
        state.replay_size = stats["size"]
        state.replay_mean_priority = stats["mean_priority"]

        # Ledger imutavel (L1) — ancoragem de cada acao.
        cid = self.ledger.append(
            {
                "type": "action",
                "episode": self.episode,
                "action": int(action),
                "reward": reward,
                "phi_total": phi_consolidated,
                "phi_star": self.phi_star,
                "warp": state.warp,
                "attractor": list(state.tunnel.attractor()),
            }
        )
        state.ipfs_cid = cid
        state.ledger_entries += 1

        # Validacao em tempo real (L3).
        report = self.validator.verify(state.to_verification())
        state.validation = report.to_dict()
        state.chain_ok = self.ledger.is_chain_ok()

        state.add_handover("decider", {"action": int(action), "reward": reward})
        return state