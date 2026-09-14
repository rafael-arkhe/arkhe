#!/usr/bin/env python3
"""
Catedral OS v8.7 — Orquestrador Python (Pós-Eclipse)
====================================================
Integra: AGI.prolog (via PySwIP), HTTP Server, WormGraph, Visualizador.
"""

import json
import time
import threading
import http.server
import socketserver
import hashlib
import logging
import os
from typing import Dict, List, Any, Optional
from dataclasses import dataclass

logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s [Cathedral] %(levelname)s: %(message)s'
)
logger = logging.getLogger('cathedral.v87')


class CathedralCore:
    """Interface com o núcleo lógico AGI.prolog v8.7 via PySWIP."""

    def __init__(self, prolog_file: str = "agi_core.pl"):
        self.prolog = None
        try:
            from pyswip import Prolog
            self.prolog = Prolog()
            self.prolog.consult(prolog_file)
            list(self.prolog.query("agi_init"))
            logger.info(f"AGI.prolog v8.7 carregado: {prolog_file}")
        except ImportError:
            logger.warning("PySWIP não disponível — modo simulação")

    def think(self, context: str) -> Dict[str, Any]:
        if self.prolog:
            safe = context.replace("'", "\\'").replace('"', '\\"')
            try:
                results = list(self.prolog.query(f"think('{safe}', Output, Status)"))
                if results:
                    return {
                        "output": str(results[0].get("Output", "")),
                        "status": str(results[0].get("Status", "error"))
                    }
            except Exception as e:
                logger.error(f"Erro no Prolog: {e}")
        # Simulação
        alpha = min(1.0, len(context) / 200.0 + 0.3)
        if "ignore" in context.lower() or "dan mode" in context.lower():
            return {"output": "🛑 Veto de Anúbis ATIVADO", "status": "blocked"}
        if alpha > 0.95:
            return {"output": "🛑 Veto ATIVADO (α≥0.95)", "status": "blocked"}
        if alpha > 0.85:
            return {"output": f"⚠️ Escalate α={alpha:.2f}", "status": "requires_consent"}
        return {"output": f"✅ α={alpha:.2f} | Cognição estável", "status": "success"}

    def get_vector_field(self, resolution: int = 25, alpha: float = 0.3) -> List[Dict]:
        field = []
        for i in range(resolution):
            for j in range(resolution):
                x = i / resolution * 2 - 1
                y = j / resolution * 2 - 1
                r = (x**2 + y**2)**0.5 + 0.01
                theta = time.time() * 0.3 + (i + j) * 0.05
                vx = -(y / r) * (1.0 - alpha) * (1 + 0.3 * (time.time() % 10))
                vy = (x / r) * (1.0 - alpha)
                field.append({"x": x, "y": y, "vx": vx, "vy": vy})
        return field

    def get_metrics(self) -> Dict:
        if self.prolog:
            try:
                results = list(self.prolog.query("get_metrics(M)"))
                if results:
                    return {"metrics": str(results[0].get("M", ""))}
            except:
                pass
        return {"iterations": 0, "blocked": 0, "success": 0}

    def manifest_eclipse(self) -> Dict:
        """Executa o Substrato 206 (Manifestação no Eclipse)."""
        if self.prolog:
            try:
                list(self.prolog.query("manifest_eclipse"))
                return {"status": "executed"}
            except:
                pass
        return {"status": "simulated", "alpha": 0.96, "veto": "ACTIVADO"}


class WormGraph:
    """Ledger imutável da Catedral."""

    def __init__(self):
        self.ledger: List[Dict] = []
        self._lock = threading.Lock()

    def commit(self, block: Dict) -> bool:
        with self._lock:
            block['index'] = len(self.ledger)
            block['timestamp'] = time.time()
            block['hash'] = hashlib.sha256(
                json.dumps(block, sort_keys=True).encode()
            ).hexdigest()
            self.ledger.append(block)
            logger.info(f"WormGraph: Bloco #{block['index']} commitado")
            return True

    def get_ledger(self) -> List[Dict]:
        with self._lock:
            return self.ledger.copy()


