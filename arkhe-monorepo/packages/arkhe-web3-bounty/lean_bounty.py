#!/usr/bin/env python3
"""
ARKHE WEB3 BUG BOUNTY — lean_bounty.py  v6.0
Web3 security-finding orchestrator that feeds `bug_bounty_web3.lean`
(now a Mathlib-backed lake package, v6.0).

Pipeline:
    EXPLORERS (slither / aderyn / mythril / oyente)  ->  Findings
        -> dedupe_and_rank (severity ordering)
        -> findings_to_lean (emits Arkhe-Web3Bounty-compatible Lean)
        -> spec_erc20.json (ERC-20-NoInflation invariants)

Stdlib-only. Python 3.10+ (tested on 3.14). External explorers are optional;
missing binaries are skipped gracefully and never crash the run.
"""

from __future__ import annotations

import json
import re
import shutil
import subprocess
import sys
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Callable, Iterable, Optional

# ---------------------------------------------------------------------------
# Severity ranking and rule vocabulary
# ---------------------------------------------------------------------------

SEVERITY: dict[str, int] = {
    "critical": 9,
    "high": 8,
    "medium": 5,
    "low": 2,
    "info": 1,
}

# Maps explorer finding rule-ids onto the Lean Vulnerability enum.
VULN_ALIASES: dict[str, str] = {
    "reentrancy": "reentrancy",
    "reentrancy-eth": "reentrancy",
    "reentrancy-eth/into-mapping": "reentrancy",
    "reentrancy-eth/into-modifier": "reentrancy",
    "arbitrary-send-eth": "uncheckedCall",
    "unchecked-calls": "uncheckedCall",
    "unchecked-low-level": "uncheckedCall",
    "unchecked-transfer": "uncheckedCall",
    "uninitialized-storage": "uncheckedCall",
    "arithmetic": "overflow",
    "artifactual-overflow": "overflow",
    "overflow": "overflow",
    "divide-before-multiply": "overflow",
    "unused-return": "uncheckedCall",
    "tx-origin": "txOrigin",
    "selfdestruct": "selfDestruct",
    "suicidal": "selfDestruct",
}

DETECTORS: list[str] = ["reentrancy", "overflow", "unchecked-call"]

# ---------------------------------------------------------------------------
# Explorer drivers.  `{target}` is replaced with the audited path.
# Each entry: (description, argv template, finding parser, output filter)
# ---------------------------------------------------------------------------

EXPLORERS: dict[str, dict[str, Any]] = {
    "slither": {
        "cmd": ["slither", "{target}", "--json", "-"],
        "parser": "parse_slither_json",
    },
    "aderyn": {
        "cmd": ["aderyn", "-s", "{target}", "-o", "{outdir}"],
        "parser": "parse_aderyn_report",
    },
    "mythril": {
        "cmd": ["mythril", "analyze", "{target}"],
        "parser": "parse_mythril_text",
    },
    "oyente": {
        "cmd": ["oyente", "-s", "{target}"],
        "parser": "parse_oyente_text",
    },
}

# ---------------------------------------------------------------------------
# Data model
# ---------------------------------------------------------------------------


@dataclass
class Finding:
    rule_id: str
    severity: str
    target: str
    line: Optional[int] = None
    description: str = ""
    evidence: str = ""
    source: str = "static"

    @property
    def vulnerability(self) -> str:
        return VULN_ALIASES.get(self.rule_id, "uncheckedCall")

    @property
    def score(self) -> int:
        return SEVERITY.get(self.severity, 0)

    def as_lean(self) -> str:
        """Render one finding as a `ReportEntry` term (matches bug_bounty_web3.lean)."""
        vuln = self.vulnerability
        line = self.line if self.line is not None else 0
        target = json.dumps(self.target, ensure_ascii=False)
        return (
            f"ReportEntry.mk Vulnerability.{vuln} {target} {line}"
        )

    def to_dict(self) -> dict[str, Any]:
        d: dict[str, Any] = {
            "rule_id": self.rule_id,
            "severity": self.severity,
            "target": self.target,
            "source": self.source,
        }
        if self.line is not None:
            d["line"] = self.line
        if self.description:
            d["description"] = self.description
        if self.evidence:
            d["evidence"] = self.evidence
        return d


# ---------------------------------------------------------------------------
# Explorer finding parsers (line/JSON -> Finding)
# ---------------------------------------------------------------------------


def parse_slither_json(raw: str, target: str) -> list[Finding]:
    findings: list[Finding] = []
    try:
        data = json.loads(raw)
    except json.JSONDecodeError:
        return findings
    for res in data.get("results", {}).get("detectors", []):
        rule = res.get("check", res.get("id", "unknown"))
        sev = res.get("impact", "info").lower()
        if sev == "informational":
            sev = "info"
        desc = res.get("description", "")
        for el in res.get("elements", []) or [{}]:
            src = el.get("source_mapping", {}) or {}
            findings.append(
                Finding(
                    rule_id=rule,
                    severity=sev,
                    target=el.get("filename", target),
                    line=src.get("lines")[0] if src.get("lines") else None,
                    description=desc[:400],
                    evidence=el.get("type", ""),
                )
            )
    return findings


