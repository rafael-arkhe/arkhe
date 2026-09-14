"""Testes do simulador PIC 2D com saturação DPDM (modos de maior k)."""

import numpy as np
import pytest

from coulomb_explosion_2d import CoulombExplosion2D_DPDM


def test_saturation_metrics_are_bounded():
    sim = CoulombExplosion2D_DPDM(n_ions=200, seed=1)
    for _ in range(100):
        sim.step(dt=1e-14)
    m = sim.get_saturation_metrics()
    assert 0.0 <= m["saturation_fraction"] <= 1.0
    assert m["kinetic_energy_j"] > 0.0
    assert m["max_density_perturb"] >= 0.0
    assert np.isfinite(m["omega_p_eff"])
    assert m["n_modes"] >= 0


def test_modes_not_excited_above_thermal_threshold():
    # Temperatura eletrônica alta → energia cinética abaixo do limiar → sem
    # modos, campo E nulo, ω_p inalterado.
    sim = CoulombExplosion2D_DPDM(n_ions=200, electron_temp_ev=1e6, seed=2)
    for _ in range(50):
        sim.step(dt=1e-14)
    assert sim.k_modes == []
    assert np.all(sim.E_field == 0.0)
    assert sim.omega_p == pytest.approx(1.0, abs=1e-12)


def test_low_thermal_threshold_excites_higher_k_modes():
    # Temperatura eletrônica baixa → energia cinética >> térmica → cascata de
    # modos de maior k com densidade perturbada e ω_p efetiva deslocada.
    sim = CoulombExplosion2D_DPDM(n_ions=300, electron_temp_ev=0.001, seed=3)
    for _ in range(80):
        sim.step(dt=1e-14)
    m = sim.get_saturation_metrics()
    assert m["n_modes"] > 0
    assert m["max_density_perturb"] > 0.0
    assert np.mean(np.abs(sim.E_field)) > 0.0
    assert m["omega_p_eff"] != pytest.approx(1.0, abs=1e-9)


def test_finite_positions_and_velocities():
    sim = CoulombExplosion2D_DPDM(n_ions=200, electron_temp_ev=0.001, seed=4)
    for _ in range(100):
        sim.step(dt=1e-14)
    assert np.isfinite(sim.x).all()
    assert np.isfinite(sim.y).all()
    assert np.isfinite(sim.vx).all()
    assert np.isfinite(sim.vy).all()


def test_radius_expands_over_time():
    sim = CoulombExplosion2D_DPDM(n_ions=300, seed=5)
    r0 = np.sqrt(sim.x**2 + sim.y**2).max()
    for _ in range(120):
        sim.step(dt=1e-14)
    r1 = np.sqrt(sim.x**2 + sim.y**2).max()
    assert r1 > r0


def test_deterministic_with_seed():
    a = CoulombExplosion2D_DPDM(n_ions=200, electron_temp_ev=0.01, seed=7)
    b = CoulombExplosion2D_DPDM(n_ions=200, electron_temp_ev=0.01, seed=7)
    for _ in range(60):
        a.step(dt=1e-14)
        b.step(dt=1e-14)
    assert np.allclose(a.x, b.x)
    assert np.allclose(a.vx, b.vx)