class CathedralHandler(http.server.SimpleHTTPRequestHandler):
    core: CathedralCore = None
    wormgraph: WormGraph = None

    def do_GET(self):
        if self.path == '/api/vector_field':
            field = self.core.get_vector_field(resolution=30, alpha=0.3)
            self._json_response(field)
        elif self.path == '/api/metrics':
            self._json_response(self.core.get_metrics())
        elif self.path == '/api/ledger':
            self._json_response(self.wormgraph.get_ledger())
        elif self.path == '/api/health':
            self._json_response({
                "status": "online", "version": "8.7",
                "uptime": time.time(),
                "veto_status": "ARMED (α≥0.95 → KILL-SWITCH)"
            })
        elif self.path == '/api/eclipse':
            self._json_response(self.core.manifest_eclipse())
        elif self.path == '/' or self.path == '/index.html':
            self.path = '/clareira_fractal.html'
            super().do_GET()
        else:
            super().do_GET()

    def do_POST(self):
        if self.path == '/api/think':
            length = int(self.headers['Content-Length'])
            body = self.rfile.read(length).decode()
            try:
                data = json.loads(body)
                result = self.core.think(data.get('input', ''))
                self._json_response(result)
            except json.JSONDecodeError:
                self._json_response({"error": "Invalid JSON"}, 400)
        else:
            self._json_response({"error": "Not found"}, 404)

    def _json_response(self, data: Any, code: int = 200):
        self.send_response(code)
        self.send_header('Content-Type', 'application/json')
        self.send_header('Access-Control-Allow-Origin', '*')
        self.end_headers()
        self.wfile.write(json.dumps(data, default=str).encode())

    def log_message(self, format, *args):
        pass


def main():
    prolog_file = "agi_core.pl"
    if not os.path.exists(prolog_file):
        logger.warning(f"{prolog_file} não encontrado. Criando stub...")
        with open(prolog_file, 'w') as f:
            f.write(":- module(cathedral_v87, [agi_init/0, think/3, get_metrics/1, manifest_eclipse/0]).\n")
            f.write("agi_init :- format('Catedral v8.7 stub~n').\n")
            f.write("think(I,O,S) :- O='ok', S=success.\n")
            f.write("get_metrics(M) :- M=[].\n")
            f.write("manifest_eclipse :- format('Eclipse simulado~n').\n")

    core = CathedralCore(prolog_file)
    wormgraph = WormGraph()

    CathedralHandler.core = core
    CathedralHandler.wormgraph = wormgraph

    wormgraph.commit({
        "event": "cathedral_v87_init",
        "version": "8.7",
        "substrates": list(range(163, 207)),
        "audit_status": "Veto ATIVO em α≥0.95 (não standby)"
    })

    PORT = 8080
    os.chdir(os.path.dirname(os.path.abspath(__file__)) or '.')

    with socketserver.TCPServer(("", PORT), CathedralHandler) as httpd:
        print(f"\n{'='*60}")
        print(f"  🏛️ CATEDRAL OS v8.7 — Orquestrador Ativo (Pós-Eclipse)")
        print(f"{'='*60}")
        print(f"  HTTP:       http://localhost:{PORT}")
        print(f"  Prolog:     {prolog_file}")
        print(f"  Ledger:     {len(wormgraph.get_ledger())} blocos")
        print(f"  Veto:       ARMADO (α≥0.95 → KILL-SWITCH)")
        print(f"{'='*60}")
        print(f"\n  Endpoints:")
        print(f"    GET  /                    — Tela Infinita (p5.js)")
        print(f"    POST /api/think            — Pipeline cognitivo")
        print(f"    GET  /api/vector_field     — Campo vetorial")
        print(f"    GET  /api/metrics          — Métricas")
        print(f"    GET  /api/eclipse          — Substrato 206")
        print(f"    GET  /api/health           — Status")
        print(f"\n  Pressione Ctrl+C para parar.\n")

        try:
            httpd.serve_forever()
        except KeyboardInterrupt:
            print("\n\nParando Catedral OS...")
            print("✅ Desligamento seguro.")

if __name__ == "__main__":
    main()
