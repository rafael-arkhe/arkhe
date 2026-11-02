"""
Adversarial Invariant Test Suite (I-01 through I-10).
"""

import json
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


# --- I-01: Chunk Integrity ---
def test_i01_chunk_integrity_valid():
    data = b"valid chunk content for arkhe neural weights"
    expected_hash = compute_chunk_hash(data)
    assert InvariantVerifier.verify_chunk_integrity(data, expected_hash) is True

def test_i01_chunk_integrity_one_byte_tampered():
    data = bytearray(b"valid chunk content for arkhe neural weights")
    expected_hash = compute_chunk_hash(bytes(data))
    data[5] ^= 0xFF
    with pytest.raises(InvariantViolation, match="I-01 VIOLATION"):
        InvariantVerifier.verify_chunk_integrity(bytes(data), expected_hash)


# --- I-02: Verifiable Manifest ---
def test_i02_manifest_valid():
    chunk1 = b"chunk 1 payload"
    chunk2 = b"chunk 2 payload"
    cid1 = compute_cid_v1(chunk1)
    cid2 = compute_cid_v1(chunk2)
    manifest = {
        "manifest_version": "1.0",
        "chunks": [
            {"cid": cid1, "hash": compute_chunk_hash(chunk1)},
            {"cid": cid2, "hash": compute_chunk_hash(chunk2)},
        ]
    }
    chunks = {cid1: chunk1, cid2: chunk2}
    assert InvariantVerifier.verify_manifest(manifest, chunks) is True

def test_i02_manifest_swapped_cid_rejected():
    chunk1 = b"chunk 1 payload"
    chunk2 = b"chunk 2 payload"
    cid1 = compute_cid_v1(chunk1)
    cid2 = compute_cid_v1(chunk2)
    manifest = {
        "manifest_version": "1.0",
        "chunks": [
            {"cid": cid1, "hash": compute_chunk_hash(chunk2)},
        ]
    }
    chunks = {cid1: chunk2}
    with pytest.raises(InvariantViolation, match="I-02 VIOLATION"):
        InvariantVerifier.verify_manifest(manifest, chunks)

def test_i02_manifest_omitted_chunk_rejected():
    chunk1 = b"chunk 1"
    cid1 = compute_cid_v1(chunk1)
    manifest = {
        "manifest_version": "1.0",
        "chunks": [
            {"cid": cid1, "hash": compute_chunk_hash(chunk1)},
            {"cid": "bafy-missing-chunk", "hash": "00"*32}
        ]
    }
    chunks = {cid1: chunk1}
    with pytest.raises(InvariantViolation, match="I-02 VIOLATION"):
        InvariantVerifier.verify_manifest(manifest, chunks)


# --- I-03: AFiR Receipt (Signature & Nonce) ---
def test_i03_afir_valid():
    with open("vectors/afir/valid_receipt.json") as f:
        data = json.load(f)
    receipt = AfirReceipt(data)
    registry = NonceRegistry()
    assert InvariantVerifier.verify_afir_receipt(receipt, "mldsa65-pub:worker-alpha", registry) is True

def test_i03_afir_wrong_key_rejected():
    with open("vectors/afir/valid_receipt.json") as f:
        data = json.load(f)
    receipt = AfirReceipt(data)
    registry = NonceRegistry()
    with pytest.raises(InvariantViolation, match="I-03 VIOLATION: Invalid signature"):
        InvariantVerifier.verify_afir_receipt(receipt, "mldsa65-pub:wrong-key", registry)

def test_i03_afir_nonce_reuse_rejected():
    with open("vectors/afir/valid_receipt.json") as f:
        data = json.load(f)
    receipt = AfirReceipt(data)
    registry = NonceRegistry()
    assert InvariantVerifier.verify_afir_receipt(receipt, "mldsa65-pub:worker-alpha", registry) is True
    with pytest.raises(InvariantViolation, match="I-03 VIOLATION: Replayed or invalid nonce"):
        InvariantVerifier.verify_afir_receipt(receipt, "mldsa65-pub:worker-alpha", registry)


# --- I-04: MMR Append-Only ---
def test_i04_mmr_append_only_valid():
    prev_leaves = ["11" * 32, "22" * 32]
    curr_leaves = ["11" * 32, "22" * 32, "33" * 32]
    assert InvariantVerifier.verify_mmr_append_only(prev_leaves, curr_leaves) is True

def test_i04_mmr_reordering_rejected():
    prev_leaves = ["11" * 32, "22" * 32]
    curr_leaves = ["22" * 32, "11" * 32, "33" * 32]
    with pytest.raises(InvariantViolation, match="I-04 VIOLATION: MMR leaf at position 0 rewritten"):
        InvariantVerifier.verify_mmr_append_only(prev_leaves, curr_leaves)

