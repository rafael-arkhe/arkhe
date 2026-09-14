"""Testes de captura (modo simulação) e do protocolo de caça completo."""

import numpy as np
import pytest

from arkhe_rf.capture import AntSDRCapture
from arkhe_rf.config import DETECTION, STARLINK_DTC_TARGETS
from arkhe_rf.doppler import DopplerTracker
from arkhe_rf.features import extract_features


def test_sim_capture_shape_and_dtype():
    with AntSDRCapture(
        center_freq=1992.5e6, sample_rate=1e6, duration=0.02, seed=7
    ) as sdr:
        assert sdr.mode == "sim"
        iq = sdr.capture()
        assert iq.dtype == np.complex64
        assert iq.shape == (int(1e6 * 0.02),)


def test_sim_capture_deterministic_with_seed():
    a = AntSDRCapture(center_freq=1912.5e6, sample_rate=5e5, duration=0.01, seed=42)
    b = AntSDRCapture(center_freq=1912.5e6, sample_rate=5e5, duration=0.01, seed=42)
    try:
        x = a.capture()
        y = b.capture()
        assert np.array_equal(x, y)
    finally:
        a.close()
        b.close()


def test_sim_reports_leo_signature():
    """A passagem sintética deve parecer satélite (SNR alto + multi-pico)."""
    with AntSDRCapture(
        center_freq=1992.5e6, sample_rate=1e6, duration=0.05, seed=11
    ) as sdr:
        iq = sdr.capture()
        feat = extract_features(iq, sample_rate=sdr.sample_rate,
                                center_freq=sdr.center_freq)
        assert feat["snr_db"] > DETECTION["snr_threshold_db"]
        assert feat["num_peaks"] >= DETECTION["min_peaks"]


def _hunt_once(sample_rate=1e6, duration=0.05, dwell_s=15.0, seed=3):
    """Monitora o downlink PCS até confirmar detecção LEO."""
    target = next(t for t in STARLINK_DTC_TARGETS if t["name"] == "DOWNLINK_PCS")
    tracker = DopplerTracker(
        window_size=60,
        slope_min_hz=500.0,
        slope_max_hz=5000.0,
        r_squared_min=0.7,
    )
    confirm = 0
    frames = 0
    confident = None
    with AntSDRCapture(
        center_freq=target["freq"],
        sample_rate=sample_rate,
        duration=duration,
        seed=seed,
    ) as sdr:
        start = np.datetime64("now").astype(np.int64) / 1e9
        while (np.datetime64("now").astype(np.int64) / 1e9 - start) < dwell_s:
            iq = sdr.capture()
            feat = extract_features(
                iq, sample_rate=sdr.sample_rate, center_freq=sdr.center_freq
            )
            state = tracker.update(
                feat["peak_freq"] - sdr.center_freq,
                float(frames) * duration,
            )
            frames += 1
            if (feat["snr_db"] > DETECTION["snr_threshold_db"]
                    and feat["num_peaks"] >= DETECTION["min_peaks"]
                    and state.is_satellite):
                confirm += 1
                confident = state
            else:
                confirm = max(0, confirm - 1)
            if confirm >= DETECTION["diffuse"]["frames_to_confirm"]:
                return {"frames": frames, "state": confident,
                        "snr": feat["snr_db"], "num_peaks": feat["num_peaks"]}
    return None


def test_hunt_protocol_confirms_detection():
    result = _hunt_once()
    assert result is not None, "protocolo não confirmou a passagem LEO simulada"
    assert result["state"].is_satellite
    assert result["state"].confidence in ("PROVAVEL", "SATELITE")
    assert result["snr"] > DETECTION["snr_threshold_db"]
    assert result["num_peaks"] >= DETECTION["min_peaks"]