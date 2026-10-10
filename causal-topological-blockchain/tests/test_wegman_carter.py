import pytest
from wegman_carter import WegmanCarterMAC, derive_keys, GF2_128, universal_hash_gf128

def test_derive_keys():
    qkd_key = b"secret_qkd_key_1234567890"
    k_hash, k_otp = derive_keys(qkd_key, key_length=32)
    assert len(k_hash) == 32
    assert len(k_otp) == 32
    assert k_hash != k_otp

def test_gf2_128_mul():
    # Simple test for GF2_128 multiplication
    a = 2
    b = 3
    result = GF2_128.mul(a, b)
    assert result == 6 # Since 2 * 3 = 6 (in polynomial basis: x * (x + 1) = x^2 + x)

def test_wegman_carter_mac_sign_verify():
    mac = WegmanCarterMAC()
    signer_id = b"Alice"
    peer_id = b"Bob"
    qkd_key = b"shared_secret_key"

    # Import key material
    mac.import_qkd_key(signer_id, peer_id, qkd_key)

    message = b"Hello, this is a secure message."

    # Sign message
    result = mac.sign(signer_id, peer_id, message)
    assert result is not None
    mac_val, key_idx = result

    # Verify message
    is_valid = mac.verify(signer_id, peer_id, message, mac_val, key_idx)
    assert is_valid == True

def test_wegman_carter_mac_invalid_verification():
    mac = WegmanCarterMAC()
    signer_id = b"Alice"
    peer_id = b"Bob"
    qkd_key = b"shared_secret_key"

    mac.import_qkd_key(signer_id, peer_id, qkd_key)
    message = b"Hello, this is a secure message."

    result = mac.sign(signer_id, peer_id, message)
    assert result is not None
    mac_val, key_idx = result

    # Verify with different message
    wrong_message = b"Hello, this is a tampered message."
    is_valid = mac.verify(signer_id, peer_id, wrong_message, mac_val, key_idx)
    assert is_valid == False
