# python/core/spectral.py
"""
Estrutura Espectral da Coerência — Invariantes I157.I160
(mirror Python puro do crate Rust arkhe-catedral-v152).

  I157  Φ_irr = (3/2) log2(ln N), saturado em 1.0
  I158  Φ_crit = 1/log2(N), normalizado em 1.0
  I159  Anti-flatness A_alpha
  I160  Cascas diadicas (niveis floor(log2 i))
"""

from __future__ import annotations

import math
from dataclasses import dataclass, field
from typing import Dict, List


def irreducible_coherence(n: int) -> float:
    """I157: coerencia irreduzivel, saturada em 1.0."""
    if n <= 2:
        return 0.0
    return min(1.0, 1.5 * math.log2(math.log(n)))


def godel_threshold(n: int) -> float:
    """I158: limiar de Goedel normalizado."""
    if n <= 2:
        return 0.0
    return min(1.0, 1.0 / math.log2(n))


def compute_phi_irr(spectrum: List[float]) -> float:
    """Renyi-2 do espectro normalizado, limitado pelo universal (I157)."""
    n = len(spectrum)
    if n <= 2:
        return 0.0
    total = sum(spectrum)
    if total <= 0.0:
        return 0.0
    p = [x / total for x in spectrum]
    sum_sq = sum(x * x for x in p)
    renyi_2 = -math.log2(sum_sq)
    return min(renyi_2, irreducible_coherence(n))


def is_godel_threshold_reached(spectrum: List[float]) -> bool:
    """I158: o espectro ja cruzou o limiar de Goedel?"""
    n = len(spectrum)
    if n <= 2:
        return False
    return compute_phi_irr(spectrum) >= godel_threshold(n)


def compute_anti_flatness(phi: List[float], alpha: float = 1.5,
                          max_samples: int = 100) -> float:
    """I159: anti-flatness A_alpha do espectro.

    Calculada sobre pares (i < j) do espectro normalizado:
      A = sum_i,j p_i p_j (p_i^(a-1) - p_j^(a-1))^2
    Com extrapolacao se max_samples < n (I160 usa amostragem).
    """
    n = len(phi)
    if n < 2:
        return 0.0
    total = sum(phi)
    if total <= 0.0:
        return 0.0
    p = [x / total for x in phi]
    sample = min(n, max_samples)
    if sample < 2:
        return 0.0
    p_pow = [pi ** (alpha - 1.0) if pi > 1e-12 else 0.0 for pi in p]

    acc = 0.0
    for i in range(sample):
        pi, ppi = p[i], p_pow[i]
        for j in range(i + 1, sample):
            d = ppi - p_pow[j]
            acc += pi * p[j] * d * d
    if sample < n:
        acc *= n / sample
    return acc


def handover_level(idx: int) -> int:
    """I160: nivel da casca diadica do i-esimo handover (1-based)."""
    if idx <= 0:
        return 0
    return int(math.floor(math.log2(idx)))


def organize_in_shells(spectrum: List[float]) -> Dict[int, List[float]]:
    """I160: distribui o espectro em cascas diadicas por nivel."""
    shells: Dict[int, List[float]] = {}
    for i, val in enumerate(spectrum, start=1):
        shells.setdefault(handover_level(i), []).append(val)
    return shells


@dataclass
class SpectralHandover:
    """Handover espectral completo (I157.I160)."""
    phi_irr: float
    phi_crit: float
    anti_flatness: float
    shells: Dict[int, List[float]] = field(default_factory=dict)
    source: str = "TCameraS3"

    @classmethod
    def extract(cls, spectrum: List[float], source: str = "TCameraS3") -> "SpectralHandover":
        return cls(
            phi_irr=compute_phi_irr(spectrum),
            phi_crit=godel_threshold(len(spectrum)),
            anti_flatness=compute_anti_flatness(spectrum),
            shells=organize_in_shells(spectrum),
            source=source,
        )