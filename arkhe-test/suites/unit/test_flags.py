import pytest
from core.flags import FeatureFlagManager, FlagPolicyViolation

def test_flag_policy_audit():
    ffm = FeatureFlagManager()
    violations = ffm.audit_flags()
    assert len(violations) == 0

def test_unregistered_flag_raises_error():
    ffm = FeatureFlagManager()
    with pytest.raises(FlagPolicyViolation, match="Unregistered flag"):
        ffm.set_flag("unauthorized.bypass_security", True)

def test_flag_defaults_match():
    ffm = FeatureFlagManager()
    assert ffm.is_enabled("network.experimental_transport") is False
    assert ffm.is_enabled("moe.receipt_tree") is True
