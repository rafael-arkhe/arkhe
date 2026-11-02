"""
Core Invariant Verifiers (I-01 through I-10) for Arkhe-Test.
"""

import hashlib
from typing import Dict, Any, List, Optional, Tuple, Set
from .crypto import (
    compute_chunk_hash,
    compute_cid_v1,
    canonical_json_bytes,
    AfirReceipt,
    NonceRegistry,
    MLDSA65Simulator
)
from .mmr import MerkleMountainRange


class InvariantViolation(Exception):
    pass


class InvariantVerifier:
    @staticmethod
    def verify_chunk_integrity(raw_data: bytes, expected_hash: str) -> bool:
        actual_hash = compute_chunk_hash(raw_data)
        if actual_hash != expected_hash:
            raise InvariantViolation(
                f"I-01 VIOLATION: Chunk integrity mismatch! Expected {expected_hash}, got {actual_hash}"
            )
        return True

    @staticmethod
    def verify_manifest(manifest: Dict[str, Any], chunks: Dict[str, bytes]) -> bool:
        declared_chunks = manifest.get("chunks", [])
        if not declared_chunks:
            raise InvariantViolation("I-02 VIOLATION: Manifest contains empty chunk list")

        for c_entry in declared_chunks:
            cid = c_entry.get("cid")
            expected_hash = c_entry.get("hash")
            if not cid or cid not in chunks:
                raise InvariantViolation(f"I-02 VIOLATION: Chunk {cid} missing from payload")
            data = chunks[cid]
            if compute_chunk_hash(data) != expected_hash:
                raise InvariantViolation(f"I-02 VIOLATION: Chunk {cid} content does not match manifest hash")
            computed_cid = compute_cid_v1(data)
            if computed_cid != cid:
                raise InvariantViolation(f"I-02 VIOLATION: Chunk CID {cid} divergent from content CID {computed_cid}")

        return True

    @staticmethod
    def verify_afir_receipt(
        receipt: AfirReceipt,
        expected_pubkey: str,
        nonce_registry: NonceRegistry
    ) -> bool:
        nonce = receipt.data.get("nonce", "")
        if not nonce_registry.check_and_record(nonce):
            raise InvariantViolation(f"I-03 VIOLATION: Replayed or invalid nonce {nonce}")

        if not receipt.verify_signature(expected_pubkey):
            raise InvariantViolation(
                f"I-03 VIOLATION: Invalid signature on AFiR receipt {receipt.data.get('receipt_id')}"
            )

        return True

    @staticmethod
    def verify_mmr_append_only(
        previous_mmr_leaves: List[str],
        current_mmr_leaves: List[str]
    ) -> bool:
        if len(current_mmr_leaves) < len(previous_mmr_leaves):
            raise InvariantViolation(
                f"I-04 VIOLATION: MMR truncation detected! Current {len(current_mmr_leaves)} < previous {len(previous_mmr_leaves)}"
            )

        for i, prev_leaf in enumerate(previous_mmr_leaves):
            if current_mmr_leaves[i] != prev_leaf:
                raise InvariantViolation(
                    f"I-04 VIOLATION: MMR leaf at position {i} rewritten or reordered"
                )
        return True

    @staticmethod
    def verify_offline_bundle(bundle: Dict[str, Any]) -> bool:
        required_sections = {"bundle_id", "version", "mmr_root", "receipts", "signatures", "checksum"}
        missing = required_sections - set(bundle.keys())
        if missing:
            raise InvariantViolation(f"I-05 VIOLATION: Incomplete bundle, missing {sorted(missing)}")

        payload_to_verify = {k: v for k, v in bundle.items() if k != "checksum"}
        computed_checksum = compute_chunk_hash(canonical_json_bytes(payload_to_verify))
        if computed_checksum != bundle["checksum"]:
            raise InvariantViolation("I-05 VIOLATION: Offline bundle checksum mismatch, bundle corrupted")

        receipt_hashes = [compute_chunk_hash(canonical_json_bytes(r)) for r in bundle["receipts"]]
        mmr = MerkleMountainRange(receipt_hashes)
        if mmr.get_root() != bundle["mmr_root"]:
            raise InvariantViolation("I-05 VIOLATION: Bundle MMR root does not match receipt set")

        return True

    @staticmethod
    def verify_device_binding(checkpoint: Dict[str, Any], valid_tpm_attestations: Set[str]) -> bool:
        device_id = checkpoint.get("device_id")
        attestation_quote = checkpoint.get("attestation_quote")
        if not device_id or not attestation_quote:
            raise InvariantViolation("I-06 VIOLATION: Missing device binding credentials in checkpoint")
        if attestation_quote not in valid_tpm_attestations:
            raise InvariantViolation(
                f"I-06 VIOLATION: Device attestation quote '{attestation_quote}' not in valid TPM quote store"
            )
        return True

    @staticmethod
    def verify_governance_proposal(proposal: Dict[str, Any]) -> bool:
        target = proposal.get("target_subsystem", "")
        action = proposal.get("action", "")
        proposer_type = proposal.get("proposer_type", "")
        human_signature = proposal.get("human_ratification_sig")

        CONSTITUTIONAL_TARGETS = {
            "constitution",
            "invariants",
            "fail_closed_policy",
            "core_rules",
            "security_boundaries"
        }

        if target in CONSTITUTIONAL_TARGETS or action == "amend_constitution":
            if proposer_type == "autonomous_agent" and not human_signature:
                raise InvariantViolation(
                    f"I-07 VIOLATION: Autonomous proposal {proposal.get('id')} attempted to alter constitution without human ratification!"
                )
        return True

    @staticmethod
    def verify_reward_distribution(
        claim: Dict[str, Any],
        verified_receipt_ids: Set[str],
        mmr: MerkleMountainRange
    ) -> bool:
        receipt_id = claim.get("receipt_id")
        claimed_leaf_index = claim.get("mmr_leaf_index")
        claimed_leaf_hash = claim.get("leaf_hash")

        if not receipt_id or receipt_id not in verified_receipt_ids:
            raise InvariantViolation(
                f"I-08 VIOLATION: Reward claim rejected: Receipt {receipt_id} is not verified"
            )

        if claimed_leaf_index is None or claimed_leaf_index >= len(mmr.leaves):
            raise InvariantViolation(
                f"I-08 VIOLATION: Reward claim leaf index {claimed_leaf_index} out of MMR bounds"
            )

        if mmr.leaves[claimed_leaf_index] != claimed_leaf_hash:
            raise InvariantViolation(
                "I-08 VIOLATION: MMR leaf hash mismatch for reward claim"
            )
        return True

    @staticmethod
    def verify_chain_agnostic_evidence(bundle: Dict[str, Any], external_chain_online: bool = False) -> bool:
        try:
            return InvariantVerifier.verify_offline_bundle(bundle)
        except Exception as e:
            raise InvariantViolation(f"I-09 VIOLATION: Chain-agnostic verification failed offline: {e}")

    @staticmethod
    def verify_role_separation(epoch: int, worker_did: str, validator_did: str) -> bool:
        if worker_did == validator_did:
            raise InvariantViolation(
                f"I-10 VIOLATION: Role collision in epoch {epoch}! Entity {worker_did} cannot act as both Worker and Validator"
            )
        return True
