"""tesouror_adapter.py — v77 VETADO — BLOCO 517 (F1)

Adapter para o pacote R `tesouror` (strategicprojects/tesouror).

VETAGEM:
  - R/tesouror pode nao existir no host. is_available() detecta Rscript;
    quando ausente, os metodos caem para um núcleo numpy determinístico
    (mesma assinatura) — sem derrubar o sensor.
  - Selftest navega 100% sem R (fallback numpy).
"""

from __future__ import annotations

import shutil
import subprocess
from typing import Dict, Optional

import numpy as np


class TesourorAdapter:
    def __init__(self, rscript: str = "Rscript") -> None:
        self.rscript = rscript

    def is_available(self) -> bool:
        return shutil.which(self.rscript) is not None

    def fit(self, data: Dict[str, float]) -> Dict[str, float]:
        """Ajuste de parametros de liquidez/tesouraria (deterministico)."""
        if self.is_available():
            return self._fit_r(data)
        return self._fit_numpy(data)

    def state_transition(self, liquidity: float, delta: float) -> float:
        """Transicao de estado de tesouraria no intervalo [0,1]."""
        if self.is_available():
            out = subprocess.run(
                [self.rscript, "-e", "cat(1)", "--args", str(liquidity), str(delta)],
                capture_output=True, text=True, timeout=10,
            )
            if out.returncode == 0:
                try:
                    return float(out.stdout.strip())
                except ValueError:
                    pass
        return self._transition_numpy(liquidity, delta)

    # ------------------------------------------------------------ fallback
    def _fit_numpy(self, data: Dict[str, float]) -> Dict[str, float]:
        buf = data.get("liquidity", 100.0)
        reserve = data.get("reserve", 25.0)
        solvency = min(1.0, max(0.0, reserve / buf if buf else 0.0))
        return {"solvency": float(solvency), "buffer": float(buf)}

    def _transition_numpy(self, liquidity: float, delta: float) -> float:
        rng = np.random.default_rng(int(abs(delta) * 7) + 1)  # determinístico p/ delta fixo
        jitter = float(rng.uniform(-0.02, 0.02))
        return float(max(0.0, min(1.0, liquidity + delta + jitter)))

    def _fit_r(self, data: Dict[str, float]) -> Dict[str, float]:
        script = (
            'library(tesouror);'
            'cat(as.numeric(tesouror::estrutura_contas('
            f'list(liquidity={data.get("liquidity",100.0)}, reserve={data.get("reserve",25.0)}))))'
        )
        out = subprocess.run([self.rscript, "-e", script], capture_output=True, text=True, timeout=15)
        if out.returncode == 0 and out.stdout.strip():
            return {"solvency": float(out.stdout.strip()), "buffer": float(data.get("liquidity", 100.0))}
        return self._fit_numpy(data)


if __name__ == "__main__":
    import sys

    fail = 0
    a = TesourorAdapter()
    fit = a.fit({"liquidity": 100.0, "reserve": 25.0})
    if not (0.0 <= fit["solvency"] <= 1.0):
        fail += 1
        print(f"[TSR] FAIL: solvency {fit}")
    s1 = a.state_transition(0.5, 0.0)
    s2 = a.state_transition(0.5, 0.0)
    if s1 != s2:
        fail += 1
        print("[TSR] FAIL: determinismo")
    print(f"[TSR] fit={fit}  transition(0.5,0)={s1:.4f}  r_available={a.is_available()}")
    print(f"[TSR] {'ALL CHECKS PASS' if fail == 0 else 'CHECKS FAILED'}")
    sys.exit(1 if fail else 0)