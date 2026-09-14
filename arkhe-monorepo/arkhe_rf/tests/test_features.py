"""Testes de extração de features espectrais."""

import numpy as np

from arkhe_rf.features import (
    compute_spectrum,
    extract_features,
    feature_vector_size,
)


def test_feature_vector_size_invariant():
    assert feature_vector_size() == 68


def test_extract_features_tone():
    sr = 10e6
    n = int(sr * 0.01)
    tt = np.arange(n) / sr
    iq = 0.5 * np.exp(2j * np.pi * 250e3 * tt)
    iq += 0.01 * (np.random.default_rng(3).standard_normal(n)
                  + 1j * np.random.default_rng(4).standard_normal(n))

    feat = extract_features(iq, sample_rate=sr, center_freq=1992.5e6)

    assert abs(feat["peak_freq"] - 1992.75e6) < 10e3
    assert abs(feat["peak_freq_offset"] - 250e3) < 10e3
    assert feat["snr_db"] > 10.0
    assert feat["num_peaks"] >= 1
    assert len(feat["all_peaks"]) == feat["num_peaks"]
    assert feat["feature_vector"].shape == (68,)
    for key in ("snr_db", "peak_freq", "peak_freq_offset", "peak_power_db",
                "noise_floor_db", "num_peaks", "all_peaks", "band_power",
                "feature_vector", "psd_db", "freqs"):
        assert key in feat
    assert set(feat["band_power"]) >= {"total_db", "subbands_db",
                                       "band_flatness_db", "noise_floor_db"}


def test_extract_features_noise_floor():
    rng = np.random.default_rng(5)
    sr = 2e6
    n = int(sr * 0.05)
    iq = 0.01 * (rng.standard_normal(n) + 1j * rng.standard_normal(n))
    feat = extract_features(iq, sample_rate=sr, center_freq=1912.5e6)
    # ruído puro: SNR baixo e nenhum pico forte
    assert feat["snr_db"] < 25.0
    assert feat["num_peaks"] <= 2
    assert feat["feature_vector"].shape == (68,)


def test_spectrum_semantic_consistency():
    sr = 4e6
    n = int(sr * 0.02)
    tt = np.arange(n) / sr
    iq = 0.3 * np.exp(2j * np.pi * (-800e3) * tt)
    freqs, psd = compute_spectrum(iq, sr, fft_size=512)
    peak_bin = int(np.argmax(psd))
    assert abs(freqs[peak_bin] + 800e3) < sr / 512.0 * 2