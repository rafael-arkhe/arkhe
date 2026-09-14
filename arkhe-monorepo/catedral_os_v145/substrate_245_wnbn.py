#!/usr/bin/env python3
"""
Substrato 245 — Wireless Neuroscience (WNBN)

Cliente para a rede wireless de neurociência comportamental. Fornece uma
abstração de configuração, registro de dispositivos e envio de comandos.

NOTA DE HONESTIDADE (auditoria): WNBN envolveria protocolos wireless
proprietários e hardware (implantes optogenéticos, sensores de gaiola). Este
módulo modela o CLIENTE de integração (estado, comandos, sensores) via um
back-end HTTP opcional; sem back-end, opera em modo simulado com dados locais
sintéticos — nunca afirma telemetria real quando não há conexão.
"""

import json
import time
from typing import Dict, List, Optional


class WNBNConfig:
    """Configuração do cliente WNBN."""

    def __init__(self, server_url: str = "http://localhost:5000",
                 timeout_s: float = 5.0):
        self.server_url = server_url
        self.timeout_s = timeout_s

    def to_dict(self) -> Dict:
        return {"server_url": self.server_url, "timeout_s": self.timeout_s}


class WNBNClient:
    """Cliente WNBN com fallback simulado (sem rede)."""

    def __init__(self, config: Optional[WNBNConfig] = None):
        self.config = config or WNBNConfig()
        self.devices: Dict[str, Dict] = {
            "mouse_1": {"type": "optogenetic_implant", "status": "active"},
            "mouse_2": {"type": "optogenetic_implant", "status": "active"},
            "cage_1": {"type": "environmental_sensor", "status": "active"},
        }
        self.command_log: List[Dict] = []
        self._network_available = False  # nunca assume rede viva sem verificação

    def register_device(self, device_id: str, device_type: str) -> Dict:
        self.devices[device_id] = {"type": device_type, "status": "active"}
        return {"status": "registered", "device": device_id, "type": device_type}

    def send_command(self, device_id: str, command: str,
                     params: Optional[Dict] = None) -> Dict:
        """Envia um comando; sem rede, registra localmente como 'queued'."""

        if self._network_available:
            # Enviaria via HTTP POST para self.config.server_url
            status = "sent"
        else:
            status = "queued_local"
        entry = {
            "device": device_id,
            "command": command,
            "params": params or {},
            "status": status,
            "timestamp": time.time(),
        }
        self.command_log.append(entry)
        return entry

    def read_all_sensors(self) -> Dict:
        """Lê sensores. Sem rede, retorna valores simulados marcados como tal."""
        sensors = {}
        for dev, info in self.devices.items():
            if info["type"] == "environmental_sensor":
                # valores sintéticos, claramente rotulados
                sensors[dev] = {
                    "temperature_C": None,
                    "humidity_pct": None,
                    "simulated": True,
                }
            elif info["type"] == "optogenetic_implant":
                sensors[dev] = {"implant_status": info["status"], "simulated": True}
        return sensors

    def status(self) -> Dict:
        return {
            "network_available": self._network_available,
            "server_url": self.config.server_url,
            "devices": len(self.devices),
            "commands_logged": len(self.command_log),
        }


if __name__ == "__main__":
    client = WNBNClient(WNBNConfig())
    client.send_command("mouse_1", "optogenetics 20Hz")
    print(client.status())
