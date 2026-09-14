# catedral_os_v280/tests/test_integration.py
"""Teste de integracao: orquestrador + BLOCO 816 (v280.0)."""

import hashlib
import json

from orchestrator import CatedralOS


def test_orchestrator_run_and_block(tmp_path):
    osl = CatedralOS(num_tunnels=3, phi_star=0.85, seed=42,
                     ledger_path=str(tmp_path / "ledger.json"))
    summary = osl.run(steps=30)
    assert summary["num_tunnels"] == 3
    assert summary["total_actions"] == 90
    assert summary["ledger"]["chain_ok"] is True
    assert summary["ledger"]["entries"] == 90
    assert summary["ledger"]["mode"] in ("ipfs", "local")
    # nenhum invariante violado
    for statuses in summary["invariants"].values():
        assert "violated" not in statuses

    seal = osl.register_block(str(tmp_path / "bloco_0816.json"))
    block_path = tmp_path / "bloco_0816.json"
    with open(block_path, "r", encoding="utf-8") as f:
        record = json.load(f)

    assert record["versao"] == "v280.0"
    assert record["bloco"] == "816"
    assert record["handover_anterior"] == 815
    assert record["seal"] == seal
    pre = json.dumps({k: v for k, v in record.items() if k != "seal"},
                     sort_keys=True, ensure_ascii=False)
    assert hashlib.sha256(pre.encode()).hexdigest() == record["seal"]
    assert record["metricas"]["ledger"]["chain_ok"] is True


def test_orchestrator_stress(tmp_path):
    osl = CatedralOS(num_tunnels=2, seed=7,
                     ledger_path=str(tmp_path / "ledger.json"))
    osl.run(steps=10)
    report = osl.run_stress(duration=0, faults=True)
    # calls_per_worker não passado: duration=0 encerra na primeira checagem;
    # garantimos apenas a forma do relatório.
    assert "overall_success_rate" in report
    assert report["total_workers"] == 2


def test_orchestrator_dashboard_mirror(tmp_path):
    from dashboard.dash_app import get_metrics

    osl = CatedralOS(num_tunnels=2, seed=3,
                     ledger_path=str(tmp_path / "ledger.json"))
    osl.run(steps=5)
    m = get_metrics()
    assert m["phi_total"] is not None
    assert isinstance(m["chain_ok"], bool)