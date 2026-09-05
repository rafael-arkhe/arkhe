# catedral_os_v152/tests/test_triad_integrator.py
"""Teste E2E do integrador da Triade + Undulator (I190.I192, I194)."""

import time

from triad_integrator import TriadIntegrator


def test_triad_runs_and_produces_blocks():
    integrator = TriadIntegrator(target_phi=0.95, decay_rate=0.0001)
    integrator.start()
    try:
        deadline = time.monotonic() + 3.0
        while time.monotonic() < deadline:
            if integrator.stats["perception_count"] >= 5:
                break
            time.sleep(0.05)
    finally:
        integrator.stop()

    assert integrator.stats["perception_count"] >= 5
    assert len(integrator.chain.blocks) > 0
    assert integrator.z1t.is_simulating  # honesto: sem hardware, simula


def test_triad_visualizer_and_governance_attach():
    from adaptive.governance_adaptive import AdaptiveGovernance
    from dashboard.visualizer import MeshVisualizer

    integrator = TriadIntegrator(target_phi=0.95)
    gov = AdaptiveGovernance(chain=integrator.chain, integrator=integrator)
    viz = MeshVisualizer(width=20, height=10)

    gov.report()  # sem crash
    viz.add_node(0.5, 0.5, 0.8, "CatedralOS")
    assert len(viz.nodes) == 1