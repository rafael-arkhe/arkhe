"""
Property-based tests.
"""

import random
import string
import pytest
from core.crypto import canonical_json_bytes, compute_chunk_hash
from core.mmr import MerkleMountainRange

def random_string(n=20):
    return ''.join(random.choices(string.ascii_letters + string.digits, k=n))

def test_property_mmr_append_only_monotony():
    mmr = MerkleMountainRange()
    previous_snapshots = []
    for step in range(25):
        leaf = compute_chunk_hash(f"prop-leaf-{step}".encode())
        mmr.append(leaf)
        previous_snapshots.append(list(mmr.leaves))

    for snap in previous_snapshots:
        ok, msg = mmr.verify_consistency(snap)
        assert ok, msg

def test_property_canonical_json_invariance():
    for _ in range(50):
        keys = [f"k_{i}" for i in range(10)]
        vals = [random_string() for _ in range(10)]
        d1 = dict(zip(keys, vals))
        shuffled_keys = list(keys)
        random.shuffle(shuffled_keys)
        d2 = {k: d1[k] for k in shuffled_keys}
        assert canonical_json_bytes(d1) == canonical_json_bytes(d2)
