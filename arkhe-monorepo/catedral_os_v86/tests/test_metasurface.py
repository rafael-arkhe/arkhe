#!/usr/bin/env python3
"""
Testes unitários para o Substrato Metasuperfície.

Executar: pytest tests/test_metasurface.py -v
"""

import sys
import os
import numpy as np

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from substrate_spinor_cayley import PauliSpinor
from substrate_metasurface_modulator import SiliconMetasurface, MetasurfaceConfig


class TestSiliconMetasurface:
    def test_init_defaults(self):
        ms = SiliconMetasurface()
        assert ms.modulation_depth == 0.28
        assert ms.speed_ps == 25.0

    def test_modulate(self):
        ms = SiliconMetasurface()
        s = PauliSpinor(1.0, 0.0)
        modulated = ms.modulate(s)
        assert modulated is not None
        norm = abs(modulated.alpha)**2 + abs(modulated.beta)**2
        assert abs(norm - 1.0) < 1e-8

    def test_modulate_with_override(self):
        ms = SiliconMetasurface()
        s = PauliSpinor(1.0, 0.0)
        modulated = ms.modulate(s, modulation=0.5)
        assert modulated is not None

    def test_modulate_with_phase(self):
        ms = SiliconMetasurface()
        s = PauliSpinor(1.0, 0.0)
        modulated = ms.modulate(s, phase_shift=0.5)
        norm = abs(modulated.alpha)**2 + abs(modulated.beta)**2
        assert abs(norm - 1.0) < 1e-8

    def test_modulation_count(self):
        ms = SiliconMetasurface()
        s = PauliSpinor(1.0, 0.0)
        ms.modulate(s)
        ms.modulate(s)
        assert ms.modulation_count == 2

    def test_throughput(self):
        ms = SiliconMetasurface()
        tp = ms.estimate_throughput()
        assert tp['max_rate_hz'] > 0
        assert tp['modulation_depth'] == 0.28

    def test_status(self):
        ms = SiliconMetasurface()
        status = ms.get_status()
        assert 'modulation_depth' in status
        assert 'speed_ps' in status
        assert 'bandwidth_nm' in status

    def test_custom_config(self):
        config = MetasurfaceConfig(modulation_depth=0.5, speed_ps=10.0)
        ms = SiliconMetasurface(config)
        assert ms.modulation_depth == 0.5
        assert ms.speed_ps == 10.0
