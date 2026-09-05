# python/hardware/tcamera_pipeline.py
"""
Pipeline da T-Camera S3 (Percepcao) — gera handovers espectrais (I157.I160)
a partir de espectros de coerencia.

No desktop isto e um SIMULADOR honesto: nao acessa camera real; produz
cadeias de espectros deterministicas com ruido fixo (seed) para testes.
O pipeline embarcado (MicroPython/ESP32) vive em `vision/pipeline.py`.
"""

from __future__ import annotations

import random
import threading
import time
from dataclasses import dataclass, field
from typing import Any, Callable, Dict, List, Optional

from core.spectral import SpectralHandover


@dataclass
class FrameHandover:
    """Handover de percepcao saído da camera."""
    phi: float
    timestamp: float
    spectral: SpectralHandover
    metadata: Dict[str, Any] = field(default_factory=dict)


class TCameraPipeline:
    """Simulador da T-Camera S3 com callback assincrono de handover."""

    def __init__(self, fps: int = 60, seed: int = 7):
        self.fps = fps
        self._rng = random.Random(seed)
        self._running = False
        self._thread: Optional[threading.Thread] = None
        self._callback: Optional[Callable[[FrameHandover], None]] = None
        self._frame_count = 0

    def set_handover_callback(self, cb: Callable[[FrameHandover], None]) -> None:
        self._callback = cb

    def start(self) -> None:
        if self._running:
            return
        self._running = True
        self._thread = threading.Thread(target=self._loop, daemon=True)
        self._thread.start()

    def stop(self) -> None:
        self._running = False
        if self._thread is not None:
            self._thread.join(timeout=1.0)

    # ------------------------------------------------------------------ #
    def _loop(self) -> None:
        dt = 1.0 / self.fps
        while self._running:
            handover = self._capture()
            self._frame_count += 1
            if self._callback is not None:
                self._callback(handover)
            time.sleep(dt)

    def _capture(self) -> FrameHandover:
        # Simula uma distribuição espectral com pico dominante.
        base = 0.6 + 0.2 * self._rng.random()
        spectrum = [
            base + self._rng.uniform(-0.05, 0.05),
            base * 0.5 + self._rng.uniform(-0.02, 0.02),
            base * 0.3 + self._rng.uniform(-0.01, 0.01),
            base * 0.2 + self._rng.uniform(-0.01, 0.01),
        ]
        spectrum = [max(0.0, v) for v in spectrum]

        spectral = SpectralHandover.extract(spectrum, source="TCameraS3")
        return FrameHandover(
            phi=spectral.phi_irr,
            timestamp=time.time(),
            spectral=spectral,
            metadata={"frame": self._frame_count, "contract": spectral.anti_flatness},
        )