#!/usr/bin/env python3
"""
Substrato 207/208 — Network Orchestrator (Rede 6G / Mesh)

Gerenciador de rede de comunicação para a Catedral OS. Fornece uma abstração
de configuração de rede, início/parada de interfaces simuladas e uma fila de
mensagens. Não fala com hardware real de rádio; opera em modo simulado e
relata em modo simulado (honestidade).
"""

import threading
import time
from typing import Dict, List, Optional
from dataclasses import dataclass, field


@dataclass
class NetworkConfig:
    """Configuração da rede."""
    interface: str = "cathedral0"
    ip: str = "10.0.0.1"
    mode: str = "simulated"  # simulated | (futuro: real)
    channel: str = "6G-1"
    max_queue: int = 1000


class NetworkOrchestrator:
    """Orquestrador de rede com fila de mensagens thread-safe."""

    def __init__(self, config: Optional[NetworkConfig] = None):
        self.config = config or NetworkConfig()
        self._running = False
        self._lock = threading.Lock()
        self._queue: List[Dict] = []
        self._sent: List[Dict] = []

    def start(self) -> Dict:
        with self._lock:
            self._running = True
            state = self._snapshot()
            self._sent.append({"kind": "ifup", **state, "ts": time.time()})
            return state

    def shutdown(self) -> Dict:
        with self._lock:
            self._running = False
            state = self._snapshot()
            self._sent.append({"kind": "ifdown", **state, "ts": time.time()})
            return state

    def is_running(self) -> bool:
        return self._running

    def send(self, destination: str, payload: Dict) -> Dict:
        msg = {
            "destination": destination,
            "payload": payload,
            "timestamp": time.time(),
            "queued": self._running,
        }
        with self._lock:
            if self._running and len(self._queue) < self.config.max_queue:
                self._queue.append(msg)
                return {"status": "queued", "message": msg}
            return {"status": "rejected_or_offline", "message": msg}

    def drain(self) -> List[Dict]:
        with self._lock:
            msgs = self._queue
            self._queue = []
            self._sent.extend(msgs)
            return msgs

    def _snapshot(self) -> Dict:
        return {
            "interface": self.config.interface,
            "ip": self.config.ip,
            "mode": self.config.mode,
            "channel": self.config.channel,
            "running": self._running,
            "queue_len": len(self._queue),
            "sent": len(self._sent),
        }

    def status(self) -> Dict:
        return self._snapshot()


if __name__ == "__main__":
    net = NetworkOrchestrator()
    net.start()
    print(net.send("node_2", {"cmd": "ping"}))
    print(net.status())
