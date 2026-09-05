# catedral_os_v152/tests/test_bridge.py
"""Testes da bridge Z1T: framing G1, CRC, fallback (G11)."""

import pytest

from hardware.z1t_bridge import (
    Z1TBridge,
    crc16_xmodem,
    frame,
    simulate_z1t_response,
    unframe,
)


def test_crc16_stable():
    assert crc16_xmodem(b"Z1T_HANDSHAKE") == crc16_xmodem(b"Z1T_HANDSHAKE")
    assert crc16_xmodem(b"abc") != crc16_xmodem(b"abd")


def test_frame_unframe_roundtrip():
    payload = b"Z1T_HANDSHAKE"
    framed = frame(payload)
    assert unframe(framed) == payload


def test_corrupted_payload_rejected():
    payload = b"Z1T_EVIDENCE"
    framed = bytearray(frame(payload))
    framed[len(framed) // 2] ^= 0xFF
    assert unframe(bytes(framed)) is None


def test_corrupted_crc_rejected():
    framed = bytearray(frame(b"Z1T_INFER"))
    framed[-3] ^= 0x01
    assert unframe(bytes(framed)) is None


def test_prefix_garbage_skipped():
    framed = frame(b"payload")
    junk = b"\x00\x11\x22" + framed
    assert unframe(junk) == b"payload"


def test_bridge_fallback_is_deterministic():
    bridge = Z1TBridge(port="COM_NONE")  # porta inexistente
    assert bridge.connect()
    assert bridge.is_simulating  # honesto: sem pyserial/hardware, simula
    r1 = bridge.send_command(b"Z1T_INFER")
    r2 = bridge.send_command(b"Z1T_INFER")
    assert r1 == r2
    assert len(r1) == 20


def test_bridge_decode_inference():
    payload = simulate_z1t_response(b"cmd")
    bridge = Z1TBridge()
    phi = bridge.decode_inference(payload)
    assert 0.0 <= phi <= 1.0