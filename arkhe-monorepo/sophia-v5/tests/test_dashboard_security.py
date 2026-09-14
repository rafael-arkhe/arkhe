"""Testes do dashboard (Flask) e da segurança (integridade)."""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "agents" / "python"))
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "dashboard"))

from sophia_local.security.integrity import IntegrityMonitor


def test_integrity_baseline_detects_new_file(tmp_path) -> None:
    src = tmp_path / "watched"
    src.mkdir()
    (src / "a.bin").write_bytes(b"content-a")

    mon = IntegrityMonitor(
        watch_dir=src,
        baseline_path=tmp_path / "file_hashes.json",
    )
    mon.build_baseline()

    # adiciona um arquivo não-baseline
    (src / "new.bin").write_bytes(b"content-new")
    alerts = mon.check_integrity()
    assert any("Novo arquivo detectado" in a for a in alerts)


def test_integrity_detects_tamper(tmp_path) -> None:
    src = tmp_path / "watched"
    src.mkdir()
    target = src / "a.bin"
    target.write_bytes(b"content-a")

    mon = IntegrityMonitor(
        watch_dir=src,
        baseline_path=tmp_path / "file_hashes.json",
    )
    mon.build_baseline()

    target.write_bytes(b"TAMPERED")
    alerts = mon.check_integrity()
    assert any("Arquivo modificado" in a for a in alerts)


def test_dashboard_status_endpoint(tmp_path, monkeypatch) -> None:
    import json

    import app as dashboard_app

    monkeypatch.chdir(tmp_path)
    dashboard_app._run_dir = tmp_path
    dashboard_app.orchestrator = dashboard_app.LocalOrchestrator(
        state_path=tmp_path / "state.json",
        ledger_path=tmp_path / "ledger.jsonl",
    )
    dashboard_app.orchestrator.run_full_cycle()

    client = dashboard_app.app.test_client()
    resp = client.get("/api/status")
    assert resp.status_code == 200
    body = json.loads(resp.data)
    assert "orchestrator" in body
    assert body["orchestrator"]["psi_C"] > 0

    health = client.get("/api/health")
    assert health.status_code == 200
    assert json.loads(health.data)["ledger_entries"] >= 1


def test_hardening_dry_run_records_commands() -> None:
    from sophia_local.security.hardening import SystemHardening

    sys_harden = SystemHardening(dry_run=True)
    applied = sys_harden.apply_hardening()
    assert applied
    assert any("iptables" in c for c in applied)
    assert any("systemctl" in c for c in applied)