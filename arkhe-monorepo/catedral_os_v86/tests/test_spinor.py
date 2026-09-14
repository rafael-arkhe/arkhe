#!/usr/bin/env python3
"""
Testes unitários para o Substrato Spinor-Cayley.

Executar: pytest tests/test_spinor.py -v
"""

import sys
import os
import numpy as np

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from substrate_spinor_cayley import (
    PauliSpinor, SIGMA_X, SIGMA_Y, SIGMA_Z, IDENTITY_2,
    cayley_propagator, random_pauli_hamiltonian,
)


class TestPauliSpinor:
    def test_initialization_default(self):
        s = PauliSpinor()
        assert abs(s.alpha - 1.0) < 1e-10
        assert abs(s.beta) < 1e-10

    def test_normalization(self):
        s = PauliSpinor(3.0, 4.0)
        norm = abs(s.alpha)**2 + abs(s.beta)**2
        assert abs(norm - 1.0) < 1e-10

    def test_coherence(self):
        s = PauliSpinor(1.0, 0.0)
        assert abs(s.coherence() - 1.0) < 1e-10

        s2 = PauliSpinor(0.0, 1.0)
        assert abs(s2.coherence() - (-1.0)) < 1e-10

    def test_coherence_superposition(self):
        s = PauliSpinor(1 / np.sqrt(2), 1 / np.sqrt(2))
        assert abs(s.coherence()) < 1e-10

    def test_phase(self):
        s = PauliSpinor(1.0, 0.0)
        assert abs(s.phase()) < 1e-10

    def test_bloch_angles(self):
        s = PauliSpinor(1.0, 0.0)
        theta, phi = s.bloch_angles()
        assert abs(theta) < 1e-10

    def test_density_matrix_trace(self):
        s = PauliSpinor(1 / np.sqrt(2), 1 / np.sqrt(2))
        rho = s.density_matrix()
        assert abs(np.trace(rho) - 1.0) < 1e-10

    def test_density_matrix_idempotent(self):
        s = PauliSpinor(0.6, 0.8)
        rho = s.density_matrix()
        rho2 = rho @ rho
        np.testing.assert_allclose(rho, rho2, atol=1e-10)

    def test_fidelity_self(self):
        s = PauliSpinor(0.6, 0.8)
        assert abs(s.fidelity_with(s) - 1.0) < 1e-10

    def test_fidelity_orthogonal(self):
        s1 = PauliSpinor(1.0, 0.0)
        s2 = PauliSpinor(0.0, 1.0)
        assert abs(s1.fidelity_with(s2)) < 1e-10

    def test_measure_z_deterministic(self):
        s = PauliSpinor(1.0, 0.0)
        result, new_s = s.measure('z', u=0.0)
        assert result == 1
        assert abs(new_s.coherence() - 1.0) < 1e-10

    def test_measure_z_other(self):
        s = PauliSpinor(0.0, 1.0)
        result, new_s = s.measure('z', u=0.99)
        assert result == -1

    def test_cayley_identity(self):
        H = np.zeros((2, 2), dtype=complex)
        s = PauliSpinor(0.6, 0.8)
        s_new = s.cayley_step(H, dt=0.1)
        assert abs(s_new.alpha - s.alpha) < 1e-10
        assert abs(s_new.beta - s.beta) < 1e-10

    def test_cayley_preserves_norm(self):
        H = 0.5 * SIGMA_Z
        s = PauliSpinor(0.6, 0.8)
        s_new = s.cayley_step(H, dt=0.01)
        norm = abs(s_new.alpha)**2 + abs(s_new.beta)**2
        assert abs(norm - 1.0) < 1e-8

    def test_cayley_unitary_property(self):
        H = 1.5 * SIGMA_X + 0.3 * SIGMA_Y
        dt = 0.05
        U = cayley_propagator(H, dt)
        product = U @ U.conj().T
        np.testing.assert_allclose(product, IDENTITY_2, atol=1e-10)

    def test_apply_pauli_x(self):
        s = PauliSpinor(1.0, 0.0)
        s_x = s.apply_pauli('x')
        assert abs(s_x.coherence() - (-1.0)) < 1e-10

    def test_to_dict_roundtrip(self):
        s = PauliSpinor(0.7, 0.7j)
        d = s.to_dict()
        s2 = PauliSpinor.from_dict(d)
        assert abs(s.alpha - s2.alpha) < 1e-10
        assert abs(s.beta - s2.beta) < 1e-10


class TestCayleyPropagator:
    def test_identity_hamiltonian(self):
        H = np.zeros((2, 2), dtype=complex)
        U = cayley_propagator(H, dt=1.0)
        np.testing.assert_allclose(U, IDENTITY_2, atol=1e-10)

    def test_unitary_for_pauli_hamiltonian(self):
        for _ in range(10):
            H = random_pauli_hamiltonian()
            dt = np.random.uniform(0.001, 0.1)
            U = cayley_propagator(H, dt)
            product = U @ U.conj().T
            np.testing.assert_allclose(product, IDENTITY_2, atol=1e-8)
