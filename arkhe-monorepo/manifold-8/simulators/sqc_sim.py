"""
Simulador da SQC Interface — orquestração quântico-HPC.

Rota externa esperada pelo ArkheBridge:
  GET /health → 200 {"status": "ok", "service": "sqc-sim", "backends": [...]}
"""

import json
import os
from http.server import BaseHTTPRequestHandler, HTTPServer

HOST = "0.0.0.0"
PORT = int(os.environ.get("PORT", "8080"))
BACKENDS = os.environ.get("QUANTUM_BACKENDS", "reimei,ibm_kobe").split(",")


class SQCHandler(BaseHTTPRequestHandler):
    def _send(self, code: int, payload: dict) -> None:
        body = json.dumps(payload).encode("utf-8")
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self) -> None:
        if self.path == "/health":
            return self._send(
                200,
                {
                    "status": "ok",
                    "service": "sqc-sim",
                    "platform": "NVIDIA CUDA-Q",
                    "backends": BACKENDS,
                    "connected_systems": ["Fugaku", "ibm_kobe", "Reimei"],
                },
            )
        self._send(404, {"error": "not found"})

    def log_message(self, fmt, *args) -> None:
        print(f"[sqc-sim] {fmt % args}")


def main() -> None:
    server = HTTPServer((HOST, PORT), SQCHandler)
    print(f"SQC simulator listening on {HOST}:{PORT} (backends={BACKENDS})")
    server.serve_forever()


if __name__ == "__main__":
    main()