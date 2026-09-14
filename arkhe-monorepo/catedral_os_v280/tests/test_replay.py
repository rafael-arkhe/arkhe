# catedral_os_v280/tests/test_replay.py
"""Testes do Prioritized Experience Replay (L2) e do Double DQN."""

import numpy as np
import pytest

from learning.dqn_agent import DoubleDQNAgent, DQNetwork
from learning.prioritized_replay import PrioritizedReplayBuffer


def test_per_add_sample_update():
    buf = PrioritizedReplayBuffer(capacity=100, seed=0)
    for i in range(50):
        buf.add((i,), td_error=float(i % 5))
    assert len(buf) == 50
    indices, samples, weights = buf.sample(10)
    assert len(indices) == 10
    assert len(samples) == 10
    assert weights.shape == (10,)
    assert np.all(weights >= 0)
    assert abs(weights.max() - 1.0) < 1e-9


def test_per_update_reflects_priorities():
    buf = PrioritizedReplayBuffer(capacity=100, seed=0)
    for i in range(20):
        buf.add((i,), td_error=0.0)
    indices, samples, _ = buf.sample(5)
    buf.update_priorities(indices, np.full(5, 100.0))
    stats = buf.stats()
    assert stats["mean_priority"] > 0
    assert stats["max_priority"] > 0


def test_per_capacity_eviction():
    buf = PrioritizedReplayBuffer(capacity=5, seed=0)
    for i in range(50):
        buf.add((i,), td_error=1.0)
    assert len(buf) == 5


def test_dqn_forward_shapes():
    net = DQNetwork(in_dim=10, hidden=16, out_dim=5, seed=0)
    q, _cache = net.forward(np.zeros((3, 10)))
    assert q.shape == (3, 5)


def test_agent_select_and_train():
    agent = DoubleDQNAgent(obs_dim=24, act_dim=5, hidden=16, seed=0,
                           eps_start=0.9, batch_size=8, buffer_capacity=100)
    obs = np.random.default_rng(0).normal(size=24)
    # exploração
    for _ in range(10):
        a, q = agent.select_action(obs)
        assert 0 <= a < 5
        assert q.shape == (5,)
        agent.store(obs, a, 0.2, obs, False, td_error=1.0)
    for _ in range(3):
        td = agent.train_step()
        assert np.isfinite(td)
    agent.soft_update()
    # rede target segue a online lentamente
    diff = float(np.abs(agent.online.w1 - agent.target.w1).max())
    assert diff > 0.0


def test_agent_save_load(tmp_path):
    agent = DoubleDQNAgent(obs_dim=8, act_dim=3, hidden=8, seed=1)
    path = str(tmp_path / "agent.npz")
    agent.save(path)
    clone = DoubleDQNAgent(obs_dim=8, act_dim=3, hidden=8, seed=99)
    assert clone.load(path)
    assert float(np.abs(clone.online.w1 - agent.online.w1).max()) == 0.0