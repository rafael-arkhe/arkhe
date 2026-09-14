"""Testes do caminho paralelo Fase 3 — QKM/VQC (qiskit opcional)."""

import numpy as np
import pytest

from arkhe_rf import QISKIT_OK

if not QISKIT_OK:
    pytest.skip("Qiskit não disponível (extra 'quantum')", allow_module_level=True)

from arkhe_rf.quantum_pipeline import (  # noqa: E402
    QuantumKernel,
    QuantumSVMPipeline,
    VQClassifier,
    compare_quantum_classical,
)


def _labeled_binary(n=240, dim=8, seed=0):
    rng = np.random.default_rng(seed)
    X = rng.standard_normal((n, dim))
    y = (X[:, 0] + 0.8 * X[:, 1] > 0).astype(int)
    return X, y


def test_kernel_symmetric_and_psd():
    X, _ = _labeled_binary(n=12, dim=4)
    k = QuantumKernel(n_qubits=4, reps=1, seed=1)
    K = k.kernel(X)
    assert K.shape == (12, 12)
    assert np.allclose(K, K.T)
    assert np.allclose(K, K.T.conj())
    diag = np.diag(K)
    assert np.all(diag > 1.0 - 1e-9)
    assert np.allclose(diag, 1.0, atol=1e-9)
    # Fidelidade |<φ|φ>|² = 1 no estado puro.
    assert np.all(np.abs(np.diag(K) - 1.0) < 1e-8)


def test_kernel_rejects_wrong_dimension():
    k = QuantumKernel(n_qubits=4, seed=1)
    with pytest.raises(ValueError):
        k._state(np.zeros(3))


def test_qkm_fit_predict_and_report():
    X, y = _labeled_binary(n=120)
    q = QuantumSVMPipeline(n_qubits=4, reps=1, seed=2)
    q.fit(X, y)
    assert q.train_emb_ is not None
    assert q.train_emb_.shape == (120, 4)
    acc = q.score(X, y)
    assert 0.5 <= acc <= 1.0
    rep = q.report()
    assert rep["n_qubits"] == 4
    assert "disclaimer" in rep
    assert rep["kernel_spectrum_min"] > 0.0  # Gram PSD (numérico)


def test_qkm_requires_fit_for_predict():
    q = QuantumSVMPipeline(n_qubits=4, seed=3)
    X, _ = _labeled_binary(n=8)
    with pytest.raises(RuntimeError):
        q.predict(X)


def test_vqc_fit_predict():
    X, y = _labeled_binary(n=48, dim=4)
    v = VQClassifier(n_qubits=3, reps=1, seed=4)
    res = v.fit(X, y, maxiter=60)
    assert "fun" in res and "success" in res
    assert v.fitted_
    pred = v.predict(X)
    assert pred.shape == (48,)
    assert set(np.unique(pred)).issubset({0, 1})
    assert v.score(X, y) >= 0.5


def test_vqc_requires_fit_for_predict():
    v = VQClassifier(n_qubits=3, seed=5)
    X, _ = _labeled_binary(n=4, dim=3)
    with pytest.raises(RuntimeError):
        v.predict(X)


def test_compare_honest_declares_regime():
    X, y = _labeled_binary(n=240)
    rep = compare_quantum_classical(X, y, n_qubits=4, seed=6)
    assert {"acc_quantum_qkm", "acc_classical_rbf", "delta"} <= set(rep)
    assert rep["dataset_regime"].startswith("sintético")
    assert "NÃO constitui" in rep["conclusion"]
    assert 0.0 <= rep["acc_quantum_qkm"] <= 1.0
    assert 0.0 <= rep["acc_classical_rbf"] <= 1.0
    assert rep["n_qubits"] == 4


def test_compare_accepts_rf_elf_shape():
    # Mesmo formato de entrada das Fases 1–2 (68+20 = 88 features).
    rng = np.random.default_rng(9)
    X = rng.standard_normal((120, 88))
    y = (X[:, 0] + X[:, 87] > 0).astype(int)
    rep = compare_quantum_classical(X, y, n_qubits=4, seed=9)
    assert rep["n_qubits"] == 4
