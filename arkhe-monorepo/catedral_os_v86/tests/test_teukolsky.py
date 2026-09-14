#!/usr/bin/env python3
"""
Testes unitários para o Substrato Teukolsky.

Executar: pytest tests/test_teukolsky.py -v
"""

import sys
import os
import numpy as np

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from substrate_teukolsky_propagator import TeukolskyEngine, TeukolskyConfig


class TestTeukolskyEngine:
    def test_init(self):
        engine = TeukolskyEngine(mass=1.0, spin=0.5)
        assert engine.r_plus > engine.r_minus

    def test_r_plus_minus(self):
        engine = TeukolskyEngine(mass=1.0, spin=0.0)
        # r_plus = M + sqrt(M²-a²) = 1+1=2, r_minus = M - sqrt(M²-a²) = 1-1=0
        assert abs(engine.r_plus - 2.0) < 1e-10
        assert abs(engine.r_minus - 0.0) < 1e-10

    def test_radial_function_finite(self):
        engine = TeukolskyEngine(mass=1.0, spin=0.5)
        psi = engine.radial_function(r=5.0, s=1, omega=1.0, l=1)
        assert np.isfinite(psi)

    def test_propagate_coherence(self):
        engine = TeukolskyEngine(mass=1.0, spin=0.5)
        result = engine.propagate_coherence(
            r_src=3.0, r_tgt=5.0,
            spinor_state={'frequency': 1.0, 'angular_momentum': 1},
        )
        assert 'coherence_ratio' in result
        assert 'phase_shift' in result
        assert result['coherence_ratio'] >= 0

    def test_hawking_temperature(self):
        engine = TeukolskyEngine(mass=1.0, spin=0.0)
        T = engine.hawking_temperature()
        assert T > 0

    def test_ergosphere_radius(self):
        engine = TeukolskyEngine(mass=1.0, spin=0.5)
        # At theta=pi/2 (equator), r_ergo = M + M = 2M > r_plus
        r_eq = engine.ergosphere_radius(np.pi / 2)
        assert r_eq > engine.r_plus

    def test_status(self):
        engine = TeukolskyEngine(mass=2.0, spin=0.8)
        status = engine.get_status()
        assert status['mass'] == 2.0
        assert status['spin'] == 0.8
        assert 'r_plus' in status
        assert 'hawking_temperature' in status

    def test_config_validation(self):
        config = TeukolskyConfig(mass=1.0, spin=0.5)
        assert config.validate()

        bad_config = TeukolskyConfig(mass=1.0, spin=2.0)
        assert not bad_config.validate()
