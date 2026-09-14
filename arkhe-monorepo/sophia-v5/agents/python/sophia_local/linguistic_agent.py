"""Agente de evolução linguística Fenício→Grego.

Integra os patches da Sophia (ψ_C, Q) para calibrar a simulação de evolução
fonética e gera relatório local (JSON) consumido pelo dashboard.
"""
from __future__ import annotations

import json
import math
from dataclasses import dataclass, field
from pathlib import Path
from typing import Dict, List, Optional

# Glifos fenícios base → letras gregas mapeadas
PHOENICIAN_TO_GREEK: Dict[str, str] = {
    "aleph": "alpha",
    "beth": "beta",
    "gimel": "gamma",
    "daleth": "delta",
    "he": "epsilon",
    "waw": "upsilon",
    "zayin": "zeta",
    "heth": "eta",
    "teth": "theta",
    "yodh": "iota",
    "kaph": "kappa",
    "lamedh": "lambda",
    "mem": "mu",
    "nun": "nu",
    "samekh": "xi",
    "ayin": "omicron",
    "pe": "pi",
    "tsade": "sampi",
    "qoph": "koppa",
    "resh": "rho",
    "shin": "sigma",
    "taw": "tau",
}


@dataclass
class LinguisticAgent:
    """Agente que evolui glifos fenícios sob influência do campo ψ_C."""

    vowel_bias_base: float = 0.22
    vowel_bias_psi_scale: float = 0.10
    report_interval_cycles: int = 50
    output_dir: Path = field(default_factory=lambda: Path("output"))
    cycles: int = 0

    def run_iteration(self, *, psi_c: float, q: float, detections: Optional[List[str]] = None) -> Dict[str, Dict[str, float]]:
        """Executa uma iteração e devolve o resultado por glifo detectado.

        Maior ψ_C → mais inovação vocálica (vowel_bias).
        Maior Q → mais fidelidade ao fluxo quântico (menor entropia).
        """
        self.cycles += 1
        vowel_bias = clamp(self.vowel_bias_base + self.vowel_bias_psi_scale * psi_c, 0.0, 1.0)
        glyphs = sorted(set(detections or []) or [k for k in PHOENICIAN_TO_GREEK.keys()][:8])

        results: Dict[str, Dict[str, float]] = {}
        entropy: List[float] = []
        for glyph in glyphs:
            conf = 0.5 + 0.4 * psi_c * (1.0 - 0.5 * q)
            innovation = vowel_bias if self._is_vowel(glyph) else vowel_bias * 0.5
            results[glyph] = {
                "confidence": round(conf, 4),
                "innovation": round(innovation, 4),
                "psi_C": round(psi_c, 6),
                "q": round(q, 6),
            }
            entropy.append(_entropy(conf, innovation))

        self._write_report(results, psi_c, q, entropy)
        return results

    @staticmethod
    def _is_vowel(glyph: str) -> bool:
        return PHOENICIAN_TO_GREEK.get(glyph, "") in {"alpha", "epsilon", "omicron", "upsilon", "eta"}

    def _write_report(self, results: Dict[str, Dict[str, float]], psi_c: float,
                      q: float, entropy: List[float]) -> None:
        self.output_dir.mkdir(parents=True, exist_ok=True)
        report = {
            "cycle": self.cycles,
            "psi_C": psi_c,
            "q_coefficient": q,
            "mean_entropy": round(sum(entropy) / len(entropy), 6) if entropy else 0.0,
            "glyphs": results,
        }
        (self.output_dir / "linguistic_metrics.json").write_text(
            json.dumps(report, indent=2, sort_keys=True), encoding="utf-8"
        )


def clamp(value: float, low: float, high: float) -> float:
    return low if value < low else high if value > high else value


def _entropy(conf: float, innovation: float) -> float:
    h = -(conf * math.log2(conf + 1e-9) + (1 - conf) * math.log2(1 - conf + 1e-9))
    return h + innovation