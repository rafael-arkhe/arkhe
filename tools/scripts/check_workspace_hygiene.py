#!/usr/bin/env python3
"""Workspace hygiene guard (bloco 1078, corrigendum 3).

Verifica, contra a ARVORE COMMITADA (git ls-tree, nao o disco), que o conjunto
de membros declarados em arkhe-monorepo/Cargo.toml e inexistentes no checkout e
IDENTICO ao contrato de divida em .github/workspace-hygiene-expected.txt.

Resultado:
  - missing == expected : PASS verde (divida conhecida e visivel, nao regressao)
  - missing != expected : FAIL vermelho com set-diff especifico
  - ambos vazios        : smoke test extra (cargo metadata --no-deps)

Uso (na raiz do repo): python3 tools/scripts/check_workspace_hygiene.py [--expected ARQ]
"""
from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path
from tomllib import load

ROOT = Path(__file__).resolve().parents[2]
MANIFEST = "arkhe-monorepo/Cargo.toml"
EXPECTED = ".github/workspace-hygiene-expected.txt"


def declared_members(manifest: Path) -> set[str]:
    with manifest.open("rb") as fh:
        ws = load(fh)["workspace"]
    return {str(path) for path in ws.get("members", []) if path.startswith(("packages/", "services/"))}


def committed_member_dirs(root: Path) -> set[str]:
    out = subprocess.run(
        ["git", "-C", str(root), "ls-tree", "-r", "--name-only", "HEAD"],
        capture_output=True,
        text=True,
        check=True,
    ).stdout
    dirs = set()
    for line in out.splitlines():
        parts = line.split("/")
        if len(parts) >= 3 and parts[0] == "arkhe-monorepo":
            dirs.add(f"{parts[1]}/{parts[2]}")
    return dirs


def expected_missing(path: Path) -> set[str]:
    lines = [ln.strip() for ln in path.read_text(encoding="utf-8").splitlines()]
    return {ln for ln in lines if ln and not ln.startswith("#")}


def cargo_metadata_smoke(root: Path) -> bool:
    return subprocess.run(
        ["cargo", "metadata", "--no-deps", "--format-version", "1"],
        cwd=root / "arkhe-monorepo",
        capture_output=True,
        text=True,
    ).returncode == 0


def main() -> int:
    args = argparse.ArgumentParser()
    args.add_argument("--expected", default=EXPECTED)
    members = declared_members(ROOT / MANIFEST)
    committed = committed_member_dirs(ROOT)
    missing = {m for m in members if m not in committed}
    expected = expected_missing(ROOT / args.parse_args().expected)

    if missing == expected:
        if missing:
            print(f"ok: divida conhecida de {len(missing)} membros (contrato satisfeito).")
            print("    retire as linhas de .github/workspace-hygiene-expected.txt ao commitar.")
        else:
            if not cargo_metadata_smoke(ROOT):
                print("::error::cargo metadata falhou em checkout limpo com workspace completo.")
                return 1
            print("ok: workspace completo commitado e resoluto (cargo metadata ok).")
        return 0

    unexpected = missing - expected
    announced_but_present = expected - missing
    if unexpected:
        print("::error::workspace-hygiene REGRESSAO: membros declarados ausentes da arvore e fora do contrato:")
        for m in sorted(unexpected):
            print(f"  - {m}  (adicionar a .github/workspace-hygiene-expected.txt ou commitar)")
    if announced_but_present:
        print("::error::contrato desatualizado: membros do ficheiro ja estao commitados:")
        for m in sorted(announced_but_present):
            print(f"  - {m}  (remover de .github/workspace-hygiene-expected.txt)")
    print(f"missing={len(missing)} expected={len(expected)}")
    return 1


if __name__ == "__main__":
    sys.exit(main())