#!/usr/bin/env python3
"""
fixture.py — Gera a fixture mínima GGUF v3 (24 bytes, 0 tensores, 0 KV).
Determinística: os bytes são fixos; os hashes são calculados localmente.
"""

import hashlib
import struct
from pathlib import Path

MAGIC = b"GGUF"
VERSION = 3
TENSOR_COUNT = 0
KV_COUNT = 0

def generate_fixture() -> bytes:
    """Retorna os 24 bytes do header GGUF v3."""
    return (
        MAGIC
        + struct.pack("<I", VERSION)
        + struct.pack("<Q", TENSOR_COUNT)
        + struct.pack("<Q", KV_COUNT)
    )

def main():
    fixture = generate_fixture()
    assert len(fixture) == 24, f"Esperado 24 bytes, obtido {len(fixture)}"

    out = Path("arkhe.gguf")
    out.write_bytes(fixture)

    sha256 = hashlib.sha256(fixture).hexdigest()

    try:
        import blake3
        b3 = blake3.blake3(fixture).hexdigest()
    except ImportError:
        b3 = "[instalar: pip install blake3]"

    print(f"Ficheiro:  {out}")
    print(f"Tamanho:   {len(fixture)} bytes")
    print(f"Hex:       {fixture.hex()}")
    print(f"SHA-256:   {sha256}")
    print(f"BLAKE3:    {b3}")

if __name__ == "__main__":
    main()
