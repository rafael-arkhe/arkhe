"""
Simulador do Glass5D — armazenamento WORM em memória (persistido em disco).

Rota externa esperada pelo ArkheBridge:
  GET  /health              → 200 {"status": "ok"}
  POST /write               → 201 {"record_id": ..., "bytes_written": N, "worm": true}
  GET  /read/{record_id}    → 200 {"record_id": ..., "data": {...}}
  GET  /datasets            → 200 {"datasets": [...]}
"""

import json
import os
import time
import uuid
from http.server import BaseHTTPRequestHandler, HTTPServer

HOST = "0.0.0.0"
PORT = int(os.environ.get("PORT", "8080"))
DATA_DIR = os.environ.get("GLASS5D_DATA_DIR", "/tmp/glass5d")
WORM = os.environ.get("WORM_ENABLED", "true").lower() == "true"

os.makedirs(DATA_DIR, exist_ok=True)
STORE_PATH = os.path.join(DATA_DIR, "records.json")


def _load() -> dict:
    if os.path.exists(STORE_PATH):
        with open(STORE_PATH, "r", encoding="utf-8") as f:
            return json.load(f)
    return {}


def _save(records: dict) -> None:
    with open(STORE_PATH, "w", encoding="utf-8") as f:
        json.dump(records, f, ensure_ascii=False)


RECORDS = _load()


class Glass5DHandler(BaseHTTPRequestHandler):
    def _send(self, code: int, payload: dict) -> None:
        body = json.dumps(payload, default=str).encode("utf-8")
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
            return self._send(200, {"status": "ok", "service": "glass5d-sim", "worm": WORM})

        if self.path.startswith("/read/"):
            record_id = self.path.split("/")[2]
            record = RECORDS.get(record_id)
            if not record:
                return self._send(404, {"error": "record not found"})
            return self._send(200, {"record_id": record_id, "data": record["data"]})

        if self.path == "/datasets":
            datasets = [
                {
                    "dataset": v["dataset"],
                    "record_id": k,
                    "bytes_written": v["bytes_written"],
                    "created_at": v["created_at"],
                }
                for k, v in RECORDS.items()
            ]
            return self._send(200, {"datasets": datasets})

        self._send(404, {"error": "not found"})

    def do_POST(self) -> None:
        if self.path == "/write":
            body = self._read_body()
            record_id = f"g5d-sim-{uuid.uuid4().hex[:12]}"
            payload_bytes = len(json.dumps(body.get("data", {}), default=str).encode("utf-8"))
            layers = max(1, int(body.get("options", {}).get("layer_count", 1)))
            bytes_written = payload_bytes * layers

            RECORDS[record_id] = {
                "dataset": body.get("dataset", "engram_default"),
                "data": body.get("data", {}),
                "bytes_written": bytes_written,
                "created_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
                "metadata": body.get("metadata", {}),
                "worm": WORM,
            }
            _save(RECORDS)
            return self._send(
                201,
                {
                    "record_id": record_id,
                    "bytes_written": bytes_written,
                    "status": "archived",
                    "worm": WORM,
                    "lifetime_years": 100_000,
                },
            )

        self._send(404, {"error": "not found"})

    def log_message(self, fmt, *args) -> None:
        print(f"[glass5d-sim] {fmt % args}")


def main() -> None:
    server = HTTPServer((HOST, PORT), Glass5DHandler)
    print(f"Glass5D simulator listening on {HOST}:{PORT} (store={STORE_PATH})")
    server.serve_forever()


if __name__ == "__main__":
    main()