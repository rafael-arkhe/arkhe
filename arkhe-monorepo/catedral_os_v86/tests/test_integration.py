#!/usr/bin/env python3
"""
Testes de integração para o Catedral OS v86.0.

Executa o ciclo completo do organismo e verifica a coesão entre módulos.

Executar: pytest tests/test_integration.py -v
"""

import sys
import os
import json
import numpy as np

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from catedral_os_v86 import CatedralOSv86


class TestCatedralOSv86Integration:
    def test_initialization(self):
        cat = CatedralOSv86()
        assert cat.cycle_count == 0
        assert cat.base_spinor is not None

    def test_single_cycle(self):
        cat = CatedralOSv86()
        result = cat.cycle()
        assert result['cycle'] == 1
        assert 'coherence' in result
        assert 'phase' in result
        assert 'consciousness' in result
        assert 'retroactive' in result
        assert 'ferron_mode' in result
        assert 'teukolsky_ratio' in result
        assert 'modulation' in result

    def test_multiple_cycles(self):
        cat = CatedralOSv86()
        results = cat.run(n_cycles=5, verbose=False)
        assert len(results) == 5
        assert results[-1]['cycle'] == 5

    def test_coherence_bounded(self):
        cat = CatedralOSv86()
        for _ in range(10):
            result = cat.cycle()
            assert -1.0 <= result['coherence'] <= 1.0

    def test_mode_switching(self):
        cat = CatedralOSv86()
        results = cat.run(n_cycles=12, verbose=False)
        ferron_modes = [r['ferron_mode'] for r in results]
        assert len(set(ferron_modes)) > 1

    def test_observatory_populated(self):
        cat = CatedralOSv86()
        cat.run(n_cycles=15, verbose=False)
        assert len(cat.observatory.observations) > 0

    def test_handovers_accumulated(self):
        cat = CatedralOSv86()
        cat.run(n_cycles=10, verbose=False)
        assert len(cat.handovers) == 10

    def test_beliefs_generated(self):
        cat = CatedralOSv86()
        cat.run(n_cycles=15, verbose=False)
        assert len(cat.beliefs) >= 1

    def test_get_state(self):
        cat = CatedralOSv86()
        cat.run(n_cycles=5, verbose=False)
        state = cat.get_state()
        assert 'cycle' in state
        assert 'spinor' in state
        assert 'coherence_history' in state
        assert 'observatory' in state
        assert 'ferronic' in state
        assert 'teukolsky' in state
        assert 'metasurface' in state

    def test_state_json_serializable(self):
        cat = CatedralOSv86()
        cat.run(n_cycles=3, verbose=False)
        state = cat.get_state()
        serialized = json.dumps(state, default=str)
        assert len(serialized) > 0
