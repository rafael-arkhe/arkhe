"""Testes do monitor de saturação ionosférica (DPDM)."""

import numpy as np
import pytest

from arkhe_rf.config import SCHUMANN_FUNDAMENTAL_HZ
from arkhe_rf.elf_loop import ELFLoop
from arkhe_rf.elf_saturation_monitor import IonosphericSaturationMonitor


def _tone(amp, fs=250.0, duration=10.0, freq=7.83):
    n = int(round(duration * fs))
    tt = np.arange(n) / fs
    return amp * np.sin(2 * np.pi * freq * tt)


def test_returns_expected_keys_and_finite_values():
    mon = IonosphericSaturationMonitor()
    res = mon.check_saturation(_tone(1.0))
    assert {"amp_fundamental", "eigenmode_amps", "harmonic_ratio",
            "saturation_score", "saturated"} <= set(res)
    assert np.isfinite(res["saturation_score"])
    assert 0.0 <= res["harmonic_ratio"]


def test_strong_fundamental_flags_saturation():
    mon = IonosphericSaturationMonitor()
    res = mon.check_saturation(_tone(8.0))
    assert res["saturated"] is True
    assert res["saturation_score"] > 2.0


def test_weak_fundamental_stays_unsaturated():
    mon = IonosphericSaturationMonitor()
    res = mon.check_saturation(_tone(0.5))
    assert res["saturated"] is False


def test_harmonic_ratio_rises_with_cavity_mode_power():
    # Um sinal com energia nos modos superiores de cavidade aumenta a razão
    # harmônica mesmo com o fundamental constante.
    fs = 250.0
    duration = 10.0
    n = int(round(duration * fs))
    tt = np.arange(n) / fs

    base = _tone(1.0, fs, duration)
    mon1 = IonosphericSaturationMonitor()
    ratio_clean = mon1.check_saturation(base)["harmonic_ratio"]

    noisy = base.copy()
    for f_h in (14.3, 20.8, 27.3, 33.8):
        noisy += 0.8 * np.sin(2 * np.pi * f_h * tt)
    mon2 = IonosphericSaturationMonitor()
    ratio_noisy = mon2.check_saturation(noisy)["harmonic_ratio"]
    assert ratio_noisy > ratio_clean


def test_sim_loop_signal_is_reported_unsaturated():
    mon = IonosphericSaturationMonitor(threshold_amp=1.0)
    with ELFLoop(sample_rate=250.0, seed=1) as loop:
        sig = loop.capture(duration=10.0)
    res = mon.check_saturation(sig)
    assert res["saturated"] is False


def test_history_is_bounded_by_window():
    mon = IonosphericSaturationMonitor(window_size=3)
    for _ in range(7):
        mon.check_saturation(_tone(1.0))
    assert len(mon.history) == 3


def test_empty_signal_is_safe():
    mon = IonosphericSaturationMonitor()
    res = mon.check_saturation(np.array([]))
    assert res["saturated"] is False
    assert res["saturation_score"] == 0.0
