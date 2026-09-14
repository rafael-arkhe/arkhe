"""Testes do orquestrador local e da TemporalChain (append-only)."""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "agents" / "python"))

from sophia_local.orchestrator import LocalOrchestrator
from sophia_local.temporal_bridge import TemporalBridge


def test_orchestrator_anchors_cycles(tmp_path) -> None:
    orch = LocalOrchestrator(
        ledger_path=tmp_path / "ledger.jsonl",
        state_path=tmp_path / "state.json",
    )
    seals = {orch.run_full_cycle() for _ in range(5)}
    assert len(seals) == 5
    ok, count = orch.bridge.verify()
    assert ok and count == 5


def test_state_file_written(tmp_path) -> None:
    orch = LocalOrchestrator(
        ledger_path=tmp_path / "ledger.jsonl",
        state_path=tmp_path / "state.json",
    )
    orch.run_full_cycle()
    assert (tmp_path / "state.json").exists()


def test_temporal_bridge_tamper_detected(tmp_path) -> None:
    br = TemporalBridge(tmp_path / "ledger.jsonl")
    br.anchor("a", {"x": 1})
    br.anchor("b", {"x": 2})

    ok, count = br.verify()
    assert ok and count == 2

    # adulteração: reescreve o último registro com outro seal
    lines = (tmp_path / "ledger.jsonl").read_text(encoding="utf-8").strip().splitlines()
    tampered = lines[0].replace('"event": "a"', '"event": "MALICIOUS"')
    (tmp_path / "ledger.jsonl").write_text("\n".join([tampered, lines[1]]) + "\n", encoding="utf-8")
    ok_after, _ = br.verify()
    assert not ok_after


def test_linguistic_agent_report_generated(tmp_path) -> None:
    from sophia_local.linguistic_agent import LinguisticAgent

    agent = LinguisticAgent(output_dir=tmp_path / "output")
    result = agent.run_iteration(psi_c=0.8, q=0.3, detections=["aleph", "beth"])
    assert "aleph" in result and "beth" in result
    assert result["aleph"]["psi_C"] == 0.8
    assert (tmp_path / "output" / "linguistic_metrics.json").exists()