# catedral_os_v152/tests/test_verifier.py
"""Testes do verificador Lean embarcado (I200)."""

from verification.lean_runtime import InvariantStatus, LeanVerifier


def test_i194_zeno_check():
    v = LeanVerifier()
    assert v.check_i194(dt_us=100, ranging_us=1000) == InvariantStatus.VERIFIED
    assert v.check_i194(dt_us=3000, ranging_us=1000) == InvariantStatus.VIOLATED


def test_i167_entropy_check():
    v = LeanVerifier()
    assert v.check_i167([0.25, 0.25, 0.25, 0.25], 8, 4) == InvariantStatus.VERIFIED


def test_i193_energy_check():
    v = LeanVerifier()
    assert v.check_i193(0.95, 1e-15) == InvariantStatus.VERIFIED
    assert v.check_i193(0.95, 1.0) == InvariantStatus.VIOLATED


def test_verify_all_pass():
    v = LeanVerifier()
    report = v.verify_all(
        [0.25, 0.25, 0.25, 0.25],
        n_total=16,
        n_handovers=8,
        phi_irr=0.7,
        phi_crit=0.3,
        anti_flatness=0.01,
        dt_us=500,
        ranging_us=1000,
        phi_local=0.95,
        phi_remote=0.94,
        energy=1e-15,
    )
    assert report.all_verified
    assert len(report.checks) == 7


def test_verify_all_fails_on_zeno():
    v = LeanVerifier()
    report = v.verify_all(
        [0.25, 0.25, 0.25, 0.25],
        n_total=16,
        n_handovers=8,
        phi_irr=0.7,
        phi_crit=0.3,
        anti_flatness=0.01,
        dt_us=5000,
        ranging_us=1000,
        phi_local=0.95,
        phi_remote=0.94,
        energy=1e-15,
    )
    assert not report.all_verified
    i194 = next(s for label, s, _ in report.checks if label == "I194")
    assert i194 == InvariantStatus.VIOLATED
    assert any(s == InvariantStatus.VIOLATED for _, s, _ in report.checks)