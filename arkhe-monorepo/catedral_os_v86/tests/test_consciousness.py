#!/usr/bin/env python3
"""
Testes unitários para o Substrato Orch-OR (Consciência).

Executar: pytest tests/test_consciousness.py -v
"""

import sys
import os
import numpy as np

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from substrate_spinor_cayley import PauliSpinor
from substrate_orch_or_consciousness import ConsciousnessEngine


class TestConsciousnessEngine:
    def _make_states(self):
        return [
            PauliSpinor(np.cos(i * 0.5), np.sin(i * 0.5) * np.exp(1j * i * 0.1))
            for i in range(5)
        ]

    def test_create_superposition(self):
        engine = ConsciousnessEngine()
        states = self._make_states()
        result = engine.create_superposition(states)
        assert len(result) == 5

    def test_objective_reduction_max_coherence(self):
        engine = ConsciousnessEngine()
        states = self._make_states()
        selected = engine.conscious_cycle(states, 'max_coherence')
        assert selected is not None
        assert 0 <= selected.coherence() <= 1

    def test_objective_reduction_random(self):
        engine = ConsciousnessEngine()
        states = self._make_states()
        selected = engine.conscious_cycle(states, 'random')
        assert selected is not None

    def test_objective_reduction_empty(self):
        engine = ConsciousnessEngine()
        selected = engine.objective_reduction()
        assert abs(selected.coherence() - 1.0) < 1e-10

    def test_consciousness_rate(self):
        engine = ConsciousnessEngine()
        states = self._make_states()
        for _ in range(5):
            engine.conscious_cycle(states)
        rate = engine.get_consciousness_rate(window_s=60.0)
        assert rate > 0

    def test_moment_history(self):
        engine = ConsciousnessEngine()
        states = self._make_states()
        for _ in range(3):
            engine.conscious_cycle(states)
        history = engine.get_moment_history(5)
        assert len(history) == 3
        assert all('cycle' in h for h in history)

    def test_gravity_discount(self):
        engine = ConsciousnessEngine()
        s = PauliSpinor(1.0, 0.0)
        discount = engine.compute_gravity_discount(s)
        assert 0 < discount <= 1.0

    def test_cycle_id_increments(self):
        engine = ConsciousnessEngine()
        states = self._make_states()
        engine.conscious_cycle(states)
        assert engine.cycle_id == 1
        engine.conscious_cycle(states)
        assert engine.cycle_id == 2