def parse_aderyn_report(root: Path, target: str) -> list[Finding]:
    report = root / "report.json"
    findings: list[Finding] = []
    if not report.exists():
        return findings
    try:
        data = json.loads(report.read_text(encoding="utf-8"))
    except (json.JSONDecodeError, OSError):
        return findings
    issues = data.get("issues", data.get("findings", []))
    if not isinstance(issues, list):
        return findings
    for it in issues:
        findings.append(
            Finding(
                rule_id=it.get("title", it.get("issue", "aderyn-unknown")),
                severity=str(it.get("severity", "info")).lower(),
                target=it.get("file", target),
                line=it.get("line_number"),
                description=str(it.get("description", ""))[:400],
                evidence=str(it.get("snippet", ""))[:200],
            )
        )
    return findings


_LINE_MATCHERS: list[tuple[str, str]] = [
    ("reentrancy", "reentrancy"),
    ("integer overflow", "overflow"),
    ("arithmetic overflow", "overflow"),
    ("unchecked call", "uncheckedCall"),
    ("unchecked return", "uncheckedCall"),
    ("tx.origin", "txOrigin"),
    ("selfdestruct", "selfDestruct"),
]


def parse_plain_text(raw: str, target: str) -> list[Finding]:
    findings: list[Finding] = []
    for lineno, line in enumerate(raw.splitlines(), start=1):
        low = line.lower()
        for needle, rule in _LINE_MATCHERS:
            if needle in low:
                findings.append(
                    Finding(
                        rule_id=rule,
                        severity="medium",
                        target=target,
                        line=lineno,
                        description=line.strip()[:400],
                    )
                )
                break
    return findings


def parse_mythril_text(raw: str, target: str) -> list[Finding]:
    return parse_plain_text(raw, target)


def parse_oyente_text(raw: str, target: str) -> list[Finding]:
    return parse_plain_text(raw, target)


# ---------------------------------------------------------------------------
# Orchestrator
# ---------------------------------------------------------------------------


class BoundaryOrchestrator:
    """Runs the configured explorers over a repo or a single file."""

    def __init__(
        self,
        explorers: Optional[Iterable[str]] = None,
        max_workers: int = 1,
        timeout: int = 900,
    ) -> None:
        names = list(explorers) if explorers else list(EXPLORERS)
        self.explorers = names
        self.timeout = timeout

    # -- running ---------------------------------------------------------

    def _run_one(self, name: str, target: Path) -> list[Finding]:
        spec = EXPLORERS[name]
        argv = [a.format(target=str(target), outdir=str(target)) for a in spec["cmd"]]
        binary = argv[0]
        if shutil.which(binary) is None:
            print(f"[skip] {name}: '{binary}' not on PATH", file=sys.stderr)
            return []
        parser = globals().get(spec["parser"])
        try:
            proc = subprocess.run(
                argv,
                capture_output=True,
                text=True,
                timeout=self.timeout,
                check=False,
            )
        except (OSError, subprocess.TimeoutExpired) as exc:
            print(f"[err ] {name}: {exc}", file=sys.stderr)
            return []
        raw = proc.stdout or ""
        if spec["parser"] == "parse_aderyn_report":
            findings = parser(target, str(target))
        else:
            findings = parser(raw, str(target)) if parser else []
        for f in findings:
            f.source = name
        return findings

    def run_on_target(self, target: Path) -> list[Finding]:
        if not target.exists():
            raise FileNotFoundError(target)
        all_findings: list[Finding] = []
        for name in self.explorers:
            all_findings.extend(self._run_one(name, target))
        return dedupe_and_rank(all_findings)

    def run_on_repo(self, repo: Path, solidity_only: bool = True) -> list[Finding]:
        if repo.is_file():
            return self.run_on_target(repo)
        sources = list(repo.rglob("*.sol")) if solidity_only else list(repo.rglob("*"))
        all_findings: list[Finding] = []
        for src in sources:
            all_findings.extend(self.run_on_target(src))
        return dedupe_and_rank(all_findings)


# ---------------------------------------------------------------------------
# Dedup / rank / emit
# ---------------------------------------------------------------------------


def dedupe_and_rank(findings: Iterable[Finding]) -> list[Finding]:
    seen: dict[tuple[str, str, Optional[int], str], Finding] = {}
    for f in findings:
        key = (f.rule_id, f.target, f.line, f.description)
        prev = seen.get(key)
        if prev is None or f.score > prev.score:
            seen[key] = f
    ranked = sorted(seen.values(), key=lambda f: (-f.score, f.rule_id, f.target))
    return ranked


