# catedral_os_v152/tests/test_undulator.py
"""Testes do Undulator (I194.I198): Zeno Veto, decaimento, adaptacao."""

import math
import time

import pytest

from hardware.undulator import (
    AdaptiveParams,
    DecayRate,
    RangingDelta,
    UndulatorNode,
)


def test_ranging_delta_zeno_threshold():
    assert RangingDelta(1000).zeno_threshold() == 2000


def test_decay_rate_from_ranging():
    d = DecayRate.from_ranging(RangingDelta(1000))
    assert d.value == pytest.approx(1000.0)
    # Phi(t) = exp(-1000*0.001) = exp(-1) ~ 0.3679
    assert d.apply(1.0, 0.001) == pytest.approx(math.exp(-1.0))


def test_zeno_veto_rejects_late_handover():
    node = UndulatorNode(RangingDelta(1000))
    time.sleep(0.0005)
    assert node.receive_handover(0.95) is not None
    time.sleep(0.003)  # 3ms > 2ms de limiar
    assert node.receive_handover(0.95) is None


def test_adaptive_decay_bounded():
    node = UndulatorNode(RangingDelta(1000), adaptive=AdaptiveParams())
    h = node.receive_handover(0.9, source="Z1T")
    assert h is not None
    stats = node.stats()
    assert stats["decay_rate"] <= AdaptiveParams().max_decay + 1e-9
    assert stats["decay_rate"] >= AdaptiveParams().min_decay - 1e-9
    assert h.decayed_phi <= h.phi


def test_stats_after_handovers():
    node = UndulatorNode(RangingDelta(1000))
    time.sleep(0.0002)
    node.receive_handover(0.8, source="Z1T")
    stats = node.stats()
    assert stats["handover_count"] == 1
    assert stats["acceptance_rate"] <= 1.0 + 1e-9


def test_packet_encoding_roundtrip():
    node = UndulatorNode(RangingDelta(1000))
    packet = node.encode_packet(0.85, source=3)
    assert len(packet) == 13  # <Bfq
    # source byte
    assert packet[0] == 3


def test_phi_reference_universal():
    from core.spectral import irreducible_coherence
    assert UndulatorNode.phi_reference() == pytest.approx(irreducible_coherence(13))