# catedral_os_v152/tests/test_vision.py
"""Testes do pipeline de visao (I157.I160) — modelo desktop de referencia."""

import pytest

from vision.pipeline import frame_packet, spectral_handovers


def test_spectral_handovers_numpy():
    np = pytest.importorskip("numpy")
    grid = np.zeros((16, 16), dtype=np.float64)
    grid[4:12, 4:12] = 1.0  # bloco central dominante
    h = spectral_handovers(grid)
    assert h["phi_irr"] > 0.0
    assert h["phi_crit"] > 0.0
    assert h["anti_flatness"] >= 0.0
    assert h["contrast"] >= 0.0
    assert len(h["shells"]) == 5


def test_spectral_handovers_pure_python():
    grid = [[x ** 2 + y * 0.5 for x in range(8)] for y in range(8)]
    h = spectral_handovers(grid)
    assert h["phi_irr"] > 0.0
    assert h["contrast"] >= 0.0


def test_frame_packet_roundtrips():
    from hardware.z1t_bridge import unframe
    packet = frame_packet(b"Z1T_HANDOVER")
    assert unframe(packet) == b"Z1T_HANDOVER"