# -*- coding: utf-8 -*-
"""aqec_petz.py — Mapa de recuperação de Petz em NumPy puro.

BLOCO 509 v69 (vetado) — C9:
  * Formula real do mapa de Petz com sigma = I (transpose channel):
        P(rho) = Soma_k  E_k† G^{-1/2} rho G^{-1/2} E_k,   G = Soma E E†.
  * G^{-1/2} por decomposicao espectral (Hermitiana PSD) — o simbolo
    inventado "np.linalg.sqrtm" da proposta (inexistente em NumPy) foi
    substituido por eigendecomposicao deterministica, sem scipy.
  * Invariantes VERIFICADOS numericamente (nao assumidos):
      - preserva traco: tr(P(rho)) = tr(rho) a ~1e-15 (200 estados testados);
      - preserva hermiticidade e positividade (CPTP no dominio de densidade);
      - determinismo.
  * NAO e afirmado (a proposta I33 alegava "coherence >= 0.618"): o mapa
    sigma=I NAO garante aumento de fidelidade para estados puros; so a
    propriedade de preservacao e honesta.
"""

import numpy as np


def amplitude_damping_kraus(p: float):
    E0 = np.array([[1.0, 0.0], [0.0, np.sqrt(1.0 - p)]], dtype=complex)
    E1 = np.array([[0.0, np.sqrt(p)], [0.0, 0.0]], dtype=complex)
    return [E0, E1]


def hermitian_inverse_sqrt(M: np.ndarray, eps: float = 1e-12) -> np.ndarray:
    """M^{-1/2} para M Hermitiana PSD, por eigendecomposicao (sem scippy)."""
    w, V = np.linalg.eigh(M)
    wc = np.maximum(w.real, eps)
    return (V @ np.diag(1.0 / np.sqrt(wc)) @ V.conj().T)


class PetzRecovery:
    """Mapa de recuperacao de Petz com estado de referencia sigma = I."""

    def __init__(self, kraus):
        self.kraus = list(kraus)
        self.gamma = sum(E @ E.conj().T for E in self.kraus)
        self.gamma_inv_sqrt = hermitian_inverse_sqrt(self.gamma)

    def apply(self, rho: np.ndarray) -> np.ndarray:
        out = np.zeros_like(rho, dtype=complex)
        for E in self.kraus:
            R = E.conj().T @ self.gamma_inv_sqrt
            out += R @ rho @ R.conj().T
        return out

    def channel(self, rho: np.ndarray) -> np.ndarray:
        out = np.zeros_like(rho, dtype=complex)
        for E in self.kraus:
            out += E @ rho @ E.conj().T
        return out


def selftest() -> None:
    print("=== AQEC SELFTEST (Petz sigma=I, NumPy puro) ===")
    petz = PetzRecovery(amplitude_damping_kraus(0.3))
    g = petz.gamma.real
    print(f"[AQ] gamma = diag({g[0, 0]:.4f}, {g[1, 1]:.4f})")

    gamma_psd = np.min(np.linalg.eigvalsh(petz.gamma).real)
    assert gamma_psd > 0.0, "gamma must be positive definite"
    print(f"[AQ] gamma PSD (min eig {gamma_psd:.3e})")

    rng = np.random.default_rng(20260901)
    worst_trace = 0.0
    worst_herm = 0.0
    worst_pos = 0.0

    for _ in range(200):
        r0 = rng.random((2, 2)) + 1j * rng.random((2, 2))
        rho = r0 @ r0.conj().T
        rho = rho / np.trace(rho).real

        pur = petz.apply(rho)
        worst_trace = max(worst_trace, abs(np.trace(pur) - np.trace(rho)))
        worst_herm = max(worst_herm, np.max(np.abs(pur - pur.conj().T)))
        worst_pos = max(worst_pos, -float(min(np.linalg.eigvalsh((pur + pur.conj().T) / 2).real)))

    print(f"[AQ] worst trace deviation  = {worst_trace:.3e}  (tol 1e-9)")
    print(f"[AQ] worst hermitian error  = {worst_herm:.3e}")
    print(f"[AQ] worst positivity leak  = {worst_pos:.3e}")
    assert worst_trace < 1e-9
    assert worst_herm < 1e-9
    assert worst_pos < 1e-9

    ref = np.array([[0.6, 0.1 + 0.2j], [0.1 - 0.2j, 0.4]], dtype=complex)
    a = petz.apply(ref)
    b = petz.apply(ref)
    assert np.array_equal(a, b), "recovery must be deterministic"
    print("[AQ] deterministic (bit-identical twice)")

    print("=== AQEC SELFTEST PASSED ===")


if __name__ == "__main__":
    selftest()