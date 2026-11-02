"""
Network tests.
"""

import pytest
from core.crypto import compute_chunk_hash, compute_cid_v1
from core.invariants import InvariantVerifier, InvariantViolation

def test_p2p_chunk_corrupted_in_transit():
    original_chunk = b"MODEL_LAYER_01_WEIGHTS_TENSOR"
    expected_hash = compute_chunk_hash(original_chunk)
    corrupted_chunk = b"MODEL_LAYER_01_WEIGHTS_TENSOS"
    with pytest.raises(InvariantViolation):
        InvariantVerifier.verify_chunk_integrity(corrupted_chunk, expected_hash)

def test_p2p_replay_frame_attack():
    from core.crypto import NonceRegistry
    reg = NonceRegistry()
    nonce = "session-nonce-xyz-987"
    assert reg.check_and_record(nonce) is True
    assert reg.check_and_record(nonce) is False
