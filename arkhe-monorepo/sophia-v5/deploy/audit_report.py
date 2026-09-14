#!/usr/bin/env python3
"""Gera relatório de auditoria da Sophia V5.0 (loopseal-3: trilha completa).

Coleta: ledger TemporalChain, integridade, hardening aplicado, portas e
serviços ativos. Saída JSON em /var/log/sophia_audit.log.
"""
from __future__ import annotations

import json
import shutil
import subprocess
from datetime import datetime, timezone
from pathlib import Path


def _run(cmd: list[str]) -> str:
    try:
        proc = subprocess.run(cmd, capture_output=True, text=True, timeout=15)
        return proc.stdout.strip()
    except (OSError, subprocess.TimeoutExpired):
        return ""


def audit_report(ledger: Path, output: Path) -> dict:
    from sophia_local.orchestrator import LocalOrchestrator

    orch = LocalOrchestrator(ledger_path=ledger)
    ledger_ok, ledger_count = orch.bridge.verify()

    report = {
        "timestamp": datetime.now(timezone.utc).isoformat(),
        "temporal_chain": {
            "verified": ledger_ok,
            "blocks": ledger_count,
            "last_seal": orch.bridge._last_hash(),
        },
        "system": {
            "platform": _run(["uname", "-a"]),
            "listening_ports": [
                ln for ln in _run(["ss", "-tulpn"]).splitlines()
                if any(p in ln for p in ("8080", "443", "22"))
            ],
        },
        "tools": {
            "wazuh": bool(shutil.which("wazuh-manager") or shutil.which("ossec-control")),
            "auditd": bool(shutil.which("auditctl")),
        },
    }

    out_dir = Path(output).parent
    out_dir.mkdir(parents=True, exist_ok=True)
    body = json.dumps(report, indent=2, sort_keys=True)
    Path(output).write_text(body, encoding="utf-8") if output else None
    print(body)
    return report


if __name__ == "__main__":
    import argparse
    import sys

    parser = argparse.ArgumentParser(prog="sophia_audit")
    parser.add_argument("--ledger", default="/var/lib/sophia/ledger.jsonl")
    parser.add_argument("--output", default="")
    args = parser.parse_args()

    if args.output:
        out = Path(args.output)
    else:
        out = Path(sys.stdout.name if hasattr(sys.stdout, "name") else "/dev/null")
    audit_report(Path(args.ledger), out)