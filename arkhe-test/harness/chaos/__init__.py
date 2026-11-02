import random

def inject_bit_flip(data: bytes, flips: int = 1) -> bytes:
    if not data:
        return data
    arr = bytearray(data)
    for _ in range(flips):
        pos = random.randint(0, len(arr) - 1)
        bit = 1 << random.randint(0, 7)
        arr[pos] ^= bit
    return bytes(arr)
