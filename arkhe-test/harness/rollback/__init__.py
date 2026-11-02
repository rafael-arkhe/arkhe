from core.rollback import RollbackManager, SystemState

class RollbackTester:
    @staticmethod
    def verify_safe_rollback_invariance(state: SystemState, target_version: str, target_config: dict) -> bool:
        initial_evidence_count = len(state.receipts)
        initial_mmr_root = state.mmr.get_root()

        RollbackManager.execute_rollback(state, target_version, target_config)

        return (
            state.code_version == target_version and
            len(state.receipts) == initial_evidence_count and
            state.mmr.get_root() == initial_mmr_root
        )
