#!/usr/bin/env python3
"""
Testes unitários para o Substrato Observatório.

Executar: pytest tests/test_observatory.py -v
"""

import sys
import os
import math

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from substrate_observatory import Observatory


class TestObservatory:
    def test_record(self):
        obs = Observatory()
        obs.record('test_metric', 42.0)
        assert len(obs.observations) == 1

    def test_statistics(self):
        obs = Observatory()
        for v in [1.0, 2.0, 3.0, 4.0, 5.0]:
            obs.record('metric', v)
        stats = obs.get_statistics('metric')
        assert stats['count'] == 5
        assert abs(stats['mean'] - 3.0) < 1e-10
        assert abs(stats['min'] - 1.0) < 1e-10
        assert abs(stats['max'] - 5.0) < 1e-10

    def test_statistics_empty(self):
        obs = Observatory()
        stats = obs.get_statistics('nonexistent')
        assert stats['count'] == 0

    def test_detect_anomalies_none(self):
        obs = Observatory()
        for v in [1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0]:
            obs.record('stable', v)
        anomalies = obs.detect_anomalies('stable', threshold=2.0)
        assert len(anomalies) == 0

    def test_detect_anomalies_present(self):
        obs = Observatory()
        for _ in range(20):
            obs.record('jitter', 1.0)
        obs.record('jitter', 100.0)
        anomalies = obs.detect_anomalies('jitter', threshold=2.0)
        assert len(anomalies) >= 1

    def test_detect_trend_stable(self):
        obs = Observatory()
        for _ in range(60):
            obs.record('flat', 1.0)
        trend = obs.detect_trend('flat')
        assert trend == 'estável' or trend is None

    def test_detect_trend_insufficient(self):
        obs = Observatory()
        obs.record('short', 1.0)
        trend = obs.detect_trend('short')
        assert trend is None

    def test_generate_insight(self):
        obs = Observatory()
        insight = obs.generate_insight('test insight', confidence=0.8)
        assert insight.content == 'test insight'
        assert insight.confidence == 0.8
        assert len(obs.insights) == 1

    def test_recent_insights(self):
        obs = Observatory()
        for i in range(5):
            obs.generate_insight(f'insight_{i}')
        recent = obs.get_recent_insights(3)
        assert len(recent) == 3
        assert recent[-1]['content'] == 'insight_4'

    def test_dashboard(self):
        obs = Observatory()
        obs.record('m1', 1.0)
        obs.record('m1', 2.0)
        obs.generate_insight('test')
        dashboard = obs.get_dashboard()
        assert dashboard['total_observations'] == 2
        assert dashboard['total_insights'] == 1
        assert 'm1' in dashboard['metrics']
