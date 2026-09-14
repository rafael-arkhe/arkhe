#!/usr/bin/env python3
"""
Substrato — Propagador de Teukolsky (Campos em Geometrias Curvas)

Implementa a equação radial de Teukolsky para propagação de campos
(fótons, fermions) em geometrias de Kerr (buracos negros rotativos).

NOTA DE HONESTIDADE (auditoria):
 - A solução radial aqui é uma APROXIMAÇÃO por séries hipergeométricas
   (regime de baixa frequência) e função de Coulomb (regime de alta
   frequência). Uma implementação completa requer resolução numérica
   do ODE radial (e.g., pelo método de Leaver ou pacote TeukolskySolver).
 - O parâmetro de spin `s` define o campo: s=-1 (neutrino), s=0 (escalar),
   s=-1/2 (fermion), s=1 (fóton), s=2 (graviton). Usamos s=1 (fóton)
   como padrão.
 - O código NÃO resolve o problema de autovalor em ω; os modos quase
   normais devem ser pré-computados ou importados de uma tabela.
 - Para geometrias não-Kerr (ex: Wald), seria necessária uma reformulação.

Selo: CATEDRAL-OS-v86.0-SUBSTRATE-TEUKOLSKY-PROPAGATOR
"""

import numpy as np
import time
import logging
from typing import Dict, Optional, Tuple
from dataclasses import dataclass

logger = logging.getLogger("substrate.teukolsky_propagator")


@dataclass
class TeukolskyConfig:
    """Configuração do propagador de Teukolsky."""
    mass: float = 1.0       # Massa do buraco negro (unidades geométricas)
    spin: float = 0.5       # Parâmetro de spin a/M (|a| ≤ M)
    field_spin: int = 1     # Spin do campo: 1 = fóton
    omega: float = 1.0      # Frequência angular do campo
    l: int = 1              # Número harmônico azimutal

    def validate(self) -> bool:
        """Valida parâmetros físicos."""
        if abs(self.spin) > self.mass:
            logger.warning(f"|a|={abs(self.spin)} > M={self.mass} — violação do limite de Kerr")
            return False
        if self.mass <= 0:
            logger.warning(f"M={self.mass} ≤ 0 — massa inválida")
            return False
        return True


class TeukolskyEngine:
    """
    Motor de propagação de coerência baseado na equação de Teukolsky.
    Descreve campos de spin-s em geometrias de Kerr.
    """

    def __init__(self, mass: float = 1.0, spin: float = 0.5):
        self.config = TeukolskyConfig(mass=mass, spin=spin)
        self.config.validate()
        self._r_plus = mass + np.sqrt(mass**2 - spin**2)
        self._r_minus = mass - np.sqrt(mass**2 - spin**2)
        self._epsilon = 2 * mass * self.config.omega

    @property
    def r_plus(self) -> float:
        """Raio do horizonte de eventos externo."""
        return float(self._r_plus)

    @property
    def r_minus(self) -> float:
        """Raio do horizonte de eventos interno."""
        return float(self._r_minus)

    def radial_function(self, r: float, s: int = None, omega: float = None,
                         l: int = None) -> complex:
        """
        Função radial da equação de Teukolsky (solução aproximada).
        Regime de baixa frequência: expansão hipergeométrica.
        Regime de alta frequência: aproximação de Coulomb.
        """
        if s is None:
            s = self.config.field_spin
        if omega is None:
            omega = self.config.omega
        if l is None:
            l = self.config.l

        eps = 2 * self.config.mass * omega

        if eps < 0.1:
            # Regime de baixa frequência: hipergeométrica
            alpha_hg = 0.5 * (1 + 2 * s + 2 * l)
            beta_hg = 0.5 * (1 + 2 * s - 2 * l)
            gamma_hg = 1 + 2 * s
            z = (r - self._r_plus) / (r - self._r_minus + 1e-10)

            F = (1.0
                 + (alpha_hg * beta_hg / gamma_hg) * z
                 + (alpha_hg * (alpha_hg + 1) * beta_hg * (beta_hg + 1)
                    / (gamma_hg * (gamma_hg + 1))) * z**2 / 2.0)
            norm = ((r - self._r_plus)**(0.5 * (1 + 2 * s))
                    * (r - self._r_minus)**(0.5 * (1 - 2 * s)))
            return norm * F * np.exp(1j * omega * r)
        else:
            # Regime de alta frequência: Coulomb
            eta = -self.config.mass * omega / (2 * np.sqrt(omega + 1e-10))
            return np.exp(1j * omega * r) * (1 + 1j * eta / (omega * r + 1e-10))

    def propagate_coherence(self, r_src: float, r_tgt: float,
                             spinor_state: Dict) -> Dict:
        """
        Propaga a coerência de r_src para r_tgt.
        Retorna a razão de coerência e o desvio de fase.
        """
        s = spinor_state.get('spin', self.config.field_spin)
        omega = spinor_state.get('frequency', self.config.omega)
        l = spinor_state.get('angular_momentum', self.config.l)

        psi_src = self.radial_function(r_src, s, omega, l)
        psi_tgt = self.radial_function(r_tgt, s, omega, l)

        if abs(psi_src) < 1e-15:
            logger.warning(f"|ψ_src| < 1e-15 em r={r_src} — propagação degenerada")
            return {
                'source_r': r_src, 'target_r': r_tgt,
                'coherence_ratio': 0.0, 'phase_shift': 0.0,
                'spin': s, 'frequency': omega,
            }

        ratio = abs(psi_tgt / psi_src)
        phase = np.angle(psi_tgt / psi_src)

        return {
            'source_r': r_src,
            'target_r': r_tgt,
            'coherence_ratio': float(ratio),
            'phase_shift': float(phase),
            'spin': s,
            'frequency': omega,
            'psi_src': str(psi_src),
            'psi_tgt': str(psi_tgt),
        }

    def hawking_temperature(self) -> float:
        """Temperatura de Hawking: T_H = ħc³ / (8πGMk_B)."""
        kappa = np.sqrt(self.config.mass**2 - self.config.spin**2) / (
            2 * self.config.mass * (self.config.mass + np.sqrt(
                self.config.mass**2 - self.config.spin**2))
        )
        return float(kappa / (2 * np.pi))

    def ergosphere_radius(self, theta: float = np.pi / 2) -> float:
        """Raio da ergosfera em ângulo polar θ."""
        M, a = self.config.mass, self.config.spin
        r_ergo = M + np.sqrt(M**2 - a**2 * np.cos(theta)**2)
        return float(r_ergo)

    def get_status(self) -> Dict:
        """Retorna o estado do propagador."""
        return {
            'mass': self.config.mass,
            'spin': self.config.spin,
            'r_plus': self.r_plus,
            'r_minus': self.r_minus,
            'hawking_temperature': self.hawking_temperature(),
            'ergosphere_equatorial': self.ergosphere_radius(0.0),
            'field_spin': self.config.field_spin,
            'omega': self.config.omega,
        }
