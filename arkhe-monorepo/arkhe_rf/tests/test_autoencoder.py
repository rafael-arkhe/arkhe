"""Testes do autoencoder multi-modal e detecção de anomalias."""

import numpy as np
import pytest

from arkhe_rf import TORCH_OK
from arkhe_rf.autoencoder import anomaly_event

if not TORCH_OK:
    pytest.skip("PyTorch não disponível (extra 'ml')", allow_module_level=True)

import torch  # noqa: E402

from arkhe_rf.autoencoder import (  # noqa: E402
    MultiModalAutoencoder,
    ReconstructionAnomalyDetector,
    train_autoencoder,
)


def _normal_batch(n, rf_dim=68, elf_dim=20, seed=0):
    rng = np.random.default_rng(seed)
    rf = rng.standard_normal((n, rf_dim))
    elf = 0.3 * rng.standard_normal((n, elf_dim))
    return rf, elf


def test_forward_shapes():
    model = MultiModalAutoencoder()
    rf = torch.randn(8, 68)
    elf = torch.randn(8, 20)
    rf_out, elf_out = model(rf, elf)
    assert tuple(rf_out.shape) == (8, 68)
    assert tuple(elf_out.shape) == (8, 20)
    assert tuple(model.encode(rf, elf).shape) == (8, model.latent_dim)


def test_training_reduces_loss():
    N = 96
    rf, elf = _normal_batch(N)
    t_rf = torch.from_numpy(rf.astype(np.float32))
    t_elf = torch.from_numpy(elf.astype(np.float32))
    model = MultiModalAutoencoder()
    first = float(model.reconstruction_loss(t_rf, t_elf).item())
    history = train_autoencoder(model, rf, elf, epochs=120, batch_size=24, lr=1e-3)
    last = history[max(history)]
    assert last < first * 0.6
    assert len(history) == 120


def test_detector_calibrates_on_normal():
    rf, elf = _normal_batch(160, seed=1)
    det = ReconstructionAnomalyDetector(sigma=3.0)
    det.fit(rf, elf, epochs=120, batch_size=24)
    assert det.fitted_
    # Novo lote da mesma distribuição: pouquíssimas anomalias espúrias.
    rf2, elf2 = _normal_batch(160, seed=2)
    res = det.detect(rf2, elf2)
    rate = float(res["is_anomaly"].mean())
    assert rate < 0.10


def test_detector_raises_before_fit():
    det = ReconstructionAnomalyDetector()
    rf, elf = _normal_batch(4)
    with pytest.raises(RuntimeError):
        det.detect(rf, elf)


def test_outlier_is_flagged():
    rf, elf = _normal_batch(120, seed=3)
    det = ReconstructionAnomalyDetector(sigma=2.0)
    det.fit(rf, elf, epochs=100, batch_size=24)
    # Injeta desvio forte num componente — deve ultrapassar μ+2σ.
    outlier_rf = rf[:1].copy()
    outlier_rf[:, 0:20] += 30.0
    outlier_elf = elf[:1].copy()
    res_out = det.detect(outlier_rf, outlier_elf)
    assert res_out["is_anomaly"][0]

    res_n = det.detect(rf[:1], elf[:1])
    assert not res_n["is_anomaly"][0]


def test_anomaly_event_schema_and_list():
    det = ReconstructionAnomalyDetector(sigma=3.0)
    rf, elf = _normal_batch(60, seed=5)
    det.fit(rf, elf, epochs=60, batch_size=16)
    events = det.to_arkhe_event(rf[:3], elf[:3], timestamps=np.arange(3) + 1_700_000_000.0)
    assert len(events) == 3
    ev = events[0]
    assert ev["kind"] == "TEMPORAL_ANOMALY"
    assert ev["channels"] == ["RF", "ELF"]
    ev2 = anomaly_event(total_err=0.9, threshold=0.5, sigma=3.0, ts=2.0)
    assert ev2["exceeds_by_sigma"] > 0.0