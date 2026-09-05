# python/vision/pipeline.py
"""
Pipeline de visao — extração de handovers espectrais (I157.I160) a partir de
uma imagem em escala de cinza.

Este modulo e o MODELO DE REFERENCIA desktop do pipeline embarcado (que roda
em MicroPython/OpenCV-espressif no ESP32-S3). No desktop ele trabalha sobre um
array numpy (simulacao honesta da T-Camera). O codigo embarcado equivalente
esta documentado no README do pacote.

Passos por frame:
  1.  Normalizacao do gradiente / contraste.
  2.  Cascas diadicas (I160): niveis 1..5 de resolucao.
  3.  Limiar de Goedel (I158) baseado na resolucao N.
  4.  Anti-flatness (I159) como desvio padrao relativo das cascas.
  5.  Coerencia irreduzivel (I157) sobre a resolução efetiva.
"""

from __future__ import annotations

import math
import time
from typing import Dict, List

from core.spectral import (
    compute_anti_flatness,
    godel_threshold,
    irreducible_coherence,
)

try:
    import numpy as np
except Exception:  # pragma: no cover - numpy ausente
    np = None  # type: ignore


def _contrast(img) -> float:
    """Contraste normalizado [0,1] do frame (numpy ou fallback python puro)."""
    if np is not None and isinstance(img, np.ndarray):
        flat = img.astype(np.float64).ravel()
        lo, hi = float(flat.min()), float(flat.max())
        if hi <= lo:
            return 0.0
        return float((flat.std() / 128.0))
    # Fallback python puro (list-of-lists)
    vals = [float(v) for row in img for v in row]
    if not vals:
        return 0.0
    mean = sum(vals) / len(vals)
    var = sum((v - mean) ** 2 for v in vals) / len(vals)
    return math.sqrt(var) / 128.0


def spectral_handovers_from_grid(grid: List[List[float]]) -> Dict[str, float]:
    """Extrai handover espectral de uma grade de intensidades (I157.I160)."""
    n = len(grid) * (len(grid[0]) if grid else 0)
    contrast = _contrast(grid)

    shells = [contrast * (1.0 / (2 ** k)) for k in range(1, 6)]
    phi_crit = godel_threshold(n + 1) \
        if n > 2 else 0.0
    anti_flatness = 0.0
    if shells and sum(shells) > 0:
        mean = sum(shells) / len(shells)
        var = sum((s - mean) ** 2 for s in shells) / len(shells)
        anti_flatness = math.sqrt(var) / (mean + 1e-8)
    phi_irr = irreducible_coherence(n + 1) if n > 0 else 0.0

    return {
        "phi_irr": phi_irr,
        "phi_crit": phi_crit,
        "shells": shells,
        "anti_flatness": anti_flatness,
        "contrast": contrast,
        "timestamp_ms": int(time.time() * 1000),
    }


def spectral_handovers(img) -> Dict[str, float]:
    """Aceita numpy 2D ou list-of-lists; retorna o handover espectral."""
    if np is not None and isinstance(img, np.ndarray):
        grid = img.tolist()
    else:
        grid = img
    return spectral_handovers_from_grid(grid)


def frame_packet(data: bytes) -> bytes:
    """G1: SOF + LEN + PAYLOAD + CRC16 + EOF (usado pelo barramento UART)."""
    from hardware.z1t_bridge import frame as _frame

    # Reutiliza o framing canonico do crate (mesmo layout).
    return _frame(data)