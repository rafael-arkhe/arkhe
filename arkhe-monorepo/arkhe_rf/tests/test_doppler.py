"""Testes de física Doppler e do rastreador."""

import numpy as np
import pytest

from arkhe_rf.doppler import (
    DopplerTracker,
    doppler_shift_iq,
    estimate_leo_doppler_max,
    validate_doppler_claim,
)
from arkhe_rf.features import compute_spectrum, extract_features


def test_leo_orbital_physics():
    est = estimate_leo_doppler_max(1992.5e6)
    # v_orbital a 550 km ≈ 7589 m/s
    assert 7400 < est["v_orbital_m_s"] < 7800
    # Δf_max a ~1,99 GHz ≈ ±50,5 kHz (não 254 kHz!)
    assert 45e3 < est["delta_f_max_hz"] < 56e3
    assert est["delta_f_max_khz"] == pytest.approx(50.5, rel=0.15)
    # taxa típica ~0,055 kHz/s (ordem da fórmula física, não kHz/s)
    assert 0.02 < est["doppler_rate_khz_per_s"] < 0.1


def test_validate_claim_pcs_is_discrepant():
    val = validate_doppler_claim(254.6, 1992.5e6)
    assert val["ratio"] > 3.0
    assert "DISCREPANTE" in val["verdict"]


def test_validate_claim_ku_is_consistent():
    # 254,6 kHz é Doppler de banda Ku (~10 GHz), não da banda PCS DTC.
    val = validate_doppler_claim(254.6, 10.0e9)
    assert val["verdict"] == "CONSISTENTE"


def make_leo_trajectory(dmax=50e3, tau=50.0, t0=-40.0):
    t = []
    f = []
    for k in range(80):
        ta = t0 + k  # 1s por frame
        t.append(ta)
        f.append(dmax * np.tanh((ta - 30.0) / tau))
    return np.array(t), np.array(f)


def test_tracker_detects_leo_pass():
    t, f = make_leo_trajectory()
    tracker = DopplerTracker(
        window_size=40, slope_min_hz=500.0, slope_max_hz=5000.0, r_squared_min=0.7
    )
    states = [tracker.update(float(fi), float(ti)) for fi, ti in zip(f, t)]
    good = [s for s in states if s.is_satellite]
    assert good, "nenhum frame classificado como satélite"
    best = max(good, key=lambda s: s.r_squared)
    assert best.confidence in ("PROVAVEL", "SATELITE")
    assert 0.5 < abs(best.slope_khz_per_s) < 5.0


def test_tracker_labels_static_signal():
    tracker = DopplerTracker(window_size=60, slope_min_hz=500.0, slope_max_hz=5000.0)
    states = [tracker.update(0.0, float(k)) for k in range(30)]
    assert not states[-1].is_satellite
    assert states[-1].confidence == "ESTATICO"


def test_doppler_shift_moves_peak():
    sr = 10e6
    n = int(sr * 0.02)
    tt = np.arange(n) / sr
    tone = 100e3
    iq = 0.5 * np.exp(2j * np.pi * tone * tt)
    iq += 0.01 * (np.random.default_rng(0).standard_normal(n)
                  + 1j * np.random.default_rng(1).standard_normal(n))

    f0 = extract_features(iq, sample_rate=sr, center_freq=1992.5e6)
    shifted = doppler_shift_iq(iq, tone, sr)  # puxa o tom para ~DC
    f1 = extract_features(shifted, sample_rate=sr, center_freq=1992.5e6)

    assert abs(f0["peak_freq_offset"] - tone) < 5e3
    assert abs(f1["peak_freq_offset"]) < 5e3


def test_compute_spectrum_centered():
    sr = 10e6
    n = int(sr * 0.01)
    tt = np.arange(n) / sr
    iq = np.exp(2j * np.pi * 0 * tt)  # DC
    freqs, psd = compute_spectrum(iq, sr, fft_size=1024)
    assert psd.shape[0] == 1024
    binw = sr / 1024.0
    assert freqs[0] == pytest.approx(-sr / 2.0)
    assert freqs[-1] == pytest.approx(sr / 2.0 - binw)
    assert np.isclose(np.max(freqs), sr / 2.0 - binw)