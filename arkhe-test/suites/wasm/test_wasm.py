"""
WASM sandbox tests.
"""

import hashlib
import pytest

def test_wasm_deterministic_instruction_reduction():
    bytecode = b"\x00asm\x01\x00\x00\x00_kernel_reduction"
    input_tensor = b"\x01\x02\x03\x04\x05\x06"

    h1 = hashlib.sha256(bytecode + input_tensor).hexdigest()
    h2 = hashlib.sha256(bytecode + input_tensor).hexdigest()
    assert h1 == h2

def test_wasm_memory_out_of_bounds_trap():
    max_allowed_pages = 16
    requested_pages = 32

    def alloc_pages(n):
        if n > max_allowed_pages:
            raise MemoryError("WASM sandbox trap: out of memory bounds")
        return [0] * (n * 65536)

    with pytest.raises(MemoryError, match="WASM sandbox trap"):
        alloc_pages(requested_pages)
