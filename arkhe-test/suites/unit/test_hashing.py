import pytest
from core.crypto import compute_chunk_hash, compute_cid_v1

def test_hash_determinism():
    data = b"arkhe deterministic payload 2026"
    h1 = compute_chunk_hash(data)
    h2 = compute_chunk_hash(data)
    assert h1 == h2
    assert len(h1) == 64

def test_cid_v1_format():
    data = b"chunk payload"
    cid = compute_cid_v1(data, codec="dag-cbor")
    assert cid.startswith("bafy-arkhe-dag-cbor-")
    assert len(cid) > 20

def test_hash_avalanche_effect():
    d1 = b"arkhe_chunk_001"
    d2 = b"arkhe_chunk_002"
    h1 = compute_chunk_hash(d1)
    h2 = compute_chunk_hash(d2)
    assert h1 != h2
