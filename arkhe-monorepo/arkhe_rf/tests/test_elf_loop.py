"""Testes do loop ELF (Schumann) — modo simulação."""

import numpy as np
import pytest

from arkhe_rf.config import ELF_FEATURE_DIM, SCHUMANN_FUNDAMENTAL_HZ
from arkhe_rf.elf_loop import ELFLoop, schumann_feature_vector_size


def test_feature_dim_invariant():
    assert schumann_feature_vector_size() == ELF_FEATURE_DIM == 20


def test_capture_length_and_dtype():
    with ELFLoop(sample_rate=250.0, seed=1) as loop:
        sig = loop.capture(duration=4.0)
        assert sig.ndim == 1
        assert sig.shape[0] == int(round(4.0 * 250.0))
        assert np.isfinite(sig).all()


def test_schumann_fundamental_detected():
    with ELFLoop(sample_rate=250.0, seed=7) as loop:
        sig = loop.capture(duration=10.0)
        feats = loop.extract_schumann_features(sig)

        assert feats["amp_fundamental"] > 0.0
        # Fundamental dominante: pico em torno de 7,83 Hz (margem de ±0,5 Hz).
        assert abs(feats["frequency_est_hz"] - SCHUMANN_FUNDAMENTAL_HZ) < 0.5
        assert feats["snr_est"] > 0.0
        assert feats["feature_vector"].shape == (ELF_FEATURE_DIM,)
        # Harmónicos presentes e decaindo: 14,3 < 7,83 em amplitude proporcional.
        assert feats["harmonics"][14.3] > 0.0


def test_harmonics_present():
    with ELFLoop(sample_rate=250.0, seed=3) as loop:
        sig = loop.capture(duration=10.0)
        feats = loop.extract_schumann_features(sig)
        amps = feats["amplitudes"]
        vals = list(amps.values())
        # Monotonicidade esperada em média (decaimento 1/h).
        assert vals[0] > vals[4]


def test_gps_annotated_timestamps():
    with ELFLoop(sample_rate=250.0, seed=5) as loop:
        t0 = 1753_000_000.0
        loop.capture(duration=2.0, gps_time=t0)
        start, end = loop.last_timestamps
        assert start == pytest.approx(t0)
        assert abs((end - start) - 2.0) < 5e-3


def test_filter_rejects_out_of_band():
    from scipy.signal import filtfilt

    loop = ELFLoop(sample_rate=250.0, seed=9)
    n = int(4.0 * loop.fs)
    tt = np.arange(n) / loop.fs

    def filtered_rms(freq):
        sig = np.sin(2 * np.pi * freq * tt)
        return float(np.sqrt(np.mean(filtfilt(loop._b, loop._a, sig) ** 2)))

    # 7,83 Hz passa; 200 Hz (alias 50 Hz) é fortemente rejeitado.
    rms_7 = filtered_rms(SCHUMANN_FUNDAMENTAL_HZ)
    rms_200 = filtered_rms(200.0)
    assert rms_200 < rms_7 / 2.0