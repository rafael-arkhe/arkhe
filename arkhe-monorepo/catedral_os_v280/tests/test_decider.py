# catedral_os_v280/tests/test_decider.py
"""Testes do decisor (Double DQN + Atencao) integrado com L1-L3."""

import numpy as np

from core.decider import ACTION_DIM, AttentionHead, Decider, GRID_FLAT
from core.state import SystemState
from core.tunnel import CoherenceTunnel
from persistence.ipfs_ledger import IPFSLedger
from validation.lean_validator import MiniLeanValidator


def test_attention_shapes():
    head = AttentionHead(size=GRID_FLAT, seed=0)
    grid = np.zeros(GRID_FLAT)
    grid[100] = 0.9
    context, weights = head.forward(grid, np.array([0.5, 0.85, 0.85]))
    assert context.shape == (8,)
    assert weights.shape == (GRID_FLAT,)
    assert abs(weights.sum() - 1.0) < 1e-9


def test_observation_dim():
    tunnel = CoherenceTunnel(seed=1)
    ledger = IPFSLedger(use_ipfs=False, persist=False)
    d = Decider(tunnel=tunnel, ledger=ledger, seed=0)
    state = SystemState(phi_star=0.85)
    obs = d._observe(state)
    assert obs.shape == (GRID_FLAT + 8 + 3,)


def test_decider_step_anchors_and_validates():
    tunnel = CoherenceTunnel(seed=1)
    ledger = IPFSLedger(use_ipfs=False, persist=False)
    validator = MiniLeanValidator()
    d = Decider(tunnel=tunnel, ledger=ledger, validator=validator,
                phi_star=0.85, seed=3)
    state = SystemState(phi_star=0.85)
    state = d.step(state)

    assert state.steps == 1
    assert state.ledger_entries == 1
    assert state.ipfs_cid != "GENESIS"
    assert len(state.phi_history) == 1
    assert -0.5 <= state.delta_reward <= 0.5  # I241
    assert ledger.verify_chain()
    assert "I238" in state.validation
    assert state.validation["I255"]["status"] in ("verified", "pending")


def test_decider_learns_over_steps():
    ledger = IPFSLedger(use_ipfs=False, persist=False)
    d = Decider(ledger=ledger, phi_star=0.85, seed=11)
    state = SystemState(phi_star=0.85)
    for _ in range(40):
        state = d.step(state)
    assert state.steps == 40
    assert np.isfinite(state.phi_total)
    assert state.replay_size > 0
    assert ledger.stats()["entries"] == 40
    assert ledger.verify_chain()
    assert not any(
        v["status"] == "violated" for v in state.validation.values()
    )


def test_decider_action_space():
    assert ACTION_DIM == 5
