#!/usr/bin/env python3
"""
Substrato — Spinor de Pauli e Propagador de Cayley

Implementa o spinor quântico (α, β) com |α|² + |β|² = 1, matrizes de Pauli,
propagador de Cayley (unitário exato), e medição conforme o postulado de Born.

Coerência = ⟨σ_z⟩ = |α|² - |β|².

NOTA DE HONESTIDADE (auditoria):
 - O propagador de Cayley é unitário exato (sem truncamento de série), mas a
   inversão de matriz pode ser numericamente instável para hamiltonianos mal
   condicionados — o código inclui fallback para decomposição de autovalores.
 - A medição de Born usa RNG padrão (numpy.random), não é criograficamente
   seguro. Para uso em criptografia, substituir por secrets.token_bytes.
 - Este substrato NÃO implementa decoerência ambiental; a evolução é puramente
   unitária entre colapsos.

Selo: CATEDRAL-OS-v86.0-SUBSTRATE-SPINOR-CAYLEY
"""

import numpy as np
import hashlib
import time
import logging
from typing import Dict, Optional, Tuple

try:
    from scipy.linalg import inv as scipy_inv
    HAS_SCIPY = True
except ImportError:
    HAS_SCIPY = False

logger = logging.getLogger("substrate.spinor_cayley")

SIGMA_X = np.array([[0.0, 1.0], [1.0, 0.0]], dtype=complex)
SIGMA_Y = np.array([[0.0, -1.0j], [1.0j, 0.0]], dtype=complex)
SIGMA_Z = np.array([[1.0, 0.0], [0.0, -1.0]], dtype=complex)
IDENTITY_2 = np.eye(2, dtype=complex)


