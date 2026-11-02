import json
import pytest
from core.mmr import MerkleMountainRange

def test_mmr_append_and_root():
    mmr = MerkleMountainRange()
    assert mmr.get_peaks() == []
    assert len(mmr.leaves) == 0

    l1 = "aa" * 32
    mmr.append(l1)
    assert len(mmr.leaves) == 1
    assert mmr.get_root() == l1

    l2 = "bb" * 32
    mmr.append(l2)
    assert len(mmr.leaves) == 2
    assert len(mmr.get_peaks()) == 1

def test_mmr_golden_vector_peaks():
    with open("vectors/mmr/golden_mmr.json") as f:
        vec = json.load(f)
    mmr = MerkleMountainRange(vec["leaves"])
    assert mmr.get_root() == vec["root"]
    assert mmr.get_peaks() == vec["peaks"]

def test_mmr_consistency_check():
    mmr = MerkleMountainRange()
    mmr.append("11" * 32)
    mmr.append("22" * 32)
    snapshot = list(mmr.leaves)

    mmr.append("33" * 32)
    ok, _ = mmr.verify_consistency(snapshot)
    assert ok

    tampered = ["ff" * 32, "22" * 32]
    bad, msg = mmr.verify_consistency(tampered)
    assert not bad
    assert "Tamper detected" in msg
