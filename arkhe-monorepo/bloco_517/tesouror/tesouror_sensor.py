"""tesouror_sensor.py — v77 VETADO — BLOCO 517 (F1)

Sensor de tesouraria: amostra receita/despesa e emite coerencia fiscal
(json) usando fiscal_calibration (F2). Sem R, sem rede.

VETAGEM: emit_json() e o contrato estavel consumido pelo sabricante externo;
o selftest roda hermetico.
"""

from __future__ import annotations

import json
import sys
from typing import Dict

sys.path.insert(0, __import__("os").path.dirname(__file__) + "/..")
from fiscal_calibration import HISTORY_2020_2024, calibrate, coherence_feed  # noqa: E402


class TesourorSensor:
    def __init__(self) -> None:
        self.params = calibrate(HISTORY_2020_2024)

    def sample(self, revenue: float, expense: float) -> Dict[str, float]:
        return {
            "revenue": revenue,
            "expense": expense,
            "coherence_feed": round(coherence_feed(revenue, expense, self.params), 6),
        }

    def emit_json(self, revenue: float, expense: float) -> str:
        return json.dumps(self.sample(revenue, expense))

    def calibrate_summary(self) -> Dict[str, object]:
        return {
            "eps": self.params["eps"],
            "gain": round(self.params["gain"], 4),
            "source": "2020-2024 embedding",
        }


if __name__ == "__main__":
    import tempfile

    sensor = TesourorSensor()
    fail = 0
    for (rev, exp) in [(620.0, 710.0), (1240.0, 980.0), (0.0, 0.0)]:
        s = sensor.sample(rev, exp)
        phi = s["coherence_feed"]
        if not (0.0 <= phi <= 1.0):
            fail += 1
            print(f"[TSE] FAIL: phi fora de [0,1] para ({rev},{exp}) -> {phi}")

    # leitura/reescrita do JSON emitido (contrato do sensor)
    _o = tempfile.NamedTemporaryFile("w", delete=False, suffix=".json", delete_on_close=False)
    _o.write(sensor.emit_json(1240.0, 980.0))
    _o.close()
    with open(_o.name, "r", encoding="utf-8") as fh:
        parsed = json.load(fh)
    if "coherence_feed" not in parsed:
        fail += 1
        print("[TSE] FAIL: json emitido invalido")
    else:
        print(f"[TSE] json emitido ok: {parsed}")

    print(f"[TSE] calibracao: {sensor.calibrate_summary()}")
    print(f"[TSE] {'ALL CHECKS PASS' if fail == 0 else 'CHECKS FAILED'}")
    sys.exit(1 if fail else 0)