class PauliSpinor:
    """Spinor (α, β) normalizado. Coerência = ⟨σ_z⟩."""

    def __init__(self, alpha: complex = 1.0, beta: complex = 0.0):
        self.alpha = complex(alpha)
        self.beta = complex(beta)
        self._normalize()

    def _normalize(self) -> None:
        norm = np.sqrt(abs(self.alpha)**2 + abs(self.beta)**2)
        if norm > 1e-15:
            self.alpha /= norm
            self.beta /= norm

    def coherence(self) -> float:
        """Retorna ⟨σ_z⟩ = |α|² - |β|² ∈ [-1, 1]."""
        return float(abs(self.alpha)**2 - abs(self.beta)**2)

    def phase(self) -> float:
        """Fase relativa φ = arg(α/β)."""
        if abs(self.beta) < 1e-12:
            return 0.0
        return float(np.angle(self.alpha / self.beta))

    def bloch_angles(self) -> Tuple[float, float]:
        """ ângulos de Bloch (θ, φ) tal que |ψ⟩ = cos(θ/2)|0⟩ + e^{iφ}sin(θ/2)|1⟩."""
        theta = 2.0 * np.arccos(np.clip(abs(self.alpha), 0.0, 1.0))
        phi = np.angle(self.beta) - np.angle(self.alpha)
        return float(theta), float(phi)

    def density_matrix(self) -> np.ndarray:
        """Matriz densidade ρ = |ψ⟩⟨ψ|."""
        vec = np.array([self.alpha, self.beta], dtype=complex)
        return np.outer(vec, np.conj(vec))

    def fidelity_with(self, other: 'PauliSpinor') -> float:
        """Fidelidade F = |⟨ψ|φ⟩|²."""
        overlap = np.conj(self.alpha) * other.alpha + np.conj(self.beta) * other.beta
        return float(abs(overlap)**2)

    def cayley_step(self, hamiltonian: np.ndarray, dt: float) -> 'PauliSpinor':
        """
        Propaga o spinor usando o propagador de Cayley:
            U = (I - Y)⁻¹ (I + Y),  Y = i·(dt/2)·H

        U é unitário exato (sem truncamento). Fallback para autovalores se
        a inversão falhar.
        """
        Y = 1j * (dt / 2.0) * hamiltonian
        n = len(hamiltonian)
        I = np.eye(n, dtype=complex)

        try:
            if HAS_SCIPY:
                U = scipy_inv(I - Y) @ (I + Y)
            else:
                U = np.linalg.inv(I - Y) @ (I + Y)
        except np.linalg.LinAlgError:
            logger.warning("Cayley inversão singular — usando decomposição espectral")
            eigvals, eigvecs = np.linalg.eig(hamiltonian)
            D = np.diag(np.exp(-1j * eigvals * dt))
            U = eigvecs @ D @ np.linalg.inv(eigvecs)

        vec = np.array([self.alpha, self.beta], dtype=complex)
        new_vec = U @ vec
        return PauliSpinor(new_vec[0], new_vec[1])

    def measure(self, axis: str = 'z', u: float = None) -> Tuple[int, 'PauliSpinor']:
        """
        Medição conforme postulado de Born.
        axis: 'z' ou 'x'.
        u: variável uniforme em [0,1) para amostragem reprodutível.
        Retorna (resultado, novo_estado).
        """
        if u is None:
            u = np.random.random()

        if axis == 'z':
            prob_up = abs(self.alpha)**2
            if u < prob_up:
                return 1, PauliSpinor(1.0, 0.0)
            else:
                return -1, PauliSpinor(0.0, 1.0)
        elif axis == 'x':
            psi_x = (SIGMA_X @ np.array([self.alpha, self.beta], dtype=complex))
            prob_up = abs(psi_x[0])**2
            if u < prob_up:
                return 1, PauliSpinor(1 / np.sqrt(2), 1 / np.sqrt(2))
            else:
                return -1, PauliSpinor(1 / np.sqrt(2), -1 / np.sqrt(2))
        else:
            raise ValueError(f"Eixo '{axis}' inválido. Use 'z' ou 'x'.")

    def apply_pauli(self, gate: str) -> 'PauliSpinor':
        """Aplica uma porta de Pauli ao spinor."""
        gates = {'x': SIGMA_X, 'y': SIGMA_Y, 'z': SIGMA_Z}
        if gate not in gates:
            raise ValueError(f"Porta '{gate}' inválida. Use 'x', 'y', ou 'z'.")
        vec = gates[gate] @ np.array([self.alpha, self.beta], dtype=complex)
        return PauliSpinor(vec[0], vec[1])

    def to_dict(self) -> Dict:
        return {
            'alpha': str(self.alpha),
            'beta': str(self.beta),
            'coherence': self.coherence(),
            'phase': self.phase(),
        }

    @classmethod
    def from_dict(cls, d: Dict) -> 'PauliSpinor':
        return cls(complex(d['alpha']), complex(d['beta']))

    def __repr__(self) -> str:
        return f"PauliSpinor(Φ={self.coherence():.4f}, φ={self.phase():.4f})"

    def __eq__(self, other: object) -> bool:
        if not isinstance(other, PauliSpinor):
            return NotImplemented
        return abs(self.alpha - other.alpha) < 1e-10 and abs(self.beta - other.beta) < 1e-10


def cayley_propagator(hamiltonian: np.ndarray, dt: float) -> np.ndarray:
    """Retorna a matriz unitária U do propagador de Cayley."""
    Y = 1j * (dt / 2.0) * hamiltonian
    n = len(hamiltonian)
    I = np.eye(n, dtype=complex)
    if HAS_SCIPY:
        return scipy_inv(I - Y) @ (I + Y)
    return np.linalg.inv(I - Y) @ (I + Y)


def random_pauli_hamiltonian(scale: float = 1.0) -> np.ndarray:
    """Gera um hamiltoniano de Pauli aleatório Hermitiano."""
    coeffs = np.random.randn(3) * scale
    H = coeffs[0] * SIGMA_X + coeffs[1] * SIGMA_Y + coeffs[2] * SIGMA_Z
    return H
