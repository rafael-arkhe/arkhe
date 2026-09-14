#!/usr/bin/env python3
"""
Testes unitários para o Substrato Motor Ferrônico.

Executar: pytest tests/test_ferronic.py -v
"""

import sys
import os

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from substrate_spinor_cayley import PauliSpinor
from substrate_ferronic_engine import FerronicEngine, FerronMode


class TestFerronicEngine:
    def test_initialize_modes(self):
        engine = FerronicEngine()
        assert len(engine.modes) == 3
        assert engine.active_mode == 'Q1'

    def test_switch_mode(self):
        engine = FerronicEngine()
        result = engine.switch_mode('Q2')
        assert result['current'] == 'Q2'
        assert engine.active_mode == 'Q2'

    def test_switch_nonexistent(self):
        engine = FerronicEngine()
        result = engine.switch_mode('Q99')
        assert 'error' in result

    def test_emit_handover(self):
        engine = FerronicEngine()
        s = PauliSpinor(0.6, 0.8)
        handover = engine.emit_handover(s)
        assert 'mode' in handover
        assert handover['mode'] == 'Q1'
        assert 'frequency_thz' in handover
        assert 'quality_factor' in handover

    def test_emission_log(self):
        engine = FerronicEngine()
        s = PauliSpinor(1.0, 0.0)
        engine.emit_handover(s)
        engine.emit_handover(s)
        assert len(engine.emission_log) == 2

    def test_emission_rate(self):
        engine = FerronicEngine()
        s = PauliSpinor(1.0, 0.0)
        for _ in range(10):
            engine.emit_handover(s)
        rate = engine.get_emission_rate()
        assert rate > 0

    def test_mode_status(self):
        engine = FerronicEngine()
        status = engine.get_mode_status()
        assert 'active_mode' in status
        assert 'modes' in status
        assert len(status['modes']) == 3

    def test_ferron_mode_energy(self):
        mode = FerronMode(
            name='test', frequency_thz=1.0, quality_factor=100,
            emission_efficiency=1.0, polarization_axis='uniaxial',
            coherence_length_um=1.0, propagation_speed_ms=1e5,
        )
        energy = mode.energy_eV()
        assert energy > 0

    def test_ferron_mode_decay_time(self):
        mode = FerronMode(
            name='test', frequency_thz=1.0, quality_factor=100,
            emission_efficiency=1.0, polarization_axis='uniaxial',
            coherence_length_um=1.0, propagation_speed_ms=1e5,
        )
        decay = mode.decay_time_ps()
        assert decay > 0
