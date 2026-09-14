"""Hardening local — aplica configurações de segurança documentáveis.

Suporta dois modos:
- `--apply`: executa comandos (Linux, requer root).
- `--dry-run`: apenas imprime o que seria feito (padrão, seguro).
Tudo é logado de forma auditável no hardening log.
"""
from __future__ import annotations

import logging
import os
import platform
import shlex
import subprocess
from dataclasses import dataclass, field
from typing import List

logger = logging.getLogger("sophia.hardening")


@dataclass
class HardeningRules:
    """Conjunto de regras de hardening por família (derivadas dos 19 invariantes)."""

    inactive_services: List[str] = field(default_factory=lambda: ["avahi-daemon", "bluetooth"])
    allowed_ports: List[int] = field(default_factory=lambda: [8080, 22])
    input_policy: str = "DROP"
    watch_paths: List[str] = field(default_factory=lambda: ["/opt/sophia"])  # auditctl -w


class SystemHardening:
    """Aplica hardening: firewall, serviços, permissões e auditoria."""

    def __init__(self, rules: HardeningRules | None = None, dry_run: bool = True) -> None:
        self.rules = rules or HardeningRules()
        self.dry_run = dry_run
        self.applied: List[str] = []

    def _run(self, cmd: str) -> bool:
        """Executa um comando de hardening (ou registra em dry-run)."""
        self.applied.append(cmd)
        logger.info("[%s] %s", "DRY-RUN" if self.dry_run else "APPLY", cmd)
        if self.dry_run:
            return True
        try:
            proc = subprocess.run(shlex.split(cmd), capture_output=True, text=True,
                                  timeout=30)
            if proc.returncode != 0:
                logger.warning("comando falhou (%s): %s", cmd, proc.stderr.strip())
            return proc.returncode == 0
        except (OSError, subprocess.TimeoutExpired) as exc:
            logger.error("falha ao executar %s: %s", cmd, exc)
            return False

    def apply_firewall(self) -> None:
        for port in self.rules.allowed_ports:
            self._run(f"sudo iptables -A INPUT -p tcp --dport {port} -j ACCEPT")
        self._run(f"sudo iptables -P INPUT {self.rules.input_policy}")

    def disable_services(self) -> None:
        for svc in self.rules.inactive_services:
            self._run(f"sudo systemctl disable {svc}")
            self._run(f"sudo systemctl stop {svc}")

    def audit_rules(self) -> None:
        for path in self.rules.watch_paths:
            self._run(f"sudo auditctl -w {path} -p wa -k sophia_changes")

    def apply_hardening(self) -> List[str]:
        self.disable_services()
        self.apply_firewall()
        self.audit_rules()
        return self.applied


def run_cli() -> None:
    import argparse
    parser = argparse.ArgumentParser(prog="sophia_hardening")
    parser.add_argument("--apply", action="store_true", help="aplica de fato (requer root)")
    args = parser.parse_args()
    logging.basicConfig(level=logging.INFO, format="%(message)s")
    print(f"Plataforma: {platform.system()}")
    sys = SystemHardening(dry_run=not args.apply)
    for cmd in sys.apply_hardening():
        print(cmd)


if __name__ == "__main__":
    run_cli()