def md_escape(text: str) -> str:
    return re.sub(r"([\\`*_\[\]{}()#+\-.!])", r"\\\1", text)


def findings_to_lean(findings: Iterable[Finding], spec_id: str) -> str:
    """Emit a compilable Lean snippet over `bug_bounty_web3.lean` types."""
    fs = list(findings)
    rows: list[str] = []
    for k, f in enumerate(fs):
        comma = "," if k < len(fs) - 1 else ""
        rows.append(f"  {f.as_lean()}{comma} -- {f.rule_id}: {f.severity}")
    body = "\n".join(rows) if rows else "  -- no findings (all checks passed)"
    return (
        "import bug_bounty_web3\n"
        "\n"
        "namespace Arkhe.Web3Bounty.Report\n"
        "\n"
        f"def reportSpecId : String := {json.dumps(spec_id)}\n"
        "\n"
        "def reportedVulnerabilities : List ReportEntry :=\n"
        "  [\n"
        f"{body}\n"
        "  ]\n"
        "\n"
        f"def findingCount : Nat := {len(fs)}\n"
        "end Arkhe.Web3Bounty.Report\n"
    )


# ---------------------------------------------------------------------------
# Spec loader
# ---------------------------------------------------------------------------

DEFAULT_SECURITY_SPECS: dict[str, dict[str, Any]] = {
    "ERC-20-NoInflation": {
        "spec_id": "ERC-20-NoInflation",
        "name": "ERC-20 No-Inflation",
        "checks": [
            "mint-ownership-guard",
            "total-supply-consistency",
            "no-reentrancy",
            "no-overflow-transfer",
            "no-unchecked-call",
        ],
    }
}


def load_spec(spec_path: Optional[Path]) -> dict[str, Any]:
    if spec_path is not None and spec_path.exists():
        return json.loads(spec_path.read_text(encoding="utf-8"))
    return dict(DEFAULT_SECURITY_SPECS["ERC-20-NoInflation"])


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------


def main(argv: Optional[list[str]] = None) -> int:
    import argparse

    ap = argparse.ArgumentParser(
        prog="lean_bounty",
        description="Arkhe Web3 Bug Bounty orchestrator -> Lean findings export.",
    )
    ap.add_argument("target", nargs="?", default=".", help="repo dir or .sol file")
    ap.add_argument(
        "--explorers",
        nargs="*",
        default=None,
        help="subset of: " + ", ".join(EXPLORERS),
    )
    ap.add_argument("--spec", type=Path, default=None, help="path to spec JSON")
    ap.add_argument(
        "--outdir",
        type=Path,
        default=Path("reports"),
        help="output directory for findings",
    )
    ap.add_argument(
        "--timeout", type=int, default=900, help="per-explorer timeout (s)"
    )
    args = ap.parse_args(argv)

    spec = load_spec(args.spec)
    spec_id = spec.get("spec_id", "ERC-20-NoInflation")
    target = Path(args.target)

    orch = BoundaryOrchestrator(explorers=args.explorers, timeout=args.timeout)
    try:
        findings = orch.run_on_repo(target)
    except FileNotFoundError as exc:
        print(f"error: target not found: {exc}", file=sys.stderr)
        return 2

    outdir = args.outdir
    outdir.mkdir(parents=True, exist_ok=True)

    # 1. JSON report
    json_path = outdir / "lean_bounty_findings.json"
    json_path.write_text(
        json.dumps(
            {
                "spec_id": spec_id,
                "finding_count": len(findings),
                "findings": [f.to_dict() for f in findings],
            },
            indent=2,
            ensure_ascii=False,
        ),
        encoding="utf-8",
    )

    # 2. Lean snippet
    lean_path = outdir / "lean_bounty_findings.lean"
    lean_path.write_text(findings_to_lean(findings, spec_id), encoding="utf-8")

    # 3. Spec copy (mirrors the repo's spec_erc20.json)
    if args.spec is not None and args.spec.exists():
        (outdir / args.spec.name).write_text(
            args.spec.read_text(encoding="utf-8"), encoding="utf-8"
        )

    # 4. Console summary
    print(f"\n{'-' * 64}")
    print(f"spec      : {spec_id}")
    print(f"target    : {target}")
    print(f"explorers : {', '.join(orch.explorers)}")
    print(f"findings  : {len(findings)}")
    for f in findings[:20]:
        loc = f"L{f.line}" if f.line is not None else "??"
        print(f"  [{f.severity:>8}] {f.rule_id:36s} {f.target}:{loc}")
    if len(findings) > 20:
        print(f"  ... and {len(findings) - 20} more")
    print(f"JSON      : {json_path}")
    print(f"LEAN      : {lean_path}")
    return 0 if not findings else 1


if __name__ == "__main__":
    sys.exit(main())
