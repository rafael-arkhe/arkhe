"""Testes do DPDM reference (fóton escuro como benchmark)."""

import numpy as np
import pytest

from arkhe_rf.dpdm_reference import DPDMReference


def test_mass_to_frequency_mapping():
    # m = 1e-6 eV → f = m·c²/h ≈ 241 MHz.
    dpdm = DPDMReference(mass_ev=1e-6)
    assert dpdm.frequency_hz / 1e6 == pytest.approx(241.0, rel=0.02)
    assert dpdm.period_s > 0.0


def test_frequency_grows_with_mass():
    light = DPDMReference(mass_ev=1e-6)
    heavy = DPDMReference(mass_ev=1e-4)
    assert heavy.frequency_hz > light.frequency_hz


def test_plasma_frequency_si_scale():
    # 1e6 cm⁻³ → f_p ≈ 8,98 MHz.
    dpdm = DPDMReference()
    f_p = dpdm.plasma_frequency(density_cm3=1e6)
    assert f_p / 1e6 == pytest.approx(8.98, rel=0.02)


def test_conversion_efficiency_degrades_with_perturbation():
    dpdm = DPDMReference()
    eff_clean = dpdm.conversion_efficiency(density_perturb=0.0)
    eff_noisy = dpdm.conversion_efficiency(density_perturb=1.0)
    assert eff_clean == pytest.approx(1.0)
    assert eff_noisy < eff_clean
    assert dpdm.conversion_efficiency(density_perturb=1.0) < 0.5


def test_generate_signal_shape_and_normalization():
    dpdm = DPDMReference(mass_ev=1e-6)
    sig = dpdm.generate_signal(duration_s=2.0, fs=100.0)
    assert len(sig["time"]) == 200
    assert len(sig["signal"]) == 200
    assert np.max(np.abs(sig["signal"])) == pytest.approx(1.0)
    assert sig["frequency"] == dpdm.frequency_hz
    assert np.isfinite(sig["signal"]).all()


def test_generate_signal_saturation_flag():
    dpdm = DPDMReference(mass_ev=1e-6)
    clean = dpdm.generate_signal(density_perturb=0.0)
    saturated = dpdm.generate_signal(density_perturb=10.0)
    assert clean["is_saturated"] is False
    assert saturated["is_saturated"] is True
    assert saturated["density_perturb"] == 10.0
