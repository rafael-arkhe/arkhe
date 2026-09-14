"""Testes do QPE com ruído de frequência de plasma (DPDM)."""

import numpy as np
import pytest

from arkhe_rf import QISKIT_OK

from arkhe_rf.qpe_estimator import (  # noqa: E402
    add_plasma_phase_noise,
    run_qpe,
    run_qpe_with_plasma_noise,
)


# ------------------------- modelo de ruído (numpy puro) -------------------

def test_noise_is_zero_for_uniform_plasma():
    n = add_plasma_phase_noise(0.25, density_perturb=0.0, n_est=6)
    assert n["phase_noise"] == 0.0
    assert n["phi_noisy"] == pytest.approx(0.25)


def test_noise_grows_with_density_perturbation():
    a = add_plasma_phase_noise(0.25, density_perturb=0.1, n_est=6)
    b = add_plasma_phase_noise(0.25, density_perturb=10.0, n_est=6)
    assert abs(b["phase_noise"]) > abs(a["phase_noise"])
    assert b["confidence"] < a["confidence"] <= 1.0


def test_k_noisy_stays_in_register_range():
    n_est = 6
    n = add_plasma_phase_noise(0.999, density_perturb=10.0, n_est=n_est)
    assert 0 <= n["k_noisy"] < (1 << n_est)


# ------------------------- circuito QPE (qiskit opcional) -----------------

@pytest.mark.skipif(not QISKIT_OK, reason="Qiskit não disponível (extra 'quantum')")
def test_run_qpe_recovers_well_aligned_phase():
    r = run_qpe(0.25, n_est=6, shots=2048)
    assert r["phi_estimated"] == pytest.approx(0.25, abs=1 / 64)
    assert r["confidence"] == 1.0
    assert r["n_est"] == 6


@pytest.mark.skipif(not QISKIT_OK, reason="Qiskit não disponível (extra 'quantum')")
def test_run_qpe_recovers_close_phase():
    r = run_qpe(0.3, n_est=5, shots=4096)
    assert r["phi_estimated"] == pytest.approx(0.3, abs=1 / 32)


@pytest.mark.skipif(not QISKIT_OK, reason="Qiskit não disponível (extra 'quantum')")
def test_plasma_noise_degrades_qpe_output():
    clean = run_qpe_with_plasma_noise(0.25, n_est=6, density_perturb=0.0)
    noisy = run_qpe_with_plasma_noise(0.25, n_est=6, density_perturb=10.0)
    assert noisy["confidence"] < clean["confidence"]
    assert "plasma_noise" in noisy
    assert noisy["density_perturb"] == 10.0
    assert noisy["phi_estimated"] != pytest.approx(clean["phi_estimated"], abs=1e-12)