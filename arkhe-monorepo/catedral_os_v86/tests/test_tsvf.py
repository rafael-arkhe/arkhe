#!/usr/bin/env python3
"""
Testes unitários para o Substrato TSVF-Retrocausal.

Executar: pytest tests/test_tsvf.py -v
"""

import sys
import os
import numpy as np

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from substrate_spinor_cayley import PauliSpinor, SIGMA_Z, SIGMA_X
from substrate_tsvf_retrocausal import TwoStateVector, RetrocausalEngine


class TestTwoStateVector:
    def test_weak_value_identity(self):
        psi = PauliSpinor(1.0, 0.0)
        phi = PauliSpinor(1.0, 0.0)
        tsv = TwoStateVector(forward=psi, backward=phi)
        wv = tsv.weak_value(SIGMA_Z)
        assert abs(wv - 1.0) < 1e-10

    def test_weak_value_orthogonal(self):
        psi = PauliSpinor(1.0, 0.0)
        phi = PauliSpinor(0.0, 1.0)
        tsv = TwoStateVector(forward=psi, backward=phi)
        wv = tsv.weak_value(SIGMA_Z)
        assert abs(wv) < 1e-10

    def test_coherence_max(self):
        psi = PauliSpinor(1.0, 0.0)
        phi = PauliSpinor(1.0, 0.0)
        tsv = TwoStateVector(forward=psi, backward=phi)
        assert abs(tsv.coherence() - 1.0) < 1e-10

    def test_coherence_min(self):
        psi = PauliSpinor(1.0, 0.0)
        phi = PauliSpinor(0.0, 1.0)
        tsv = TwoStateVector(forward=psi, backward=phi)
        assert abs(tsv.coherence()) < 1e-10

    def test_overlap(self):
        psi = PauliSpinor(1 / np.sqrt(2), 1 / np.sqrt(2))
        phi = PauliSpinor(1 / np.sqrt(2), -1 / np.sqrt(2))
        tsv = TwoStateVector(forward=psi, backward=phi)
        ov = tsv.overlap()
        assert abs(ov) < 1e-10  # ortogonais


class TestRetrocausalEngine:
    def test_initialize(self):
        engine = RetrocausalEngine()
        state = engine.initialize(PauliSpinor(1.0, 0.0))
        assert state is not None
        assert len(engine.history) == 1

    def test_register_decision(self):
        engine = RetrocausalEngine()
        engine.initialize(PauliSpinor(1.0, 0.0))
        decision = engine.register_future_decision('d1', {'retro_angle': 0.5})
        assert decision.decision_id == 'd1'
        assert not decision.executed

    def test_apply_retroactive(self):
        engine = RetrocausalEngine()
        engine.initialize(PauliSpinor(1.0, 0.0))
        engine.register_future_decision('d1', {
            'retro_angle': 0.3, 'retro_phase': 0.1
        })
        applied = engine.apply_retroactive_effect('d1')
        assert applied
        assert engine.future_decisions['d1'].executed

    def test_apply_nonexistent(self):
        engine = RetrocausalEngine()
        engine.initialize(PauliSpinor(1.0, 0.0))
        applied = engine.apply_retroactive_effect('nonexistent')
        assert not applied

    def test_apply_idempotent(self):
        engine = RetrocausalEngine()
        engine.initialize(PauliSpinor(1.0, 0.0))
        engine.register_future_decision('d1', {
            'retro_angle': 0.3, 'retro_phase': 0.1
        })
        engine.apply_retroactive_effect('d1')
        applied2 = engine.apply_retroactive_effect('d1')
        assert not applied2

    def test_too_late_choice(self):
        engine = RetrocausalEngine()
        result = engine.too_late_choice_handover(
            PauliSpinor(1.0, 0.0),
            {'retro_angle': 0.5, 'retro_phase': 0.2}
        )
        assert 'handover_id' in result
        assert result['retroactive_effect']
        assert 'weak_values' in result
        assert len(engine.history) >= 1

    def test_anomalous_weak_values(self):
        engine = RetrocausalEngine()
        engine.initialize(PauliSpinor(1 / np.sqrt(2), 1 / np.sqrt(2)))
        engine.too_late_choice_handover(
            PauliSpinor(1 / np.sqrt(2), 1 / np.sqrt(2)),
            {'retro_angle': 0.8, 'retro_phase': 0.0}
        )
        anomalies = engine.get_anomalous_weak_values(
            SIGMA_Z, eigenvalues=[1.0, -1.0]
        )
        assert isinstance(anomalies, list)
