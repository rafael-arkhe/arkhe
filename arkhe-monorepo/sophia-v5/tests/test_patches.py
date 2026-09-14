"""Testes dos patches #19-22 e dos bounds constitucionais (Gap-1)."""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "agents" / "python"))

from sophia_local.patches import (
    PHI_C_MAX,
    PHI_C_MIN,
    SophiaPatches,
    UnifiedCoherenceField,
)


def test_unified_field_respects_constitutional_bounds() -> None:
    uf = UnifiedCoherenceField(value=0.5)
    for _ in range(500):
        uf.step()
        assert uf.in_bounds
        assert PHI_C_MIN <= uf.value <= PHI_C_MAX


def test_psi_c_stays_above_minimum() -> None:
    uf = UnifiedCoherenceField(value=PHI_C_MIN)
    for _ in range(2000):
        uf.step()
        assert uf.value >= PHI_C_MIN


def test_q_coefficient_from_twin_study() -> None:
    import random

    sp = SophiaPatches(seed=1)
    responses = [random.random() for _ in range(100)]
    q = sp.run_cycle(twin_responses=responses)["q_coefficient"]
    assert 0.0 <= q <= 1.0


def test_full_cycle_metrics_complete() -> None:
    sp = SophiaPatches(seed=7, psi_initial=0.6)
    m = sp.run_cycle(twin_responses=[0.5, 0.55, 0.45])
    assert set(m) >= {"psi_C", "q_coefficient", "global_coherence",
                      "decoherence_time", "in_bounds"}
    assert m["in_bounds"] == 1.0


def test_cavity_decoherence_eventually_decays() -> None:
    sp = SophiaPatches(seed=3)
    out = sp.run_cycle()
    assert sp.cavity.decoherence_time() > 0