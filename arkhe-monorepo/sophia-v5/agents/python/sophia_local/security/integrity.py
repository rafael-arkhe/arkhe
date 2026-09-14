"""Monitor de integridade (baseline SHA-256 + detecção de mudanças).

Gera baseline de hashes dos arquivos críticos, verifica em loop contínuo
e grava alertas em log append-only. Sem dependências externas.
"""
from __future__ import annotations

import hashlib
import json
import os
import threading
import time
from dataclasses import dataclass, field
from pathlib import Path
from typing import Dict, List, Optional


@dataclass
class IntegrityMonitor:
    """Monitora integridade de um diretório/arquivos críticos."""

    watch_dir: Path = field(default_factory=lambda: Path("."))
    baseline_path: Path = field(default_factory=lambda: Path("file_hashes.json"))
    critical_files: List[Path] = field(default_factory=list)
    alerts: List[str] = field(default_factory=list)
    interval_s: int = 60

    def __post_init__(self) -> None:
        self.running = False
        self._lock = threading.Lock()

    @staticmethod
    def _sha256(path: Path) -> str:
        h = hashlib.sha256()
        with open(path, "rb") as fh:
            for chunk in iter(lambda: fh.read(65536), b""):
                h.update(chunk)
        return h.hexdigest()

    def _files(self) -> List[Path]:
        files: List[Path] = [Path(f) for f in self.critical_files if Path(f).is_file()]
        root = Path(self.watch_dir)
        if root.is_dir():
            for p in sorted(root.rglob("*")):
                if p.is_file():
                    files.append(p)
        return files

    def build_baseline(self) -> Dict[str, str]:
        baseline = {str(f): self._sha256(f) for f in self._files()}
        self.baseline_path.parent.mkdir(parents=True, exist_ok=True)
        self.baseline_path.write_text(json.dumps(baseline, indent=2, sort_keys=True),
                                      encoding="utf-8")
        return baseline

    def check_integrity(self) -> List[str]:
        new_alerts: List[str] = []
        try:
            baseline = json.loads(self.baseline_path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            baseline = self.build_baseline()

        current = {str(f): self._sha256(f) for f in self._files()}
        for path, h in current.items():
            if path not in baseline:
                new_alerts.append(f"Novo arquivo detectado: {path}")
            elif baseline[path] != h:
                new_alerts.append(f"Arquivo modificado: {path} (hash alterado)")
        with self._lock:
            self.alerts.extend(new_alerts)
        return new_alerts

    def run_continuous(self, interval_s: Optional[int] = None) -> None:
        self.running = True

        def monitor_loop() -> None:
            while self.running:
                self.check_integrity()
                time.sleep(interval_s or self.interval_s)

        threading.Thread(target=monitor_loop, daemon=True).start()

    def stop(self) -> None:
        self.running = False

    def recent_alerts(self, n: int = 10) -> List[str]:
        with self._lock:
            return self.alerts[-n:]


def run_cli() -> None:
    import argparse
    import logging
    from pathlib import Path as _P

    parser = argparse.ArgumentParser(prog="sophia_integrity")
    parser.add_argument("--daemon", action="store_true", help="roda continuamente")
    parser.add_argument("--build-baseline", action="store_true")
    parser.add_argument("--check", action="store_true")
    parser.add_argument(
        "--state-dir", default=".", help="onde gravar baseline (" +
        str(_P("file_hashes.json")) + ")"
    )
    args = parser.parse_args()
    logging.basicConfig(level=logging.INFO, format="[sophia_integrity] %(message)s")

    state = _P(args.state_dir)
    state.mkdir(parents=True, exist_ok=True)
    mon = IntegrityMonitor(
        watch_dir=_P("."),
        baseline_path=state / "file_hashes.json",
        interval_s=60,
    )

    if args.build_baseline:
        n = len(mon.build_baseline())
        print(f"baseline gerado: {n} arquivos em {mon.baseline_path}")
        return
    if args.check:
        for alert in mon.check_integrity():
            logging.info("%s", alert)
        print(f"{len(mon.alerts)} alertas")
        return
    if args.daemon:
        mon.build_baseline()
        mon.run_continuous()
        import time
        while True:
            time.sleep(1)