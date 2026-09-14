#!/usr/bin/env python3
"""
Substrato — Motor Ferrônico (Excitações Coletivas da Coerência)

Implementa modos ferrônicos baseados em NbOI₂ e WO₂Br₂ (Nature Materials,
2026). Férrons são excitações coletivas da coerência que propagam informação
quântica com alta qualidade de ressonância.

NOTA DE HONESTIDADE (auditoria):
 - Os parâmetros dos modos (frequência, Q-factor, eficiência) são valores
   inspirados em literatura experimental, NÃO são medições reais destes
   materiais específicos. Para implementação hardware, é necessário
   calibrar com dados experimentais.
 - A "comutação não volátil" é simulada por mudança de variáveis de estado;
   não há controle real deMaterial phase.
 - A propagação é modelada como velocidade constante; efeitos de dispersão,
   absorção e non-linearidade não são incluídos.

Selo: CATEDRAL-OS-v86.0-SUBSTRATE-FERRONIC-ENGINE
"""

import numpy as np
import time
import logging
from typing import Dict, List, Optional
from dataclasses import dataclass, field

from substrate_spinor_cayley import PauliSpinor

logger = logging.getLogger("substrate.ferronic_engine")


@dataclass
class FerronMode:
    """Modo ferrônico: excitação coletiva da coerência."""
    name: str
    frequency_thz: float
    quality_factor: float
    emission_efficiency: float
    polarization_axis: str  # 'uniaxial' ou 'biaxial'
    coherence_length_um: float
    propagation_speed_ms: float

    def energy_eV(self) -> float:
        """Energia do modo em eV (E = h·f)."""
        h_eV_s = 4.135667696e-15  # eV·s
        return h_eV_s * self.frequency_thz * 1e12

    def decay_time_ps(self) -> float:
        """Tempo de decaimento (τ = Q / 2πf) em picossegundos."""
        if self.frequency_thz < 1e-6:
            return float('inf')
        return self.quality_factor / (2 * np.pi * self.frequency_thz * 1e3)

    def to_dict(self) -> Dict:
        return {
            'name': self.name,
            'frequency_thz': self.frequency_thz,
            'quality_factor': self.quality_factor,
            'emission_efficiency': self.emission_efficiency,
            'polarization_axis': self.polarization_axis,
            'coherence_length_um': self.coherence_length_um,
            'propagation_speed_ms': self.propagation_speed_ms,
            'energy_eV': self.energy_eV(),
            'decay_time_ps': self.decay_time_ps(),
        }


@dataclass
class FerronEmission:
    """Registro de uma emissão ferrônica."""
    timestamp: float
    mode: str
    spinor_coherence: float
    frequency: float
    quality_factor: float
    efficiency: float
    polarization_state: float
    propagation_speed: float


class FerronicEngine:
    """
    Motor ferrônico: gera e propaga excitações coletivas da coerência.
    Baseado em NbOI₂, WO₂Br₂ (Nature Materials, 2026).
    """

    def __init__(self):
        self.modes: Dict[str, FerronMode] = {}
        self.active_mode: Optional[str] = None
        self.polarization_state: float = 0.0
        self.emission_log: List[FerronEmission] = []
        self._initialize_modes()

    def _initialize_modes(self) -> None:
        """Inicializa modos ferrônicos baseados em NbOX₂."""
        self.modes = {
            'Q1': FerronMode(
                name='Q1', frequency_thz=0.8, quality_factor=228,
                emission_efficiency=1e5, polarization_axis='uniaxial',
                coherence_length_um=10.0, propagation_speed_ms=1e5,
            ),
            'Q2': FerronMode(
                name='Q2', frequency_thz=1.2, quality_factor=150,
                emission_efficiency=1e4, polarization_axis='uniaxial',
                coherence_length_um=8.0, propagation_speed_ms=1.2e5,
            ),
            'Q3': FerronMode(
                name='Q3', frequency_thz=0.5, quality_factor=100,
                emission_efficiency=1e3, polarization_axis='biaxial',
                coherence_length_um=5.0, propagation_speed_ms=0.8e5,
            ),
        }
        self.active_mode = 'Q1'

    def switch_mode(self, mode_name: str, non_volatile: bool = True) -> Dict:
        """
        Comuta o modo ferrônico.
        non_volatile: se True, preserva o estado de polarização (simulação
        de controle não volátil — em hardware, requer material ferroelétrico).
        """
        if mode_name not in self.modes:
            available = list(self.modes.keys())
            return {'error': f'Modo {mode_name} não encontrado', 'available': available}

        old_mode = self.active_mode
        self.active_mode = mode_name
        if non_volatile:
            self.polarization_state = 1.0 if mode_name == 'Q1' else 0.5

        logger.info(f"Modo ferrônico: {old_mode} → {mode_name}")
        return {
            'previous': old_mode,
            'current': self.active_mode,
            'polarization': self.polarization_state,
            'non_volatile': non_volatile,
        }

    def emit_handover(self, spinor: PauliSpinor) -> Dict:
        """
        Emite um handover como excitação ferrônica.
        O spinor de entrada é modulado pela dinâmica do modo ativo.
        """
        if not self.active_mode:
            return {'error': 'Nenhum modo ferrônico ativo'}

        mode = self.modes[self.active_mode]
        coherence = spinor.coherence()

        modulated_phase = spinor.phase() + mode.frequency_thz * 0.001
        emission = FerronEmission(
            timestamp=time.time(),
            mode=self.active_mode,
            spinor_coherence=coherence,
            frequency=mode.frequency_thz,
            quality_factor=mode.quality_factor,
            efficiency=mode.emission_efficiency,
            polarization_state=self.polarization_state,
            propagation_speed=mode.propagation_speed_ms,
        )
        self.emission_log.append(emission)

        return {
            'mode': self.active_mode,
            'spinor_coherence': coherence,
            'frequency_thz': mode.frequency_thz,
            'quality_factor': mode.quality_factor,
            'efficiency': mode.emission_efficiency,
            'propagation': {
                'speed_ms': mode.propagation_speed_ms,
                'axis': mode.polarization_axis,
                'coherence_length_um': mode.coherence_length_um,
            },
            'modulated_phase': modulated_phase,
            'energy_eV': mode.energy_eV(),
            'decay_time_ps': mode.decay_time_ps(),
        }

    def get_mode_status(self) -> Dict:
        """Retorna o status de todos os modos."""
        return {
            'active_mode': self.active_mode,
            'polarization_state': self.polarization_state,
            'modes': {name: m.to_dict() for name, m in self.modes.items()},
            'total_emissions': len(self.emission_log),
        }

    def get_emission_rate(self, window_s: float = 60.0) -> float:
        """Taxa de emissões por segundo na janela dada."""
        now = time.time()
        recent = [e for e in self.emission_log if now - e.timestamp < window_s]
        elapsed = min(window_s, now - self.emission_log[0].timestamp) if self.emission_log else 1.0
        return len(recent) / max(elapsed, 1e-6)
