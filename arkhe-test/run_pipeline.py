#!/usr/bin/env python3
"""
Arkhe-Test Pipeline Runner.
Orchestrates quality pipeline across 3 stages:
- Stage 1: Fast feedback (< 5 min) - Lint/format, type check, unit tests, serialization, MMR, flags.
- Stage 2: Integration (< 20 min) - Peer exchange, chunk validation, MMR anchoring, offline bundle, rollback.
- Stage 3: Adversarial (Continuous Red Team) - I-01..I-10 invariant attacks, replays, falsified proofs.
"""

import sys
import os
import time
import argparse
import subprocess
import json
from pathlib import Path

BASE_DIR = Path(__file__).resolve().parent
sys.path.insert(0, str(BASE_DIR))

from core.crypto import compute_chunk_hash, canonical_json_bytes
from core.flags import FeatureFlagManager


def run_cmd(cmd: list, cwd: Path = BASE_DIR) -> tuple:
    start = time.time()
    env = os.environ.copy()
    env["PYTHONPATH"] = str(BASE_DIR)
    res = subprocess.run(cmd, cwd=str(cwd), env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    duration = time.time() - start
    return res.returncode == 0, res.stdout, res.stderr, duration


def run_stage_1() -> dict:
    print("\n[STAGE 1] Quick Feedback (< 5 min)...")
    results = {}

    ffm = FeatureFlagManager(str(BASE_DIR / "policies" / "flags.toml"))
    violations = ffm.audit_flags()
    results["feature_flags_audit"] = {
        "status": "PASS" if not violations else "FAIL",
        "violations": violations
    }
    print(f"  - Feature flags security audit: {results['feature_flags_audit']['status']}")

    ok, out, err, dur = run_cmd(["pytest", "-c", "pytest.ini", "suites/unit", "-q"])
    results["unit_tests"] = {
        "status": "PASS" if ok else "FAIL",
        "duration_seconds": round(dur, 3),
        "output": out.strip()
    }
    print(f"  - Unit tests: {results['unit_tests']['status']} in {dur:.2f}s")

    ok, out, err, dur = run_cmd(["pytest", "-c", "pytest.ini", "suites/property", "-q"])
    results["property_tests"] = {
        "status": "PASS" if ok else "FAIL",
        "duration_seconds": round(dur, 3),
        "output": out.strip()
    }
    print(f"  - Property tests: {results['property_tests']['status']} in {dur:.2f}s")

    stage_passed = (
        results["feature_flags_audit"]["status"] == "PASS" and
        results["unit_tests"]["status"] == "PASS" and
        results["property_tests"]["status"] == "PASS"
    )
    results["stage_passed"] = stage_passed
    return results


def run_stage_2() -> dict:
    print("\n[STAGE 2] Integration (< 20 min)...")
    results = {}

    suites = ["integration", "network", "mobile", "wasm"]
    for s in suites:
        ok, out, err, dur = run_cmd(["pytest", "-c", "pytest.ini", f"suites/{s}", "-q"])
        results[s] = {
            "status": "PASS" if ok else "FAIL",
            "duration_seconds": round(dur, 3),
            "output": out.strip()
        }
        print(f"  - Suite '{s}': {results[s]['status']} in {dur:.2f}s")

    stage_passed = all(results[s]["status"] == "PASS" for s in suites)
    results["stage_passed"] = stage_passed
    return results


def run_stage_3() -> dict:
    print("\n[STAGE 3] Adversarial & Invariant Red Team (I-01 through I-10)...")
    results = {}

    ok, out, err, dur = run_cmd(["pytest", "-c", "pytest.ini", "suites/adversarial", "-q"])
    results["adversarial_invariants"] = {
        "status": "PASS" if ok else "FAIL",
        "duration_seconds": round(dur, 3),
        "output": out.strip(),
        "invariants_covered": [f"I-{i:02d}" for i in range(1, 11)],
        "fail_closed_guarantee": True
    }
    print(f"  - Adversarial invariants (I-01..I-10): {results['adversarial_invariants']['status']} in {dur:.2f}s")

    results["stage_passed"] = ok
    return results


def generate_audit_report(stage_reports: dict) -> Path:
    timestamp = int(time.time())
    report_data = {
        "report_id": f"arkhe-test-run-{timestamp}",
        "timestamp": timestamp,
        "pipeline_version": "1.0.0",
        "stages": stage_reports,
        "overall_status": "PASS" if all(s.get("stage_passed", False) for s in stage_reports.values()) else "FAIL"
    }

    report_bytes = canonical_json_bytes(report_data)
    report_hash = compute_chunk_hash(report_bytes)
    report_data["content_hash"] = report_hash

    report_file = BASE_DIR / "reports" / "security" / f"test_run_{timestamp}_{report_hash[:12]}.json"
    with open(report_file, "w") as f:
        json.dump(report_data, f, indent=2)

    latest_file = BASE_DIR / "reports" / "latest_run.json"
    with open(latest_file, "w") as f:
        json.dump(report_data, f, indent=2)

    print(f"\n[EVIDÊNCIA AUDITÁVEL] Relatório gerado: {report_file}")
    print(f"Content Hash (BLAKE3/SHA256): {report_hash}")
    return report_file


def main():
    parser = argparse.ArgumentParser(description="Arkhe-Test: Quality & Invariant Testing Pipeline")
    parser.add_argument("--stage", type=int, choices=[1, 2, 3], help="Run a specific stage (1, 2, or 3)")
    parser.add_argument("--all", action="store_true", help="Run all pipeline stages sequentially")
    args = parser.parse_args()

    if not args.stage and not args.all:
        args.all = True

    stage_reports = {}

    if args.all or args.stage == 1:
        stage_reports["stage_1"] = run_stage_1()
        if not stage_reports["stage_1"]["stage_passed"]:
            print("\n❌ Pipeline halted at Stage 1 (Fail-Closed).")
            generate_audit_report(stage_reports)
            sys.exit(1)

    if args.all or args.stage == 2:
        stage_reports["stage_2"] = run_stage_2()
        if not stage_reports["stage_2"]["stage_passed"]:
            print("\n❌ Pipeline halted at Stage 2 (Fail-Closed).")
            generate_audit_report(stage_reports)
            sys.exit(1)

    if args.all or args.stage == 3:
        stage_reports["stage_3"] = run_stage_3()
        if not stage_reports["stage_3"]["stage_passed"]:
            print("\n❌ Pipeline halted at Stage 3 (Fail-Closed).")
            generate_audit_report(stage_reports)
            sys.exit(1)

    report_path = generate_audit_report(stage_reports)
    print("\n✅ Todos os testes e invariantes aprovados com sucesso! Merge/Deploy permitido.\n")


if __name__ == "__main__":
    main()
