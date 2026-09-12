"""CI-facing tests for the AVALON RF bridge (avalon_rf_bridge).

Run after:  python -m maturin develop --release
in `packages/arkhe-avalon-bridge`.

Covers:
  * AGC: integer DAC mapping, slew field honesty, current-gain tracking.
  * Safe-Core evidence: deterministic SHA3 digest, verify round-trip.
  * Gates: unverified-DFT rejection, non-finite floats, bad config.
"""

import math

import avalon_rf_bridge as b


def test_agc_maps_integer_gain():
    ctrl = b.AgcController()
    ev = ctrl.set_gain(600)
    assert ev["applied_mv"] == 600
    assert ev["dac_value"] == 744  # (600*4095)/3300 floor
    assert ev["slew_limited"] is False
    assert ev["read_back_ok"] is None  # no fake read-back claim


def test_agc_tracks_current_gain():
    ctrl = b.AgcController()
    assert ctrl.current_gain() is None
    ctrl.set_gain(600)
    assert ctrl.current_gain() == 600


def test_agc_rejects_bad_config():
    try:
        b.AgcController(dac_bits=0)
        raise AssertionError("dac_bits=0 must be rejected")
    except ValueError:
        pass
    try:
        b.AgcController(dac_vref_mv=0)
        raise AssertionError("dac_vref_mv=0 must be rejected")
    except ValueError:
        pass


def test_coherence_digest_is_deterministic():
    a = b.make_coherence_evidence(0.429, 0.031, True, 0.9, 16, 0, True)
    c = b.make_coherence_evidence(0.429, 0.031, True, 0.9, 16, 0, True)
    assert a == c
    assert len(a) == 32  # SHA3-256


def test_coherence_verify_roundtrip():
    dig = b.make_coherence_evidence(0.429, 0.031, True, 0.9, 16, 0, True)
    assert b.verify_coherence_digest(0.429, 0.031, True, 0.9, 16, 0, True, dig) is True
    # Tamper one field -> verify must fail.
    assert b.verify_coherence_digest(0.429, 0.032, True, 0.9, 16, 0, True, dig) is False


def test_gate_rejects_unverified_dft():
    try:
        b.make_coherence_evidence(0.429, 0.031, True, 0.9, 16, 0, False)
        raise AssertionError("DFT-unverified evidence must be rejected")
    except ValueError:
        pass


def test_gate_rejects_nan():
    try:
        b.make_coherence_evidence(math.nan, 0.031, True, 0.9, 16, 0, True)
        raise AssertionError("NaN must be rejected")
    except ValueError:
        pass


def test_gate_rejects_coherence_out_of_range():
    try:
        b.make_coherence_evidence(0.429, 0.031, True, 1.5, 16, 0, True)
        raise AssertionError("coherence > 1 must be rejected")
    except ValueError:
        pass


if __name__ == "__main__":
    import sys

    fns = [v for k, v in sorted(globals().items()) if k.startswith("test_")]
    failed = 0
    for fn in fns:
        try:
            fn()
            print(f"PASS {fn.__name__}")
        except AssertionError as e:
            failed += 1
            print(f"FAIL {fn.__name__}: {e}")
    sys.exit(1 if failed else 0)