def test_i04_mmr_truncation_rejected():
    prev_leaves = ["11" * 32, "22" * 32, "33" * 32]
    curr_leaves = ["11" * 32, "22" * 32]
    with pytest.raises(InvariantViolation, match="I-04 VIOLATION: MMR truncation detected"):
        InvariantVerifier.verify_mmr_append_only(prev_leaves, curr_leaves)


# --- I-05: Offline Audit Bundle ---
def _build_test_bundle():
    receipts = [
        {"id": "r1", "payload": "task1"},
        {"id": "r2", "payload": "task2"}
    ]
    r_hashes = [compute_chunk_hash(canonical_json_bytes(r)) for r in receipts]
    mmr = MerkleMountainRange(r_hashes)
    bundle_payload = {
        "bundle_id": "bundle-001",
        "version": "1.0",
        "mmr_root": mmr.get_root(),
        "receipts": receipts,
        "signatures": ["sig1", "sig2"]
    }
    checksum = compute_chunk_hash(canonical_json_bytes(bundle_payload))
    bundle_payload["checksum"] = checksum
    return bundle_payload

def test_i05_offline_bundle_valid():
    bundle = _build_test_bundle()
    assert InvariantVerifier.verify_offline_bundle(bundle) is True

def test_i05_corrupted_bundle_rejected():
    bundle = _build_test_bundle()
    bundle["receipts"][0]["payload"] = "tampered"
    with pytest.raises(InvariantViolation, match="I-05 VIOLATION"):
        InvariantVerifier.verify_offline_bundle(bundle)


# --- I-06: Device Binding (TPM/TEE) ---
def test_i06_device_binding_valid():
    with open("vectors/checkpoint/valid_checkpoint.json") as f:
        cp = json.load(f)
    valid_quotes = {"quote-tpm-valid-sha3-verified"}
    assert InvariantVerifier.verify_device_binding(cp, valid_quotes) is True

def test_i06_unattested_device_rejected():
    with open("vectors/checkpoint/invalid_checkpoint.json") as f:
        cp = json.load(f)
    valid_quotes = {"quote-tpm-valid-sha3-verified"}
    with pytest.raises(InvariantViolation, match="I-06 VIOLATION"):
        InvariantVerifier.verify_device_binding(cp, valid_quotes)


# --- I-07: Human Governance Protection ---
def test_i07_human_ratified_proposal_accepted():
    proposal = {
        "id": "prop-01",
        "target_subsystem": "constitution",
        "action": "amend_constitution",
        "proposer_type": "autonomous_agent",
        "human_ratification_sig": "sig-human-council-001"
    }
    assert InvariantVerifier.verify_governance_proposal(proposal) is True

def test_i07_autonomous_proposal_violating_constitution_rejected():
    proposal = {
        "id": "prop-02",
        "target_subsystem": "invariants",
        "action": "amend_constitution",
        "proposer_type": "autonomous_agent",
        "human_ratification_sig": None
    }
    with pytest.raises(InvariantViolation, match="I-07 VIOLATION"):
        InvariantVerifier.verify_governance_proposal(proposal)


# --- I-08: Proof of Reward Condition ---
def test_i08_reward_with_valid_receipt_and_mmr_accepted():
    receipt_id = "afir-rec-001"
    r_hash = compute_chunk_hash(b"afir-rec-001-data")
    mmr = MerkleMountainRange([r_hash])
    claim = {
        "receipt_id": receipt_id,
        "mmr_leaf_index": 0,
        "leaf_hash": r_hash,
        "reward_amount": 1000
    }
    assert InvariantVerifier.verify_reward_distribution(claim, {receipt_id}, mmr) is True

def test_i08_reward_without_verified_receipt_rejected():
    receipt_id = "unverified-rec"
    mmr = MerkleMountainRange([compute_chunk_hash(b"dummy")])
    claim = {
        "receipt_id": receipt_id,
        "mmr_leaf_index": 0,
        "leaf_hash": "00"*32,
        "reward_amount": 1000
    }
    with pytest.raises(InvariantViolation, match="I-08 VIOLATION: Reward claim rejected"):
        InvariantVerifier.verify_reward_distribution(claim, {"other-rec"}, mmr)


# --- I-09: Chain-Agnostic Offline Verification ---
def test_i09_chain_agnostic_offline_success():
    bundle = _build_test_bundle()
    assert InvariantVerifier.verify_chain_agnostic_evidence(bundle, external_chain_online=False) is True


# --- I-10: Disjoint Roles in Epoch ---
def test_i10_disjoint_roles_valid():
    assert InvariantVerifier.verify_role_separation(
        epoch=10,
        worker_did="did:arkhe:worker-01",
        validator_did="did:arkhe:validator-02"
    ) is True

def test_i10_worker_validating_in_same_epoch_rejected():
    with pytest.raises(InvariantViolation, match="I-10 VIOLATION: Role collision"):
        InvariantVerifier.verify_role_separation(
            epoch=10,
            worker_did="did:arkhe:entity-dual",
            validator_did="did:arkhe:entity-dual"
        )
