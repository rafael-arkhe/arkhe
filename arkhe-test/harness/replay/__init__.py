from core.crypto import NonceRegistry

class ReplayTester:
    def __init__(self):
        self.registry = NonceRegistry()

    def test_replay_rejection(self, nonce: str) -> bool:
        assert self.registry.check_and_record(nonce) is True
        return not self.registry.check_and_record(nonce)
