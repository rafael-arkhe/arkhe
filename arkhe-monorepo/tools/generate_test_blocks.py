#!/usr/bin/env python3
"""generate_test_blocks.py — Gera blocos de teste para verificacao do ledger.

Contrato canonico v582.0 (bloco 1075): o hash e uma **hex string de 64
caracteres**, identica nos tres artefactos (Rust/Python/PowerShell).

Algoritmo: SHA3-256 (Ghost-1, sem dependencias) — espelha
`compute_block_hash` do crate `arkhe-block-registry`:

    SHA3-256(numero_le4 || tipo_utf8 || parent_fromhex?)

Compativel com Python 3.9+ (usa `Optional` em vez de `|`).
Corrige o bug da auditoria estatica: `scenario_collision` usava `_b` como
suffix para nao sobrescrever o ficheiro do primeiro bloco (antes, o segundo
escrito sobrepunha o primeiro e a colisao sumia).
"""

import argparse
import hashlib
import json
from pathlib import Path
from typing import Optional


def compute_hash(numero: int, tipo: str, parent: Optional[str] = None) -> str:
    """Hash canonico do bloco — SHA3-256, mesmo layout de bytes do Rust."""
    h = hashlib.sha3_256()
    h.update(numero.to_bytes(4, "little"))
    h.update(tipo.encode("utf-8"))
    if parent:
        h.update(bytes.fromhex(parent))
    return h.hexdigest()


def write_block(
    d: Path,
    numero: int,
    tipo: str,
    parent: Optional[str] = None,
    suffix: str = "",
) -> str:
    """Escreve um bloco. O suffix evita sobrescrita em cenarios de colisao."""
    h = compute_hash(numero, tipo, parent)
    payload = {"numero": numero, "tipo": tipo, "hash": h}
    if parent:
        payload["parent_hash"] = parent
    filename = f"bloco_{numero:04d}{suffix}.json"
    (d / filename).write_text(json.dumps(payload, indent=2), encoding="utf-8")
    return h


def scenario_valid(base: Path) -> None:
    """Cadeia valida de 3 blocos encadeados."""
    d = base / "valid"
    d.mkdir(parents=True, exist_ok=True)
    h1 = write_block(d, 1, "ROOT")
    h2 = write_block(d, 2, "CHILD", parent=h1)
    write_block(d, 3, "GRANDCHILD", parent=h2)
    print(f"OK valid: 3 blocos encadeados em {d}")


def scenario_collision(base: Path) -> None:
    """Colisao de numero: dois blocos com numero 1, tipos diferentes.

    BUG CORRIGIDO: `suffix="_b"` — sem isto o segundo ficheiro sobrescrevia o
    primeiro e a colisao nao era detetada por nenhum verificador.
    """
    d = base / "collision"
    d.mkdir(parents=True, exist_ok=True)
    write_block(d, 1, "ROOT")
    write_block(d, 1, "AUDITORIA", suffix="_b")
    print(f"OK collision: 2 blocos com numero 1 em {d}")


def scenario_orphan(base: Path) -> None:
    """Bloco com parent inexistente (hash de zeros)."""
    d = base / "orphan"
    d.mkdir(parents=True, exist_ok=True)
    write_block(d, 1, "ROOT")
    fake_parent = "00" * 32
    write_block(d, 2, "ORPHAN", parent=fake_parent)
    print(f"OK orphan: parent inexistente em {d}")


def scenario_invalid(base: Path) -> None:
    """JSON malformado (o verificador deve falhar no parse)."""
    d = base / "invalid"
    d.mkdir(parents=True, exist_ok=True)
    (d / "bloco_0001.json").write_text("{invalid json", encoding="utf-8")
    print(f"OK invalid: JSON malformado em {d}")


def main() -> None:
    parser = argparse.ArgumentParser(description="Gera blocos de teste")
    parser.add_argument("--base", type=Path, default=Path("./test_blocks"))
    args = parser.parse_args()

    args.base.mkdir(parents=True, exist_ok=True)
    scenario_valid(args.base)
    scenario_collision(args.base)
    scenario_orphan(args.base)
    scenario_invalid(args.base)

    print()
    print(f"Blocos gerados em {args.base.resolve()}")
    print()
    print("Comandos de teste (expectativas):")
    print(f"  pwsh tools/verify-blocks.ps1 -BlocksDir {args.base}/valid      # PASS (exit 0)")
    print(f"  pwsh tools/verify-blocks.ps1 -BlocksDir {args.base}/collision  # FAIL (exit 1)")
    print(f"  pwsh tools/verify-blocks.ps1 -BlocksDir {args.base}/orphan     # FAIL (exit 1)")
    print(f"  pwsh tools/verify-blocks.ps1 -BlocksDir {args.base}/invalid    # FAIL (exit 1)")
    print(f"  cargo run -p arkhe-block-registry --bin verify -- {args.base}/valid")


if __name__ == "__main__":
    main()