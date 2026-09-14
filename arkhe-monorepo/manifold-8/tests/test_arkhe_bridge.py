"""
Testes de integração do ArkheBridge com o pipeline (modo simulação).
Execução: pytest tests/ -v   (a partir da raiz manifold-8)
"""

import sys
import os

sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), "..")))

import numpy as np
import json
import pytest
from datetime import datetime, timezone

from src.arkhe_bridge import ArkheBridge, ArkhePriority, TopologicalValidation
from src.models import Event, ProcessedEvent


class TestArkheBridge:
    """Testes do ArkheBridge integrado ao pipeline."""

    @pytest.fixture(autouse=True)
    def setup(self):
        os.environ["SIMULATION_MODE"] = "true"
        self.bridge = ArkheBridge()
        yield

    def _manual_setup(self):
        """Equivalente não-fixture para execução direta (python tests/...)."""
        os.environ["SIMULATION_MODE"] = "true"
        self.bridge = ArkheBridge()

    def test_initialization(self):
        assert self.bridge.session_id is not None
        assert len(self.bridge.session_id) == 16
        assert self.bridge.headers["X-Session-Id"] == self.bridge.session_id
        assert self.bridge.headers["X-Client-Version"] == "8.1"

    def test_health_check(self):
        health = self.bridge.health_check()
        assert health["bridge"] == "healthy"
        assert health["session"] == self.bridge.session_id
        assert "roquo" in health
        assert "glass5d" in health
        assert "sqc" in health

    def test_topological_validation(self):
        t = np.linspace(0, 1, 44100)
        signal = np.sin(2 * np.pi * 963 * t) + 0.5 * np.sin(2 * np.pi * 440 * t)
        result = self.bridge.validate_topological(signal)
        assert "winding_number" in result
        assert "resilience" in result
        assert "validation" in result
        assert "topological_charge" in result
        assert result["signal_length"] == len(signal)
        assert result["validation"] in [
            TopologicalValidation.VALID.value,
            TopologicalValidation.DEGRADED.value,
        ]

    def test_roquo_job_submission(self):
        payload = {"test_data": [1, 2, 3, 4, 5], "timestamp": datetime.now(timezone.utc).isoformat()}
        result = self.bridge.submit_job_to_roquo(payload, priority=ArkhePriority.HIGH, timeout_seconds=60)
        assert "job_id" in result
        assert len(result["job_id"]) > 0

    def test_glass5d_write_read(self):
        test_data = {
            "message": "Hello Arkhe",
            "timestamp": datetime.now(timezone.utc).isoformat(),
            "test_id": "integration_test_001",
        }
        write_result = self.bridge.write_to_glass5d(data=test_data, dataset_name="integration_test", layer_count=1)
        assert "record_id" in write_result
        record_id = write_result["record_id"]
        read_result = self.bridge.read_from_glass5d(record_id)
        assert read_result is not None
        assert "data" in read_result

    def test_engram_processing(self):
        raw_data = {
            "source": "integration_test",
            "data": {"value": 963, "unit": "Hz", "description": "Frequência de ressonância"},
        }
        t = np.linspace(0, 1, 44100)
        signal = np.sin(2 * np.pi * 963 * t)
        result = self.bridge.process_engram(
            raw_data=raw_data,
            signal=signal,
            dataset_name="test_engram_integration",
            priority=ArkhePriority.NORMAL,
            validate_topology=True,
            wait_for_roquo=False,
        )
        assert result["status"] in ["success", "failed"]
        assert "steps" in result
        assert "topological_validation" in result["steps"]
        assert "roquo_job" in result["steps"]
        assert "glass5d_archive" in result["steps"]

    def test_verbal_chemistry_integration(self):
        from core.verbal_chemistry import VerbalStatement
        from verbal_events_processor import VerbalEventProcessor

        statement = VerbalStatement.from_text("I am healing and strong")
        processor = VerbalEventProcessor()
        event = processor.verbal_statement_to_event(statement)
        payload = {
            "event": event.to_dict(),
            "verbal_analysis": {
                "polarity": statement.polarity.name,
                "emotional_charge": statement.emotional_charge,
                "biochemical_impact": statement.biochemical_impact,
            },
        }
        result = self.bridge.process_engram(
            raw_data=payload,
            signal=np.array([statement.emotional_charge] * 100),
            dataset_name=f"verbal_engram_{datetime.now(timezone.utc).strftime('%Y%m%d')}",
            validate_topology=True,
            wait_for_roquo=False,
        )
        assert result["status"] in ["success", "failed"]

    def test_status_tracking(self):
        status = self.bridge.get_status()
        assert "session_id" in status
        assert "initialized_at" in status
        assert "engrams_processed" in status
        assert "glass5d_bytes_written" in status
        assert "cached_jobs" in status
        assert "cached_glass5d" in status

    def test_session_reset(self):
        old_session = self.bridge.session_id
        new_session = self.bridge.reset_session()
        assert new_session != old_session
        assert self.bridge.headers["X-Session-Id"] == new_session

    def test_event_models(self):
        event = Event(source="test", event_type="verbal", payload={"x": 1})
        assert event.event_id.startswith("evt-")
        processed = ProcessedEvent(event_id=event.event_id, source="test", polarity="COHERENT")
        assert processed.status == "processed"
        assert "event_id" in processed.to_dict()


if __name__ == "__main__":
    print("🧪 Running ArkheBridge Integration Tests...")

    tester = TestArkheBridge()
    tester._manual_setup()

    tests = [
        ("Initialization", tester.test_initialization),
        ("Health Check", tester.test_health_check),
        ("Topological Validation", tester.test_topological_validation),
        ("ROQUO Job Submission", tester.test_roquo_job_submission),
        ("Glass5D Write/Read", tester.test_glass5d_write_read),
        ("Engram Processing", tester.test_engram_processing),
        ("Verbal Chemistry Integration", tester.test_verbal_chemistry_integration),
        ("Status Tracking", tester.test_status_tracking),
        ("Session Reset", tester.test_session_reset),
        ("Event Models", tester.test_event_models),
    ]

    results = []
    for name, test_func in tests:
        try:
            test_func()
            results.append((name, "✅ PASSED"))
            print(f"  {name}: ✅ PASSED")
        except Exception as e:
            results.append((name, f"❌ FAILED: {e}"))
            print(f"  {name}: ❌ FAILED - {e}")

    print(f"\n{'='*50}")
    print(f"TESTS: {sum(1 for _, r in results if 'PASSED' in r)}/{len(results)} passed")

    if all("PASSED" in r for _, r in results):
        print("🎉 ALL INTEGRATION TESTS PASSED!")
    else:
        print("⚠️ Some tests failed. Check integration.")
    print("=" * 50)