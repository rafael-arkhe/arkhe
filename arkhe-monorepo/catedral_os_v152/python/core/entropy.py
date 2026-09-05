# python/core/entropy.py
"""
Entropia Construtiva — I167.
  S~ = -sum p log2 p + log2 N        (entropia espectral)
  A  = anti-flatness (I159)
  Phi_crit = limiar de Goedel (I158)
  custo = ceil(log2 n_handovers)
  V~ = S~ + A - Phi_crit - custo
"""

from __future__ import annotations

import math
from typing import List

from .spectral import compute_anti_flatness, godel_threshold


def log2_ceil(n: int) -> int:
    if n <= 1:
        return 0
    return int(math.ceil(math.log2(n)))


def spectral_entropy(spectrum: List[float], n_total: int) -> float:
    """S~ = -sum p log2 p + log2 N."""
    total = sum(spectrum)
    if total <= 0.0:
        return 0.0
    p = [x / total for x in spectrum]
    shannon = -sum(pi * math.log2(pi) for pi in p if pi > 0.0)
    log_n = math.log2(n_total) if n_total > 0 else 0.0
    return shannon + log_n


def constructive_entropy(spectrum: List[float], n_total: int,
                         n_handovers: int, alpha: float = 1.5) -> float:
    """I167: entropia construtiva do espectro."""
    s_tilde = spectral_entropy(spectrum, n_total)
    a = compute_anti_flatness(spectrum, alpha)
    phi_crit = godel_threshold(n_total)
    cost = float(log2_ceil(n_handovers))
    return s_tilde + a - phi_crit - cost