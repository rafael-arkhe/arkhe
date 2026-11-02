"""
Rollback Engine for Arkhe-Test.
"""

from typing import Dict, Any, List, Optional
from .crypto import compute_chunk_hash, canonical_json_bytes
from .mmr import MerkleMountainRange


class RollbackViolation(Exception):
    pass


class SystemState:
    def __init__(self, code_version: str, config: Dict[str, Any]):
        self.code_version = code_version
        self.config = dict(config)
        self.mmr = MerkleMountainRange()
        self.receipts: List[Dict[str, Any]] = []
        self.checkpoints: List[Dict[str, Any]] = []

    def record_receipt(self, receipt_data: Dict[str, Any]):
        h = compute_chunk_hash(canonical_json_bytes(receipt_data))
        self.receipts.append(receipt_data)
        self.mmr.append(h)

    def record_checkpoint(self, checkpoint_data: Dict[str, Any]):
        self.checkpoints.append(checkpoint_data)


class RollbackManager:
    @staticmethod
    def execute_rollback(
        current_state: SystemState,
        target_code_version: str,
        target_config: Dict[str, Any],
        attempt_evidence_prune: bool = False
    ) -> SystemState:
        if attempt_evidence_prune:
            raise RollbackViolation(
                "ROLLBACK SECURITY HALT: Cannot rollback MMR, receipts, or checkpoints! Evidence is immutable."
            )

        initial_leaf_count = len(current_state.mmr.leaves)
        initial_root = current_state.mmr.get_root()
        initial_receipts_count = len(current_state.receipts)
        initial_checkpoints_count = len(current_state.checkpoints)

        current_state.code_version = target_code_version
        current_state.config = dict(target_config)

        if len(current_state.mmr.leaves) != initial_leaf_count or current_state.mmr.get_root() != initial_root:
            raise RollbackViolation("Integrity check failed: MMR state changed during rollback!")

        if len(current_state.receipts) != initial_receipts_count:
            raise RollbackViolation("Integrity check failed: Receipts altered during rollback!")

        if len(current_state.checkpoints) != initial_checkpoints_count:
            raise RollbackViolation("Integrity check failed: Checkpoints altered during rollback!")

        return current_state
