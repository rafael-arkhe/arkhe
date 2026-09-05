# catedral_os_v152/tests/test_spectral.py
"""Testes dos invariantes I157.I160 (estrutura espectral)."""

import math

import pytest

from core import spectral as sp


def test_irreducible_coherence_saturation():
    # Para N>=10 satura em 1.0.
    assert sp.irreducible_coherence(10) == pytest.approx(1.0)
    assert sp.irreducible_coherence(2) == 0.0
    # N=4 nao satura: (3/2)*log2(ln4) ~ 0.707
    expected = 1.5 * math.log2(math.log(4))
    assert sp.irreducible_coherence(4) == pytest.approx(expected)


def test_godel_threshold():
    assert sp.godel_threshold(100) == pytest.approx(1.0 / math.log2(100))
    assert sp.godel_threshold(1) == 0.0


def test_phi_irr_normalized():
    spectrum = [0.25, 0.25, 0.25, 0.25]
    phi = sp.compute_phi_irr(spectrum)
    # Renyi-2 de distrib. uniforme de 4 estados = log2(4) = 2, limitada por
    # universal (0.707).
    assert phi == pytest.approx(sp.irreducible_coherence(4))


def test_godel_threshold_reached():
    assert sp.is_godel_threshold_reached([0.9, 0.9, 0.9, 0.9])
    # Espectro de pico unico: Renyi-2 baixa, abaixo do limiar.
    assert not sp.is_godel_threshold_reached([0.9, 0.05, 0.05, 0.0])


def test_anti_flatness_nonneg_and_flat_mesh_is_zero():
    spec = [0.1, 0.2, 0.3, 0.4]
    assert sp.compute_anti_flatness(spec) >= 0.0
    flat = [0.25, 0.25, 0.25, 0.25]
    assert sp.compute_anti_flatness(flat) == pytest.approx(0.0, abs=1e-9)


def test_diadic_shells():
    shells = sp.organize_in_shells([0.1, 0.2, 0.3, 0.4])
    # niveis: i=1→0, i=2→1, i=3→1, i=4→2
    assert set(shells.keys()) == {0, 1, 2}
    assert sp.handover_level(1) == 0
    assert sp.handover_level(2) == 1
    assert sp.handover_level(4) == 2


def test_spectral_handover_extraction():
    h = sp.SpectralHandover.extract([0.4, 0.4, 0.2], source="TCameraS3")
    assert h.source == "TCameraS3"
    assert h.phi_irr > 0.0
    assert h.phi_crit > 0.0
    assert h.shells