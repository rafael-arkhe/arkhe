# catedral_os_v280/tests/test_validator.py
"""Testes do validador em tempo real (L3) + verificacao Lean."""

import os

import pytest

from validation.lean_validator import (InvariantStatus, MiniLeanValidator,
                                       VerificationReport)


def make_state(**kw):
    return {
        "phi_history": kw.get("phi_history", [0.5, 0.6, 0.7]),
        "phi_star": kw.get("phi_star", 0.85),
        "warp": kw.get("warp", 0.85),
        "delta_reward": kw.get("delta_reward", 0.1),
        "ipfs_cid": kw.get("ipfs_cid", "a" * 64),
        "chain_ok": kw.get("chain_ok", True),
        "replay_size": kw.get("replay_size", 10),
        "replay_mean_priority": kw.get("replay_mean_priority", 1.0),
        "cpu_usage": kw.get("cpu_usage", 3.0),
        "memory_usage": kw.get("memory_usage", 24.0),
    }


def test_i238_monotonic():
    v = MiniLeanValidator()
    assert v.check_i238([0.5, 0.6, 0.7]) == InvariantStatus.VERIFIED
    assert v.check_i238([0.7, 0.6, 0.8]) == InvariantStatus.VIOLATED
    assert v.check_i238([0.7]) == InvariantStatus.PENDING


def test_i239_convergence():
    v = MiniLeanValidator()
    assert v.check_i239([0.5], 0.85) == InvariantStatus.PENDING
    hist = [0.85] * 30
    assert v.check_i239(hist, 0.85) == InvariantStatus.VERIFIED
    noisy = [0.85] * 20 + [0.3] * 10
    assert v.check_i239(noisy, 0.85) == InvariantStatus.PENDING


def test_i240_warp_sync():
    v = MiniLeanValidator()
    assert v.check_i240(0.84, 0.85) == InvariantStatus.VERIFIED
    assert v.check_i240(0.20, 0.85) == InvariantStatus.VIOLATED


def test_i241_reward_bound():
    v = MiniLeanValidator()
    assert v.check_i241(0.4) == InvariantStatus.VERIFIED
    assert v.check_i241(1.0) == InvariantStatus.VIOLATED


def test_i255_cid_format():
    v = MiniLeanValidator()
    assert v.check_i255("a" * 64, True) == InvariantStatus.VERIFIED
    assert v.check_i255("Qm" + "b" * 44, True) == InvariantStatus.VERIFIED
    assert v.check_i255("GENESIS", True) == InvariantStatus.PENDING
    assert v.check_i255("abc", False) == InvariantStatus.VIOLATED


def test_i256_per():
    v = MiniLeanValidator()
    assert v.check_i256(0, 0.0) == InvariantStatus.PENDING
    assert v.check_i256(10, 1.0) == InvariantStatus.VERIFIED
    assert v.check_i256(10, 0.0) == InvariantStatus.VIOLATED


def test_i257_dashboard_light():
    v = MiniLeanValidator()
    assert v.check_i257(3.0, 24.0) == InvariantStatus.VERIFIED
    assert v.check_i257(9.0, 24.0) == InvariantStatus.VIOLATED
    assert v.check_i257(3.0, 500.0) == InvariantStatus.VIOLATED


def test_verify_report():
    v = MiniLeanValidator()
    report = v.verify(make_state())
    assert isinstance(report, VerificationReport)
    assert len(report.checks) == 7
    assert not report.any_violated
    labels = [label for label, _, _ in report.checks]
    assert labels == ["I238", "I239", "I240", "I241", "I255", "I256", "I257"]
    as_dict = report.to_dict()
    assert as_dict["I238"]["status"] == "verified"


def test_verify_with_lean(tmp_path):
    root = os.path.dirname(os.path.abspath(__file__))
    theorem = os.path.join(root, "..", "invariants", "ConsolidationInvariants.lean")
    assert os.path.exists(theorem)
    v = MiniLeanValidator(theorem_file=theorem)
    status, detail = v.verify_with_lean()
    # lean está instalado no ambiente; em CI sem lean, status é unavailable.
    assert status in (InvariantStatus.VERIFIED, InvariantStatus.UNAVAILABLE)
    if status == InvariantStatus.VERIFIED:
        assert "compilado" in detail