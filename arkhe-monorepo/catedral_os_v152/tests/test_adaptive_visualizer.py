# catedral_os_v152/tests/test_adaptive_visualizer.py
"""Testes da governanca adaptativa (I199/G14) e visualizador."""

import time

import pytest

from adaptive.governance_adaptive import AdaptiveGovernance, GovernanceMetrics
from adaptive.shell_selector import ShellSelector
from core.temporal_chain import Handover, TemporalChain
from dashboard.visualizer import MeshVisualizer


# --------------------------------------------------------------------------- #
# ShellSelector (I199)
# --------------------------------------------------------------------------- #
def test_shell_selector_selects_something():
    selector = ShellSelector()
    spectrum = [0.4, 0.1, 0.1, 0.1, 0.1, 0.1, 0.1, 0.0]
    level = selector.select_shell(spectrum, n_handovers=8)
    assert isinstance(level, int)
    assert level >= 0
    assert level in {0, 1, 2}


def test_shell_selector_empty_spectrum():
    selector = ShellSelector()
    assert selector.select_shell([], n_handovers=0) == 0


def test_shell_selector_tracks_history():
    selector = ShellSelector(history_window=5)
    for _ in range(10):
        selector.select_shell([0.3, 0.3, 0.4], n_handovers=8)
    assert len(selector._entropy_history) <= 5


# --------------------------------------------------------------------------- #
# AdaptiveGovernance
# --------------------------------------------------------------------------- #
class _FakeIntegrator:
    def __init__(self):
        self.stats = {
            "perception_count": 10,
            "inference_count": 8,
            "proof_count": 4,
            "undulator_count": 3,
        }


def test_governance_adjusts_low_coherence():
    chain = TemporalChain(target_phi=0.95, decay_rate=0.001)
    integrator = _FakeIntegrator()
    gov = AdaptiveGovernance(chain=chain, integrator=integrator)
    gov.inference_interval = 0.1
    gov.target_phi = 0.95
    metrics = GovernanceMetrics(
        perception_rate=1.0, inference_rate=0.8, proof_rate=0.4,
        undulator_rate=0.3, total_coherence=0.2,  # < 80% do alvo
        energy_efficiency=1e16,
    )
    gov._adjust_parameters(metrics)
    assert gov.inference_interval < 0.1  # acelerou a inferencia


def test_governance_adjusts_low_efficiency():
    chain = TemporalChain(target_phi=0.95, decay_rate=0.001)
    integrator = _FakeIntegrator()
    gov = AdaptiveGovernance(chain=chain, integrator=integrator)
    gov.decay_rate = 0.001
    metrics = GovernanceMetrics(
        perception_rate=1.0, inference_rate=0.8, proof_rate=0.4,
        undulator_rate=0.3, total_coherence=0.9,
        energy_efficiency=1e10,  # < 1e14
    )
    gov._adjust_parameters(metrics)
    assert gov.decay_rate > 0.001  # esquece mais


# --------------------------------------------------------------------------- #
# Visualizer (G14)
# --------------------------------------------------------------------------- #
def test_visualizer_renders_grid():
    viz = MeshVisualizer(width=10, height=4)
    viz.add_node(0.0, 0.0, 0.9, "Z1T")
    viz.add_node(1.0, 1.0, 0.3, "Undulator")
    out = viz.render()
    assert len(out.splitlines()) == 4 + 2  # linhas + separador + legenda
    assert "NOS: 2" in out


def test_visualizer_empty():
    viz = MeshVisualizer(width=8, height=3)
    out = viz.render()
    lines = out.splitlines()
    assert "PHI: 0.000" in lines[-1]