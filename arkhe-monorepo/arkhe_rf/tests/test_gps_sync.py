"""Testes de sincronização GPS (PPS) — coerência temporal RF/ELF."""

import math

import pytest

from arkhe_rf.gps_sync import GPSSync, pps_next_second


@pytest.mark.parametrize("now", [1_000_000_000.3, 1_000_000_000.0, 1_000_000_001.9])
def test_pps_next_second_alignment(now):
    pps = pps_next_second(now)
    assert math.isclose(pps, round(pps))
    assert pps >= now
    assert pps - now < 1.0 + 1e-9


def test_get_pps_monotonic():
    gps = GPSSync()
    p1 = gps.get_pps_timestamp()
    p2 = gps.get_pps_timestamp()
    assert p2 >= p1


def test_within_tolerance_rule():
    gps = GPSSync(tolerance_ms=1.0)
    t = 1_000_000_000.0
    assert gps.within_tolerance(t, t)              # mesmo pulso
    assert not gps.within_tolerance(t, t + 1.0)    # 1 s é muito fora da tol 1 ms


def test_annotate_metadata():
    gps = GPSSync()
    meta = gps.annotate(gps.get_pps_timestamp(), channel="ELF")
    assert meta["channel"] == "ELF"
    assert meta["pps_locked"] is True
    assert set(meta) >= {"unix_epoch_s", "skew_ms", "coherent_tolerance_ms"}


def test_sim_backend_skew_zero():
    gps = GPSSync()
    assert gps.mode == "sim"
    assert gps.skew_ms() == 0.0
    assert gps.lock_offset_s() == 0.0