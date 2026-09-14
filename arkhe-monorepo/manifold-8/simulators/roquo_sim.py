"""
Simulador do ROQUO — expõe a API de submissão e consulta de jobs.

Rota externa esperada pelo ArkheBridge:
  GET  /health          → 200 {"status": "ok"}
  POST /jobs            → 202 {"job_id": ..., "status": "queued"}
  GET  /jobs/{job_id}   → 200 {"job_id": ..., "status": "completed"}
"""

import json
import os
import time
import uuid
from http.server import BaseHTTPRequestHandler, HTTPServer

HOST = "0.0.0.0"
PORT = int(os.environ.get("PORT", "8080"))
JOBS: dict = {}


class RoquoHandler(BaseHTTPRequestHandler):
    def _send(self, code: int, payload: dict) -> None:
        body = json.dumps(payload).encode("utf-8")
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def _read_body(self) -> dict:
        length = int(self.headers.get("Content-Length", 0))
        if length == 0:
            return {}
        return json.loads(self.rfile.read(length) or b"{}")

    def do_GET(self) -> None:
        if self.path == "/health":
            return self._send(200, {"status": "ok", "service": "roquo-sim"})

        if self.path.startswith("/jobs/"):
            job_id = self.path.split("/")[2]
            job = JOBS.get(job_id)
            if not job:
                return self._send(404, {"error": "job not found"})
            job["status"] = "completed"
            return self._send(200, job)

        self._send(404, {"error": "not found"})

    def do_POST(self) -> None:
        if self.path == "/jobs":
            body = self._read_body()
            job_id = f"roquo-sim-{uuid.uuid4().hex[:8]}"
            JOBS[job_id] = {
                "job_id": job_id,
                "status": "queued",
                "priority": body.get("priority", "normal"),
                "payload": body.get("payload"),
                "submitted_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
            }
            return self._send(202, dict(JOBS[job_id]))

        self._send(404, {"error": "not found"})

    def log_message(self, fmt, *args) -> None:
        print(f"[roquo-sim] {fmt % args}")


def main() -> None:
    server = HTTPServer((HOST, PORT), RoquoHandler)
    print(f"ROQUO simulator listening on {HOST}:{PORT}")
    server.serve_forever()


if __name__ == "__main__":
    main()