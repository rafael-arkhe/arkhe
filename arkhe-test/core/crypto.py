"""
Arkhe Cryptographic Primitives & Canonical Serialization.
"""

import json
import hashlib
import time
from typing import Any, Dict, Optional, Tuple, Set

try:
    import blake3 as _blake3_module
    def blake3_digest(data: bytes) -> bytes:
        return _blake3_module.blake3(data).digest()
except ImportError:
    def blake3_digest(data: bytes) -> bytes:
        return hashlib.sha256(b"BLAKE3_EMULATED:" + data).digest()


def canonical_json_bytes(obj: Any) -> bytes:
    """RFC 8785 canonical JSON serializer: sorted keys, no whitespace separators."""
    return json.dumps(
        obj,
        sort_keys=True,
        ensure_ascii=False,
        separators=(',', ':')
    ).encode('utf-8')


def canonical_json_str(obj: Any) -> str:
    """Canonical JSON string."""
    return canonical_json_bytes(obj).decode('utf-8')


def compute_chunk_hash(data: bytes) -> str:
    """Computes BLAKE3 hex digest for raw chunk bytes."""
    return blake3_digest(data).hex()


def compute_cid_v1(data: bytes, codec: str = "raw") -> str:
    h = compute_chunk_hash(data)
    return f"bafy-arkhe-{codec}-{h[:32]}"


class NonceRegistry:
    """Tracks observed nonces to guarantee replay protection (I-03)."""
    def __init__(self):
        self._used_nonces: Set[str] = set()

    def check_and_record(self, nonce: str) -> bool:
        if not nonce or nonce in self._used_nonces:
            return False
        self._used_nonces.add(nonce)
        return True

    def is_used(self, nonce: str) -> bool:
        return nonce in self._used_nonces

    def clear(self):
        self._used_nonces.clear()


class MLDSA65Simulator:
    """Simulated Post-Quantum ML-DSA-65 signature scheme."""
    @staticmethod
    def sign(private_key_seed: str, message: bytes) -> str:
        h = hashlib.sha3_512(private_key_seed.encode('utf-8') + b":ML-DSA-65:" + message).hexdigest()
        return f"mldsa65-sig:{h}"

    @staticmethod
    def verify(public_key: str, message: bytes, signature: str) -> bool:
        if not signature or not signature.startswith("mldsa65-sig:"):
            return False
        if not public_key.startswith("mldsa65-pub:"):
            return False
        pub_id = public_key.split(":", 1)[1]
        expected_sig = MLDSA65Simulator.sign(f"seed-{pub_id}", message)
        return signature == expected_sig


class AfirReceipt:
    REQUIRED_FIELDS = {
        "receipt_id", "epoch", "worker_did", "task_cid",
        "result_hash", "nonce", "timestamp", "signature"
    }

    def __init__(self, data: Dict[str, Any]):
        self.data = dict(data)
        self.validate_schema()

    def validate_schema(self):
        missing = self.REQUIRED_FIELDS - set(self.data.keys())
        if missing:
            raise ValueError(f"AFiR receipt missing required fields: {sorted(missing)}")
        if not isinstance(self.data["epoch"], int) or self.data["epoch"] < 0:
            raise ValueError("epoch must be non-negative integer")
        if not isinstance(self.data["nonce"], str) or len(self.data["nonce"]) < 8:
            raise ValueError("nonce must be at least 8 chars")

    def get_signing_bytes(self) -> bytes:
        signable_payload = {k: v for k, v in self.data.items() if k != "signature"}
        return canonical_json_bytes(signable_payload)

    def calculate_hash(self) -> str:
        return compute_chunk_hash(canonical_json_bytes(self.data))

    def verify_signature(self, public_key: str) -> bool:
        sig = self.data.get("signature", "")
        msg = self.get_signing_bytes()
        return MLDSA65Simulator.verify(public_key, msg, sig)
