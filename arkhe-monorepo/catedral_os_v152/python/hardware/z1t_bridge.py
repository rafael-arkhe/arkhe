# python/hardware/z1t_bridge.py
"""
Bridge Z1T — UART/USB com framing G1 (SOF+LEN+CRC16+EOF) e fallback para
simulacao (G8 reconexao com backoff, G11 fallback).

Nota de auditoria:
- `pyserial` e importado EN VERBE optional: sem ele, a bridge opera apenas em
  modo simulado (nenhuma comunicacao serial real e tentada), de forma honesta.
- A simulacao e deterministica (sem RNG), para manter testes reprodutiveis.
"""

from __future__ import annotations

import binascii
import struct
import threading
import time
from typing import Optional

try:
    import serial  # type: ignore
    _HAS_SERIAL = True
except Exception:  # pragma: no cover - ambiente sem pyserial
    serial = None  # type: ignore
    _HAS_SERIAL = False

SOF = 0xAA
EOF = 0xBB


def crc16_xmodem(data: bytes) -> int:
    """CRC16-XMODEM (mesmo polinomio do crate Rust)."""
    crc = 0x0000
    for b in data:
        crc ^= b << 8
        for _ in range(8):
            if crc & 0x8000:
                crc = ((crc << 1) ^ 0x1021) & 0xFFFF
            else:
                crc = (crc << 1) & 0xFFFF
    return crc


def frame(payload: bytes) -> bytes:
    """G1: [SOF][LEN:u16 BE][PAYLOAD][CRC16 BE][EOF]."""
    length = len(payload)
    return struct.pack(f">BH{length}sHB", SOF, length, payload, crc16_xmodem(payload), EOF)


def unframe(data: bytes) -> Optional[bytes]:
    """G1: extrai payload validando CRC e EOF. Retorna None se invalido."""
    if len(data) < 6:
        return None
    i = 0
    while i < len(data) - 1 and data[i] != SOF:
        i += 1
    if i >= len(data) - 1:
        return None
    length = struct.unpack(">H", data[i + 1:i + 3])[0]
    payload_start = i + 3
    payload_end = payload_start + length
    crc_pos = payload_end
    if crc_pos + 2 >= len(data) or data[crc_pos + 2] != EOF:
        return None
    payload = data[payload_start:payload_end]
    crc_recv = struct.unpack(">H", data[crc_pos:crc_pos + 2])[0]
    if crc_recv != crc16_xmodem(payload):
        return None
    return payload


def simulate_z1t_response(cmd: bytes) -> bytes:
    """Resposta deterministica: 16 p-bits (u8) + coerencia estimada (f32).

    A coerencia universal usa I157 com N=13 (constante canonica).
    """
    from .undulator import UndulatorNode

    payload = bytes((i * 3 + 128) & 0xFF for i in range(16))
    payload += struct.pack("<f", UndulatorNode.phi_reference())
    # Descartamos o comando, mas a coerencia nao depende dele no simulador.
    _ = cmd
    return payload


class Z1TBridge:
    """Bridge UART/USB para Z1T com reconexao (G8) e fallback (G11)."""

    def __init__(self, port: str = "COM3", baud: int = 115200,
                 max_retries: int = 3, backoff_s: float = 0.5):
        self.port = port
        self.baud = baud
        self.max_retries = max_retries
        self.backoff_s = backoff_s
        self._serial = None
        self._connected = False
        self._simulating = not _HAS_SERIAL
        self._lock = threading.RLock()
        self._last_contact = 0.0

    # ------------------------------------------------------------------ #
    # Conexao (G8)
    # ------------------------------------------------------------------ #
    def connect(self) -> bool:
        """Conecta com retry/backoff; degrada para simulacao (G11)."""
        if self._connected:
            return True
        with self._lock:
            if not _HAS_SERIAL:
                self._simulating = True
                return True
            for attempt in range(self.max_retries):
                try:
                    serial_port = serial.Serial(
                        self.port, self.baud, timeout=0.5, write_timeout=0.5
                    )
                    serial_port.write(frame(b"Z1T_HANDSHAKE"))
                    time.sleep(0.1)
                    if serial_port.in_waiting:
                        raw = serial_port.read(256)
                        if unframe(raw) is not None:
                            self._serial = serial_port
                            self._connected = True
                            self._simulating = False
                            self._last_contact = time.time()
                            return True
                    serial_port.close()
                except Exception:
                    pass
                time.sleep(self.backoff_s * (2 ** attempt))
            self._simulating = True
            return True  # fallback honesto

    # ------------------------------------------------------------------ #
    # Comando (G1 framing)
    # ------------------------------------------------------------------ #
    def send_command(self, cmd: bytes) -> Optional[bytes]:
        """Envia comando com framing; retorna payload da resposta ou None."""
        if not self._connected:
            self.connect()
        if self._simulating:
            return simulate_z1t_response(cmd)

        with self._lock:
            try:
                assert self._serial is not None
                self._serial.write(frame(cmd))
                time.sleep(0.01)
                if self._serial.in_waiting:
                    raw = self._serial.read(1024)
                    resp = unframe(raw)
                    if resp is not None:
                        self._last_contact = time.time()
                        return resp
            except Exception:
                self._connected = False
            self._simulating = True
            return simulate_z1t_response(cmd)

    def disconnect(self) -> None:
        with self._lock:
            if self._serial is not None:
                try:
                    self._serial.close()
                except Exception:
                    pass
            self._serial = None
            self._connected = False
            self._simulating = not _HAS_SERIAL

    # ------------------------------------------------------------------ #
    # Estado
    # ------------------------------------------------------------------ #
    @property
    def is_connected(self) -> bool:
        return self._connected

    @property
    def is_simulating(self) -> bool:
        return self._simulating

    def decode_inference(self, payload: bytes) -> float:
        """Extrai a coerencia de inferencia do payload Z1T (f32 final)."""
        if len(payload) < 4:
            return 0.0
        phi, = struct.unpack_from("<f", payload, len(payload) - 4)
        return max(0.0, min(1.0, phi))