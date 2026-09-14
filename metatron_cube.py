# -*- coding: utf-8 -*-
"""metatron_cube.py — espelho hermético (host, numpy) do metatron_unitary_kernel.c

BLOCO 507 — Alicerce Metatrônico: Cubo de Metatron como evolução unitária
de um vetor de estado de 13 dimensões sobre o grafo completo K13
(78 = C(13,2) conexões, grau 12). 1:1 com o kernel C (mesmos resultados).

Regras da casa: heraético, padrão-padrão, sem rede, sem jax/redis/TGN.
A costura com o Observador entrega apenas o CoherenceState REAL de
coherence.h (phi_c, phi_delta, ratio, entropy).
"""

import numpy as np

DIM = 13
DEGREE = 12.0
DT = 0.001
GAMMA_B = 255.0


def build_hamiltonian():
    """H = I + A/DEGREE, A = adjacência do grafo completo K13."""
    adjacency = np.ones((DIM, DIM)) - np.eye(DIM)
    return np.eye(DIM) + adjacency / DEGREE


class MetatronCube:
    def __init__(self, dt=DT, gamma_b=GAMMA_B):
        H = build_hamiltonian() + gamma_b * np.eye(DIM)
        half = 0.5 * dt * H
        temp_a = np.eye(DIM, dtype=np.complex128) - 1j * half
        temp_b = np.eye(DIM, dtype=np.complex128) + 1j * half
        self.U = np.linalg.inv(temp_a) @ temp_b
        self.dt = float(dt)
        self.gamma_b = float(gamma_b)
        self.psi = np.zeros(DIM, dtype=np.complex128)
        self.psi[0] = 1.0
        self.steps = 0

    def evolve(self, k=1):
        for _ in range(k):
            self.psi = self.U @ self.psi
        self.steps += k
        return self

    def place_unit_state(self, first_mag):
        assert 0.0 <= first_mag <= 1.0
        psi = np.zeros(DIM, dtype=np.complex128)
        psi[0] = np.sqrt(first_mag)
        psi[1] = np.sqrt(1.0 - first_mag)
        self.psi = psi
        self.steps = 0
        return self

    @property
    def handover(self):
        return float(np.abs(self.psi[0]) ** 2)

    @property
    def norm(self):
        return float(np.vdot(self.psi, self.psi).real)

    @property
    def unitarity_error(self):
        return float(np.max(np.abs(self.U.conj().T @ self.U - np.eye(DIM))))

    def to_coherence(self):
        hv = min(1.0, max(0.0, self.handover))
        return {
            "phi_c": hv,
            "phi_delta": 1.0 - hv,
            "ratio": hv,
            "entropy": 1.0 - hv,
        }

    def __repr__(self):
        return (
            f"MetatronCube(handover={self.handover:.15f}, "
            f"norm={self.norm:.15f}, steps={self.steps})"
        )


REFERENCE = 0.926693032450800  # handover C após 1000 passos (WSL, -O2/ASan)


def selftest():
    print("=== METATRON SELFTEST (K13, dt=0.001, gamma_b=255.0) ===")

    cube = MetatronCube()
    uerr = cube.unitarity_error
    print(f"[MET] unitarity_error(13) = {uerr:.3e} (tol 1e-9)")
    assert uerr < 1e-9

    cube.evolve(1000)
    norm = cube.norm
    hv = cube.handover
    print(f"[MET] norm_after_1000_steps = {norm:.15f}")
    print(f"[MET] handover_after_1000_steps = {hv:.15f}")
    print(f"[MET] C/host agreement            = {abs(hv - REFERENCE):.3e}")
    assert abs(norm - 1.0) < 1e-9
    assert 0.0 <= hv <= 1.0
    assert abs(hv - REFERENCE) < 1e-9

    a = MetatronCube().evolve(10)
    b = MetatronCube().evolve(10)
    assert np.array_equal(a.psi, b.psi), "deterministic evolution violated"
    assert a.handover == b.handover

    cube.place_unit_state(0.2)
    assert abs(cube.handover - 0.2) < 1e-12
    assert abs(cube.norm - 1.0) < 1e-12

    coh = cube.to_coherence()
    assert set(coh) == {"phi_c", "phi_delta", "ratio", "entropy"}
    assert abs(coh["phi_c"] - 0.2) < 1e-12
    assert abs(coh["phi_delta"] - 0.8) < 1e-12
    assert abs(coh["entropy"] - 0.8) < 1e-12

    print("=== METATRON SELFTEST PASSED ===")


if __name__ == "__main__":
    selftest()