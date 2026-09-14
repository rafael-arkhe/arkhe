"""tgn_persistence.py — v77 VETADO — BLOCO 517 (T1)

Persistencia de estado para o IncrementalTGN e o SelfCalibrator.

VETAGEM:
  - A proposta v77 usava `deque` sem importar (NameError). Corrigido aqui.
  - O repositorio NAO possui as classes IncrementalTGN/SelfCalibrator; o
    manager le atributos conhecidos via getattr com defaults, aceitando
    QUALQUER objeto duck-typed. O selftest usa MiniTGN/MiniCalibrator.
  - Persistencia empirica do Polar: buffer não serializa torch — apenas
    metadados (step_count, buffers, parametros, history) em JSON.
"""

from __future__ import annotations

import json
from collections import deque
from dataclasses import dataclass, field
from typing import Any, Deque, Dict, Optional


@dataclass
class TGNParams:
    g1: float = 1.0
    g2: float = 0.9
    threshold_awake: float = 0.618
    threshold_zeno: float = 0.500


@dataclass
class MiniCalibrator:
    params: TGNParams = field(default_factory=TGNParams)
    history: Deque[float] = field(default_factory=lambda: deque(maxlen=1000))
    calibration_step: int = 0


@dataclass
class MiniTGN:
    step_count: int = 0
    replay_buffer: Deque[Any] = field(default_factory=lambda: deque(maxlen=500))


class TGNStateManager:
    """Salva/restaura o estado observavel de TGN + Calibrator."""

    def __init__(self, filepath: str = "tgn_state.json") -> None:
        self.filepath = filepath

    def snapshot(self, tgn: Any, calibrator: Any) -> Dict[str, Any]:
        """Extrai estado (duck-typed, com defaults)."""
        return {
            "tgn": {
                "step_count": getattr(tgn, "step_count", 0),
                "memory_buffer": list(getattr(tgn, "replay_buffer", [])),
            },
            "calibrator": {
                "history": list(getattr(calibrator, "history", [])),
                "params": {
                    "g1": getattr(calibrator.params, "g1", 1.0),
                    "g2": getattr(calibrator.params, "g2", 0.9),
                    "threshold_awake": getattr(calibrator.params, "threshold_awake", 0.618),
                    "threshold_zeno": getattr(calibrator.params, "threshold_zeno", 0.5),
                },
                "calibration_step": getattr(calibrator, "calibration_step", 0),
            },
        }

    def save_state(self, tgn: Any, calibrator: Any) -> None:
        with open(self.filepath, "w", encoding="utf-8") as fh:
            json.dump(self.snapshot(tgn, calibrator), fh, indent=2, default=str)

    def restore(self, state: Dict[str, Any], tgn: Any, calibrator: Any) -> bool:
        """Aplica o estado persistido em objetos existentes."""
        try:
            tgn.step_count = state["tgn"]["step_count"]
            tgn.replay_buffer = deque(state["tgn"]["memory_buffer"], maxlen=500)
            calibrator.history = deque(state["calibrator"]["history"], maxlen=1000)
            calibrator.params.g1 = state["calibrator"]["params"]["g1"]
            calibrator.params.g2 = state["calibrator"]["params"]["g2"]
            calibrator.params.threshold_awake = state["calibrator"]["params"]["threshold_awake"]
            calibrator.params.threshold_zeno = state["calibrator"]["params"]["threshold_zeno"]
            calibrator.calibration_step = state["calibrator"]["calibration_step"]
            return True
        except Exception as exc:
            print(f"[TGN] erro ao carregar estado: {exc}")
            return False

    def load_state(self, tgn: Any, calibrator: Any) -> bool:
        try:
            with open(self.filepath, "r", encoding="utf-8") as fh:
                return self.restore(json.load(fh), tgn, calibrator)
        except (OSError, ValueError, json.JSONDecodeError) as exc:
            print(f"[TGN] estado ausente/corrompido: {exc}")
            return False


if __name__ == "__main__":
    import os
    import sys
    import tempfile

    fail = 0
    with tempfile.TemporaryDirectory() as tmp:
        path = os.path.join(tmp, "state.json")
        mgr = TGNStateManager(path)

        tgn = MiniTGN(step_count=168, replay_buffer=deque([1, 2, 3], maxlen=500))
        cal = MiniCalibrator()
        cal.history.extend([0.6, 0.7, 0.66])
        cal.params.g1 = 2.5
        cal.calibration_step = 12
        mgr.save_state(tgn, cal)

        tgn2 = MiniTGN()
        cal2 = MiniCalibrator()
        if not mgr.load_state(tgn2, cal2):
            fail += 1
            print("[TGN] FAIL: load_state")
        checks = (
            tgn2.step_count == 168
            and list(tgn2.replay_buffer) == [1, 2, 3]
            and list(cal2.history) == [0.6, 0.7, 0.66]
            and cal2.params.g1 == 2.5
            and cal2.calibration_step == 12
        )
        if not checks:
            fail += 1
            print("[TGN] FAIL: roundtrip")
        else:
            print(f"[TGN] roundtrip ok (step_count={tgn2.step_count})")

        with open(path, "w", encoding="utf-8") as fh:
            fh.write("{ nao-json")
        if mgr.load_state(MiniTGN(), MiniCalibrator()):
            fail += 1
            print("[TGN] FAIL: arquivo corrompido deveria falhar suave")
        else:
            print("[TGN] arquivo corrompido tratado sem crash")

    print(f"[TGN] {'ALL CHECKS PASS' if fail == 0 else 'CHECKS FAILED'}")
    sys.exit(1 if fail else 0)