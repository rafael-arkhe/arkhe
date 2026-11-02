"""
Feature Flag Policy Engine.
"""

import sys
import tomllib
from pathlib import Path
from typing import Dict, Any, List, Set, Optional

FORBIDDEN_FLAG_CONTROLS = {
    "signature_check",
    "hash_check",
    "mmr_append_only",
    "role_separation",
    "chunk_validation",
    "receipt_signature",
    "nonce_verification",
    "root_verification",
    "leaf_proof",
    "offline_evidence_validity",
    "minimum_mandatory_verification",
    "human_canonification",
    "constitution_amendment",
    "self_modification",
}


class FlagPolicyViolation(Exception):
    pass


class FeatureFlagManager:
    def __init__(self, policy_path: Optional[str] = None):
        self.policy_path = policy_path or str(Path(__file__).parents[1] / "policies" / "flags.toml")
        self.policy = self._load_policy()
        self.active_flags: Dict[str, bool] = {}
        self._init_defaults()

    def _load_policy(self) -> Dict[str, Any]:
        with open(self.policy_path, "rb") as f:
            return tomllib.load(f)

    def _init_defaults(self):
        flags_def = self.policy.get("flags", {})
        for name, config in flags_def.items():
            self.active_flags[name] = config.get("default", False)

    def is_enabled(self, flag_name: str) -> bool:
        return self.active_flags.get(flag_name, False)

    def set_flag(self, flag_name: str, enabled: bool):
        flags_def = self.policy.get("flags", {})
        if flag_name not in flags_def:
            raise FlagPolicyViolation(f"Unregistered flag attempted: '{flag_name}'")
        self.active_flags[flag_name] = enabled

    def audit_flags(self) -> List[str]:
        violations = []
        flags_def = self.policy.get("flags", {})
        for name, cfg in flags_def.items():
            allowed = set(cfg.get("allowed_controls", []))
            compromised = allowed.intersection(FORBIDDEN_FLAG_CONTROLS)
            if compromised:
                violations.append(
                    f"Flag '{name}' illegally permits controlling security invariants: {sorted(compromised)}"
                )
        return violations

    def validate_or_panic(self):
        violations = self.audit_flags()
        if violations:
            raise FlagPolicyViolation(f"Feature flag policy compromised: {violations}")
        return True
