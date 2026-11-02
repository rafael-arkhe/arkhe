"""
Integration Test Suite.
"""

import pytest
from core.crypto import (
    compute_chunk_hash,
    compute_cid_v1,
    canonical_json_bytes,
    AfirReceipt,
    NonceRegistry,
    MLDSA65Simulator
)
from core.mmr import MerkleMountainRange
from core.invariants import InvariantVerifier, InvariantViolation
from core.rollback import RollbackManager, SystemState, RollbackViolation


class MockNode:
    def __init__(self, node_id: str, seed: str):
        self.node_id = node_id
        self.pubkey = f"mldsa65-pub:{node_id}"
        self.seed = seed
        self.storage = {}
        self.mmr = MerkleMountainRange()
        self.receipts = []

    def store_chunk(self, chunk: bytes) -> str:
        cid = compute_cid_v1(chunk)
        self.storage[cid] = chunk
        return cid

    def execute_and_sign_receipt(self, task_cid: str, result_hash: str, epoch: int, nonce: str) -> AfirReceipt:
        payload = {
            "receipt_id": f"rec-{self.node_id}-{epoch}-{nonce[:6]}",
            "epoch": epoch,
            "worker_did": f"did:arkhe:{self.node_id}",
            "task_cid": task_cid,
            "result_hash": result_hash,
            "nonce": nonce,
            "timestamp": 1761984000
        }
        sig = MLDSA65Simulator.sign(self.seed, canonical_json_bytes(payload))
        payload["signature"] = sig
        rec = AfirReceipt(payload)
        self.receipts.append(rec)
        self.mmr.append(rec.calculate_hash())
        return rec


def test_node_chunk_exchange_and_tamper_rejection():
    node1 = MockNode("worker-alpha", "seed-worker-alpha")
    node2 = MockNode("worker-beta", "seed-worker-beta")

    raw_weights = b"matrix-weights-v1-float16-arkhe"
    cid = node1.store_chunk(raw_weights)

    expected_hash = compute_chunk_hash(raw_weights)
    assert InvariantVerifier.verify_chunk_integrity(raw_weights, expected_hash) is True

    corrupted_chunk = b"matrix-weights-v1-corrupted-bad"
    with pytest.raises(InvariantViolation):
        InvariantVerifier.verify_chunk_integrity(corrupted_chunk, expected_hash)


def test_afir_anchoring_and_mmr_history_tamper():
    worker = MockNode("worker-alpha", "seed-worker-alpha")
    registry = NonceRegistry()

    r1 = worker.execute_and_sign_receipt("bafy-task-1", "00"*32, 10, "nonce-seq-10001")
    assert InvariantVerifier.verify_afir_receipt(r1, worker.pubkey, registry) is True

    snapshot_leaves = list(worker.mmr.leaves)

    r2 = worker.execute_and_sign_receipt("bafy-task-2", "11"*32, 10, "nonce-seq-10002")
    assert InvariantVerifier.verify_afir_receipt(r2, worker.pubkey, registry) is True

    assert InvariantVerifier.verify_mmr_append_only(snapshot_leaves, worker.mmr.leaves) is True

    tampered_leaves = list(worker.mmr.leaves)
    tampered_leaves[0] = "ff"*32
    with pytest.raises(InvariantViolation, match="I-04 VIOLATION"):
        InvariantVerifier.verify_mmr_append_only(snapshot_leaves, tampered_leaves)


def test_offline_bundle_and_safe_rollback_pipeline():
    state = SystemState(code_version="v2.1.0", config={"telemetry": "high"})
    state.record_receipt({"receipt_id": "r100", "task": "inference", "nonce": "n10001"})
    state.record_receipt({"receipt_id": "r101", "task": "inference", "nonce": "n10002"})

    bundle = {
        "bundle_id": "arkbundle-export-01",
        "version": "1.0",
        "mmr_root": state.mmr.get_root(),
        "receipts": state.receipts,
        "signatures": ["sig-valid-1", "sig-valid-2"]
    }
    bundle["checksum"] = compute_chunk_hash(canonical_json_bytes(bundle))

    assert InvariantVerifier.verify_chain_agnostic_evidence(bundle) is True

    prior_root = state.mmr.get_root()
    RollbackManager.execute_rollback(state, "v2.0.0", {"telemetry": "standard"})

    assert state.code_version == "v2.0.0"
    assert state.config["telemetry"] == "standard"
    assert state.mmr.get_root() == prior_root
    assert len(state.receipts) == 2

    with pytest.raises(RollbackViolation):
        RollbackManager.execute_rollback(state, "v1.0.0", {}, attempt_evidence_prune=True)
