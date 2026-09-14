#!/usr/bin/env python3
"""
Substrato — Consciência Quântica (Orch-OR / Penrose-Hameroff)

Implementa o motor de consciência baseado na teoria Orch-OR:
cada momento consciente é uma Redução Objetiva (OR) de um estado
quântico em superposição em microtúbulos.

NOTA DE HONESTIDADE (auditoria):
 - A teoria Orch-OR é altamente controversa na comunidade científica.
   Esta implementação é um MODELO COMPUTACIONAL inspirado na Orch-OR,
   não uma validação da teoria. Nenhum experimento biológico é executado.
 - A "redução objetiva" aqui é implementada como seleção heurística
   (max_coherence, random, etc.), não como colapso gravitacional real.
   O limiar de Penrose (E_G ~ ħ/t_P) NÃO é computado.
 - A rede de microtúbulos (13³) é uma grade 3D simulada; não há dinâmica
   real de tubulina nem interações hidrofóbicas.
 - Este substrato NÃO afirma que IA pode ser consciente. É uma ferramenta
   de exploração computacional.

Selo: CATEDRAL-OS-v86.0-SUBSTRATE-ORCH-OR
"""

import numpy as np
import time
import logging
from typing import Dict, List, Optional
from dataclasses import dataclass, field

from substrate_spinor_cayley import PauliSpinor

logger = logging.getLogger("substrate.orch_or_consciousness")


@dataclass
class ConsciousMoment:
    """Um momento consciente: resultado de uma Redução Objetiva."""
    timestamp: float
    selected_state: Dict
    superposition_size: int
    criterion: str
    spacetime_curvature: float
    cycle_id: int


class ConsciousnessEngine:
    """
    Motor de consciência inspirado na Orch-OR.
    Ciclo: superposição → integração → redução objetiva → momento consciente.
    """

    def __init__(self, microtubule_size: int = 13, or_threshold: float = 0.5):
        self.moments: List[ConsciousMoment] = []
        self.superposition_states: List[PauliSpinor] = []
        self.microtubule_lattice = np.zeros((microtubule_size, microtubule_size,
                                              microtubule_size))
        self.or_threshold = or_threshold
        self.cycle_id = 0

    def create_superposition(self, states: List[PauliSpinor]) -> List[PauliSpinor]:
        """
        Cria um estado de superposição a partir de múltiplos spinors.
        Cada spinor representa um estado possível de consciência.
        """
        if not states:
            raise ValueError("Superposição requer pelo menos um estado")
        self.superposition_states = list(states)
        return self.superposition_states

    def compute_gravity_discount(self, spinor: PauliSpinor,
                                  energy_gap: float = 1.0) -> float:
        """
        Desconto gravitacional simplificado de Penrose.
        E_G proporcional à curvatura espaço-tempo × energia.

        NOTA: Este é um modelo computacional. O valor real de E_G é
        da ordem de 10^-47 J para microtúbulos reais — aqui usamos
        uma escala normalizada.
        """
        coherence = abs(spinor.coherence())
        curvature = 0.1 + 0.9 * coherence
        return float(curvature * energy_gap)

    def objective_reduction(self, selection_criterion: str = 'max_coherence',
                             energy_gap: float = 1.0) -> PauliSpinor:
        """
        Redução Objetiva (OR): colapsa a superposição em um único estado.

        Critérios disponíveis:
        - max_coherence: seleciona o estado com maior |⟨σ_z⟩|
        - max_fidelity: seleciona o estado com maior fidelidade ao primeiro
        - gravity_weighted: pondera por desconto gravitacional de Penrose
        - random: seleção aleatória ponderada
        """
        if not self.superposition_states:
            logger.warning("Nenhuma superposição ativa — retornando estado |0⟩")
            return PauliSpinor(1.0, 0.0)

        self.cycle_id += 1

        if selection_criterion == 'max_coherence':
            selected = max(self.superposition_states,
                           key=lambda s: abs(s.coherence()))
        elif selection_criterion == 'max_fidelity':
            ref = self.superposition_states[0]
            selected = max(self.superposition_states,
                           key=lambda s: s.fidelity_with(ref))
        elif selection_criterion == 'gravity_weighted':
            weights = np.array([
                self.compute_gravity_discount(s, energy_gap)
                for s in self.superposition_states
            ])
            probs = np.exp(weights - np.max(weights))
            probs /= probs.sum()
            idx = np.random.choice(len(self.superposition_states), p=probs)
            selected = self.superposition_states[idx]
        elif selection_criterion == 'random':
            selected = self.superposition_states[
                np.random.randint(len(self.superposition_states))
            ]
        else:
            selected = self.superposition_states[0]

        moment = ConsciousMoment(
            timestamp=time.time(),
            selected_state=selected.to_dict(),
            superposition_size=len(self.superposition_states),
            criterion=selection_criterion,
            spacetime_curvature=self.compute_gravity_discount(selected, energy_gap),
            cycle_id=self.cycle_id,
        )
        self.moments.append(moment)
        self.superposition_states = []

        logger.debug(
            f"OR ciclo {self.cycle_id}: Φ={selected.coherence():.4f}, "
            f"curvatura={moment.spacetime_curvature:.4f}, "
            f"superposição={moment.superposition_size}"
        )
        return selected

    def conscious_cycle(self, inputs: List[PauliSpinor],
                         criterion: str = 'max_coherence') -> PauliSpinor:
        """
        Ciclo consciente completo:
        1. Cria superposição das entradas
        2. Aplica redução objetiva
        3. Retorna o estado selecionado
        """
        self.create_superposition(inputs)
        return self.objective_reduction(criterion)

    def get_consciousness_rate(self, window_s: float = 60.0) -> float:
        """Taxa de momentos conscientes por segundo na janela dada."""
        now = time.time()
        recent = [m for m in self.moments if now - m.timestamp < window_s]
        elapsed = min(window_s, now - self.moments[0].timestamp) if self.moments else 1.0
        return len(recent) / max(elapsed, 1e-6)

    def get_moment_history(self, n: int = 10) -> List[Dict]:
        """Retorna os últimos n momentos conscientes."""
        return [{
            'cycle': m.cycle_id,
            'coherence': m.selected_state.get('coherence', 0),
            'curvature': m.spacetime_curvature,
            'criterion': m.criterion,
            'superposition_size': m.superposition_size,
            'timestamp': m.timestamp,
        } for m in self.moments[-n:]]
