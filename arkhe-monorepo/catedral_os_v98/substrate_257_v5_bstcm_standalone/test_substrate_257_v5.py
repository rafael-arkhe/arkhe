#!/usr/bin/env python3
"""
Testes automatizados para Substrato 257 v5 (pytest) — BSTCM Audit-Fixed.
"""
import numpy as np
import pytest
from substrate_257_v5_bstcm import (
    SSVEPFrequency,
    HarmonicChannel,
    SSVEPProcessor,
    STCMetasurface,
    STCCommandMapping,
    HarmonicCipherSplit,
    Substrate257BSTCM,
)


def make_signal(freq: float, fs: int = 512, duration: float = 4.0,
                snr: float = 4.0, seed: int = 0, amplitude: float = 0.8):
    np.random.seed(seed)
    t = np.linspace(0, duration, int(fs * duration))
    noise = amplitude / snr
    return np.column_stack([
        amplitude * np.sin(2 * np.pi * freq * t) + noise * np.random.randn(len(t)),
        amplitude * np.sin(2 * np.pi * freq * t + 0.2) + noise * np.random.randn(len(t)),
    ])


@pytest.fixture
def processor():
    return SSVEPProcessor()


def test_frequency_enum_values():
    assert [f.value for f in SSVEPFrequency] == [8.5, 10.0, 11.5, 7.0]


def test_harmonic_enum_values():
    assert HarmonicChannel.PLUS_2ND.value == 2
    assert HarmonicChannel.MINUS_1ST.value == -1


def test_reference_signals_2harmonics(processor):
    refs = processor._ref_signals
    assert set(refs.keys()) == set(processor.frequencies)
    for freq, ref in refs.items():
        # fundamental sin/cos + 2º harmônico sin/cos = 4 colunas
        assert ref.shape == (processor.n_samples, 4)


def test_filter_bank_returns_4_bands(processor):
    sig = make_signal(10.0)
    bands = processor._apply_filter_bank(sig)
    assert len(bands) == 4
    for band in bands:
        assert band.shape[0] == processor.n_samples
        assert band.shape[1] == 2


def test_cca_detects_true_frequency(processor):
    # FBCCA deve detectar a frequência dominante corretamente
    for target in [7.0, 8.5, 10.0, 11.5]:
        sig = make_signal(target, snr=5.0)
        freq, conf = processor._fbcca_classify(processor._apply_filter_bank(sig))
        assert freq == pytest.approx(target, abs=0.01)


def test_process_signal_valid_high_confidence(processor):
    sig = make_signal(11.5, snr=5.0)
    result = processor.process_signal(sig)
    assert result['detected_frequency'] == pytest.approx(11.5)
    assert result['command'] == 2
    assert result['status'] == 'valid'
    assert 0.0 < result['confidence'] <= 1.0


def test_process_signal_noise_rejected(processor):
    # Ruído puro não deve atingir o threshold
    np.random.seed(3)
    sig = np.random.randn(processor.n_samples, 2) * 0.05
    result = processor.process_signal(sig)
    assert result['command'] == -1
    assert result['status'] == 'noise_rejected'


def test_process_signal_dimension_validation(processor):
    with pytest.raises(AssertionError):
        processor.process_signal(np.random.randn(100))            # 1D
    with pytest.raises(AssertionError):
        processor.process_signal(np.random.randn(2000, 3))        # 3 canais
    with pytest.raises(AssertionError):
        processor.process_signal(np.random.randn(100, 2))         # poucas amostras


def test_stc_physics_computation():
    # Com T_mod=8.74ns, λ=0.043, d=0.019: sin_θ = m*0.043/(0.019*8.74e-9)
    ms = STCMetasurface()
    for h in HarmonicChannel:
        beam = ms.generate_harmonic_beam(h)
        assert beam['valid'] is True
        assert abs(beam['angle_deg']) <= 90.0
        # Fundamental -> broa normal (θ=0)
        if h == HarmonicChannel.FUNDAMENTAL:
            assert beam['angle_deg'] == pytest.approx(0.0)


def test_stc_does_not_collapse_arcsin():
    # A física corrigida NÃO deve produzir -90° por colapso de arcsin.
    ms = STCMetasurface()
    for h in list(HarmonicChannel):
        if h == HarmonicChannel.FUNDAMENTAL:
            continue
        beam = ms.generate_harmonic_beam(h)
        assert abs(beam['angle_deg']) != pytest.approx(90.0)


def test_command_mapping():
    assert STCCommandMapping.harmonic_for_command(0) == HarmonicChannel.PLUS_1ST
    assert STCCommandMapping.harmonic_for_command(3) == HarmonicChannel.MINUS_2ND
    assert STCCommandMapping.angle_for_command(1) == 30.0


def test_harmonic_cipher_roundtrip():
    plain = b"comando_SSVEP"
    nonce, cipher, key = HarmonicCipherSplit.encrypt(plain)
    assert cipher != plain
    assert len(nonce) == 12 and len(key) == 32
    h1, h2 = HarmonicCipherSplit.split_ciphertext(cipher)
    # reconexão e descriptografia
    joined = HarmonicCipherSplit.join_ciphertext(h1, h2)
    assert joined == cipher
    assert HarmonicCipherSplit.decrypt(nonce, joined, key) == plain


def test_cipher_tamper_detected():
    plain = b"segredo"
    nonce, cipher, key = HarmonicCipherSplit.encrypt(plain)
    tampered = bytearray(cipher)
    tampered[0] ^= 0xFF
    with pytest.raises(Exception):
        HarmonicCipherSplit.decrypt(nonce, bytes(tampered), key)


def test_full_substrate_pipeline():
    sub = Substrate257BSTCM()
    sig = make_signal(8.5, snr=5.0)
    out = sub.process_signal(sig)
    assert out['classification']['status'] == 'valid'
    assert out['classification']['command'] == 0
    assert out['beam'] is not None
    assert len(sub.history) == 1
    status = sub.get_status()
    assert status['n_signals'] == 1
    assert status['confidence_threshold'] == 0.3


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
