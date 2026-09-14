#!/usr/bin/env python3
"""
Testes automatizados para Substrato 257 v4 (pytest) — BSTCM Unified Integration.
"""
import numpy as np
import pytest
from substrate_257_v4_unified import (
    SSVEPFrequency,
    HarmonicChannel,
    SSVEPProcessor,
    STCMetasurface,
    FSOBridge,
    Substrate257Unified,
)


@pytest.fixture
def ssvep():
    return SSVEPProcessor()


@pytest.fixture
def signal():
    fs, duration = 512, 4
    t = np.linspace(0, duration, int(fs * duration))
    return np.column_stack([
        0.9 * np.sin(2 * np.pi * 10.0 * t) + 0.3 * np.random.randn(len(t)),
        0.9 * np.sin(2 * np.pi * 10.0 * t + 0.2) + 0.3 * np.random.randn(len(t)),
    ])


def test_frequency_enum():
    assert [f.value for f in SSVEPFrequency] == [8.5, 10.0, 11.5, 7.0]


def test_harmonic_enum():
    assert HarmonicChannel.PLUS_1ST.value == 1
    assert HarmonicChannel.MINUS_2ND.value == -2


def test_reference_signals(ssvep):
    refs = ssvep._ref_signals
    assert set(refs.keys()) == set(ssvep.frequencies)
    for freq, ref in refs.items():
        # 3 harmônicos × (sen+cos) = 6 colunas
        assert ref.shape == (ssvep.n_samples, 6)


def test_peak_detect(ssvep):
    freqs = np.fft.rfftfreq(ssvep.n_samples, 1 / ssvep.sampling_rate)
    spec = np.zeros(len(freqs))
    target = 10.0
    idx = np.argmin(np.abs(freqs - target))
    spec[idx] = 5.0
    freq, conf = ssvep._peak_detect(spec, freqs)
    assert freq == 10.0
    assert 0.0 < conf <= 1.0


def test_process_signal_returns_plausible(signal, ssvep):
    result = ssvep.process_signal(signal)
    assert result['detected_frequency'] in ssvep.frequencies
    assert result['command'] in (0, 1, 2, 3)
    assert isinstance(result['sig_spec'], list)


def test_stc_matrix_validation():
    ms = STCMetasurface()
    with pytest.raises(ValueError):
        ms.apply_stc_matrix("bad", np.zeros((8, 8)).astype(np.uint8),
                            HarmonicChannel.FUNDAMENTAL, 0.0)


def test_stc_apply_and_expand():
    ms = STCMetasurface()
    mat = np.random.randint(0, 2, (16, 16)).astype(np.uint8)
    stc = ms.apply_stc_matrix("M0", mat, HarmonicChannel.PLUS_1ST, -15.0)
    assert ms.pin_diodes.shape == (32, 32)  # kron(2x2) expand
    assert stc.time_intervals == 11
    assert ms.stc_matrices["M0"] is stc


def test_set_leds():
    ms = STCMetasurface()
    ms.set_leds((0, 1), 1550.0)
    # região (linha 0..16, col 16..32)
    assert ms.led_frequencies[0, 16] == 1550.0
    assert ms.led_frequencies[15, 31] == 1550.0
    assert ms.led_frequencies[16, 16] == 0.0  # fora da região


def test_harmonic_beam():
    ms = STCMetasurface()
    result = ms.generate_harmonic_beam(HarmonicChannel.MINUS_2ND)
    assert result['angle_deg'] == 10.0
    assert result['harmonic'] == -2


def test_rsi_optimize_collects_then_optimizes():
    ms = STCMetasurface()
    r1 = ms.rsi_optimize(0.5)
    assert r1['status'] == 'collecting'
    ms.rsi_optimize(0.6)
    r3 = ms.rsi_optimize(0.62)  # melhoria
    assert r3['status'] == 'optimized'
    assert r3['improvement'] > 0
    assert ms._rsi_fitness_history == [0.5, 0.6, 0.62]


def test_fso_hybrid_transmit():
    fso = FSOBridge()
    data = b"command_3"
    result = fso.hybrid_transmit(data, HarmonicChannel.MINUS_2ND, {})
    assert result['hybrid_transmission'] is True
    assert result['total_size'] == len(data)
    assert result['rf']['data_size'] == len(data) // 2
    assert result['optical']['wavelength'] == 1550
    assert len(result['optical']['qkd_key']) == 16


def test_coherence_measurement():
    fso = FSOBridge()
    c = fso.measure_communication_coherence(30.0, 30.0)
    assert c == pytest.approx(1.0)
    c_low = fso.measure_communication_coherence(0.0, 0.0)
    assert c_low == pytest.approx(0.0)
    assert 0.0 <= fso.measure_communication_coherence(22.0, 18.0) <= 1.0


def test_unified_pipeline(signal):
    class MockDIMeter:
        def measure_communication_coherence(self, rf, opt):
            return 0.5 + 0.5 * (rf / 30.0 + opt / 30.0) / 2

    substrate = Substrate257Unified(di_meter=MockDIMeter())
    result = substrate.process_and_evolve(signal, validation_score=0.85)
    assert result['classification']['command'] in (0, 1, 2, 3)
    assert result['stc_angle'] in (-15, 30, -45, 10)
    assert result['stc_harmonic'] in (-2, -1, 1, 2)
    assert result['coherence'] is not None
    assert len(substrate.history) == 1
    assert result['rsi']['status'] == 'collecting'


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
