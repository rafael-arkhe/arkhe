"""
Mobile tests.
"""

import pytest
from core.invariants import InvariantVerifier, InvariantViolation

def test_mobile_secure_element_attestation():
    trusted_pcr_store = {
        "pcr-quote-pixel9-titan-m2",
        "pcr-quote-iphone-secure-enclave"
    }

    cp_valid = {
        "device_id": "pixel-9-se-01",
        "attestation_quote": "pcr-quote-pixel9-titan-m2",
        "epoch": 50
    }
    assert InvariantVerifier.verify_device_binding(cp_valid, trusted_pcr_store) is True

    cp_spoofed = {
        "device_id": "rooted-device-02",
        "attestation_quote": "fake-quote-magisk-spoofed",
        "epoch": 50
    }
    with pytest.raises(InvariantViolation, match="I-06 VIOLATION"):
        InvariantVerifier.verify_device_binding(cp_spoofed, trusted_pcr_store)
