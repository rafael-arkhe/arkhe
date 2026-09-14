"""Dashboard local (porta 8080) — Flask + Socket.IO (opcional).

Exibe métricas dos agentes, detecções de edge vision e alertas de
segurança em tempo real, tudo lido localmente.
"""
from __future__ import annotations

import json
import threading
import time
import webbrowser
from pathlib import Path
from typing import Any, Dict, Optional

import yaml
from flask import Flask, jsonify, render_template

from sophia_local.orchestrator import LocalOrchestrator
from sophia_local.edge_vision import EdgeVisionClient
from sophia_local.security.integrity import IntegrityMonitor

app = Flask(__name__)

# --- Estado em runtime (configurável por env) ---
BASE_DIR = Path(__file__).resolve().parent.parent
CONFIG_PATH = Path(__file__).resolve().parent.parent.parent / "config" / "sophia.yaml"
CONFIG: Dict[str, Any] = {}
if CONFIG_PATH.exists():
    CONFIG = yaml.safe_load(CONFIG_PATH.read_text(encoding="utf-8")) or {}

_run_dir = Path(CONFIG.get("paths", {}).get("output", BASE_DIR))
_run_dir.mkdir(parents=True, exist_ok=True)

orchestrator = LocalOrchestrator(
    state_path=_run_dir / "state.json",
    ledger_path=_run_dir / "ledger.jsonl",
)
vision = EdgeVisionClient(detections_path=_run_dir / "edge_detections.json")
integrity = IntegrityMonitor(
    watch_dir=Path(CONFIG.get("security", {}).get("watch_dir", BASE_DIR)),
    baseline_path=_run_dir / "file_hashes.json",
    critical_files=[BASE_DIR / "pyproject.toml"],
)

_orch_thread: Optional[threading.Thread] = None
_running = False


def _start_background() -> None:
    """Inicia orquestrador e monitor de integridade em threads."""
    global _orch_thread, _running
    if _running:
        return
    _running = True
    orchestrator.bridge.anchor("dashboard_boot", {"version": "5.0.0"})
    _orch_thread = threading.Thread(target=orchestrator.start, daemon=True)
    _orch_thread.start()
    # mantém uma detecção de exemplo para o dashboard renderizar glifos
    demo = {
        "detections": [
            {"label": "Aleph", "confidence": 0.95, "bbox": {"x": 10, "y": 10, "w": 50, "h": 50}},
            {"label": "Beth", "confidence": 0.87, "bbox": {"x": 70, "y": 30, "w": 40, "h": 60}},
            {"label": "Mem", "confidence": 0.72, "bbox": {"x": 20, "y": 80, "w": 45, "h": 45}},
        ]
    }
    (_run_dir / "edge_detections.json").write_text(json.dumps(demo), encoding="utf-8")


@app.route("/")
def index() -> str:
    return render_template("dashboard.html")


@app.route("/api/status")
def api_status() -> Any:
    det = vision.reload()
    status = {
        "orchestrator": orchestrator.status()["orchestrator"],
        "temporal": orchestrator.status()["temporal"],
        "vision": {
            "last_detection": vision.last_detection(),
            "detections": det,
            "camera_path": str(vision.detections_path),
        },
        "security": {
            "alerts": integrity.recent_alerts(10),
            "baseline": str(integrity.baseline_path),
        },
    }
    return jsonify(status)


@app.route("/api/health")
def api_health() -> Any:
    ok, count = orchestrator.bridge.verify()
    return jsonify({"ok": ok, "ledger_entries": count, "ts": time.time()})


def main() -> None:
    _start_background()
    host = CONFIG.get("dashboard", {}).get("host", "0.0.0.0")
    port = int(CONFIG.get("dashboard", {}).get("port", 8080))
    debug = bool(CONFIG.get("dashboard", {}).get("debug", False))
    if CONFIG.get("dashboard", {}).get("open_browser"):
        threading.Timer(1.5, lambda: webbrowser.open(f"http://127.0.0.1:{port}")).start()
    app.run(host=host, port=port, debug=debug, use_reloader=False)


if __name__ == "__main__":
    main()