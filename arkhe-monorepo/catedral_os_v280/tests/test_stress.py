# catedral_os_v280/tests/test_stress.py
"""Testes do teste de estresse com injecao de falhas (L6)."""

import random

from stress.stress_tester import StressTester


def _flaky_target(rate):
    rng = random.Random(rate)
    def _target():
        if rng.random() < rate:
            raise RuntimeError("falha simulada")
        return True
    return _target


def test_stress_report_shape():
    tester = StressTester(_flaky_target(0.0), num_workers=4, seed=1,
                          fault_probability=0.0)
    report = tester.start(duration=0, calls_per_worker=20)
    assert report["total_workers"] == 4
    assert report["total_successes"] > 0
    assert report["total_failures"] == 0
    assert report["overall_success_rate"] == 1.0
    assert len(report["workers_report"]) == 4


def test_stress_with_injection_fails_some():
    tester = StressTester(_flaky_target(0.5), num_workers=4, seed=2,
                          fault_probability=0.0)
    report = tester.start(duration=0, calls_per_worker=20)
    assert report["total_successes"] + report["total_failures"] == 80
    assert report["total_failures"] > 0
    assert 0.0 < report["overall_success_rate"] < 1.0
    # por worker
    assert all(0.0 <= r["success_rate"] <= 1.0 for r in report["workers_report"])


def test_fault_injection_toggle():
    tester = StressTester(_flaky_target(0.0), num_workers=2, seed=3,
                          fault_probability=0.3)
    tester.inject_faults(False)
    assert tester.fault_probability == 0.0
    tester.inject_faults(True)
    assert tester.fault_probability == 0.3