# catedral_os_v280/tests/test_state_tunnel.py
"""Testes de estado do sistema e do tunel 16x16."""

import time

from core.state import SystemState
from core.tunnel import CoherenceTunnel


def test_state_defaults():
    state = SystemState()
    assert state.phi_star == 0.85
    assert 0.0 <= state.phi_total <= 1.0
    assert state.phi_history == []
    assert state.ipfs_cid == "GENESIS"


def test_record_estimate_monotone():  # I238
    state = SystemState()
    a = state.record_estimate(0.5)
    b = state.record_estimate(0.3)  # menor: nao decai a estimativa
    c = state.record_estimate(0.7)
    assert a <= b <= c
    assert state.phi_history == [0.5, 0.5, 0.7]


def test_state_roundtrip():
    state = SystemState()
    state.record_estimate(0.6)
    state.add_handover("decider", {"action": 1})
    data = state.to_dict()
    restored = SystemState.from_dict(data)
    assert len(restored.phi_history) == 1
    assert restored.handovers[0]["source"] == "decider"
    assert restored.tunnel.grid.shape == (16, 16)


def test_tunnel_grid_and_actions():
    t = CoherenceTunnel(seed=7)
    assert t.grid.shape == (16, 16)
    assert t.grid.min() >= 0.0 and t.grid.max() <= 1.0
    a = t.attractor()
    moved, info = t.apply_action(0)  # ficar
    assert info["moved"] is False
    t.apply_action(1)  # cima
    assert t.attractor()[0] == max(a[0] - 1, 0)


def test_tunnel_render():
    t = CoherenceTunnel(seed=1)
    text = t.render()
    assert "TUNNEL 16x16" in text
    assert len(text.splitlines()) == 18  # 16 linhas + separador + legenda


def test_tunnel_coherence_bounded():
    t = CoherenceTunnel(seed=3)
    for _ in range(10):
        t.apply_action(4)
    assert 0.0 <= t.compute_coherence() <= 1.0
    assert time.time() > 0