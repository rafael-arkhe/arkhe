"""
Merkle Mountain Range (MMR) implementation for Arkhe-Test.
"""

import hashlib
from typing import List, Optional, Tuple, Dict, Any


def hash_leaf(data: bytes) -> str:
    return hashlib.sha256(b"\x00" + data).hexdigest()


def hash_nodes(left: str, right: str) -> str:
    return hashlib.sha256(b"\x01" + bytes.fromhex(left) + bytes.fromhex(right)).hexdigest()


class MerkleMountainRange:
    def __init__(self, initial_leaves: Optional[List[str]] = None):
        self.leaves: List[str] = []
        self.nodes: List[str] = []
        if initial_leaves:
            for l in initial_leaves:
                self.append(l)

    def append(self, leaf_hash: str) -> int:
        leaf_idx = len(self.leaves)
        self.leaves.append(leaf_hash)
        self.nodes.append(leaf_hash)
        return leaf_idx

    def get_peaks(self) -> List[str]:
        if not self.leaves:
            return []
        leaf_count = len(self.leaves)
        bits = []
        temp = leaf_count
        while temp > 0:
            bits.append(temp % 2)
            temp //= 2

        peaks = []
        current_offset = 0
        for power in range(len(bits) - 1, -1, -1):
            if bits[power] == 1:
                size = 1 << power
                subtree_leaves = self.leaves[current_offset: current_offset + size]
                peaks.append(self._merkle_root(subtree_leaves))
                current_offset += size
        return peaks

    def _merkle_root(self, leaves: List[str]) -> str:
        if not leaves:
            return hashlib.sha256(b"").hexdigest()
        layer = leaves
        while len(layer) > 1:
            next_layer = []
            for i in range(0, len(layer), 2):
                if i + 1 < len(layer):
                    next_layer.append(hash_nodes(layer[i], layer[i + 1]))
                else:
                    next_layer.append(layer[i])
            layer = next_layer
        return layer[0]

    def get_root(self) -> str:
        if not self.leaves:
            return hashlib.sha256(b"EMPTY_MMR").hexdigest()
        peaks = self.get_peaks()
        if len(peaks) == 1:
            return peaks[0]
        bag = peaks[-1]
        for p in reversed(peaks[:-1]):
            bag = hash_nodes(p, bag)
        root_final = hashlib.sha256(bytes.fromhex(bag) + len(self.leaves).to_bytes(8, 'big')).hexdigest()
        return root_final

    def verify_consistency(self, previous_snapshot_leaves: List[str]) -> Tuple[bool, str]:
        prev_len = len(previous_snapshot_leaves)
        curr_len = len(self.leaves)
        if curr_len < prev_len:
            return False, f"Truncation detected: current length {curr_len} < previous {prev_len}"
        for i in range(prev_len):
            if self.leaves[i] != previous_snapshot_leaves[i]:
                return False, f"Tamper detected at leaf index {i}: historical record rewritten"
        return True, "MMR is strictly append-only and consistent"

    def clone(self) -> "MerkleMountainRange":
        return MerkleMountainRange(list(self.leaves))
