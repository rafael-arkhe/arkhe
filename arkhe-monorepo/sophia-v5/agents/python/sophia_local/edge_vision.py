"""Cliente de visão em edge (consome detecções do Raspberry Pi / ESP32).

O dispositivo de borda grava um JSON local (rotacionado) em
`detections_path`; este módulo lê, filtra por confiança e disponibiliza
o resultado para o orquestrador — tudo sem rede.
"""
from __future__ import annotations

import json
import time
from dataclasses import dataclass, field
from pathlib import Path
from typing import Dict, List, Optional

# Mapeamento rótulo->glifo usado pelo agente linguístico
LABEL_TO_GLYPH: Dict[str, str] = {
    "Aleph": "aleph", "Beth": "beth", "Gimel": "gimel", "Daleth": "daleth",
    "He": "he", "Waw": "waw", "Zayin": "zayin", "Heth": "heth",
    "Teth": "teth", "Yodh": "yodh", "Kaph": "kaph", "Lamedh": "lamedh",
    "Mem": "mem", "Nun": "nun", "Samekh": "samekh", "Ayin": "ayin",
    "Pe": "pe", "Tsade": "tsade", "Qoph": "qoph", "Resh": "resh",
    "Shin": "shin", "Taw": "taw",
}


@dataclass
class EdgeVisionClient:
    """Lê o último lote de detecções produzido pelo pipeline na borda."""

    detections_path: Path = field(default_factory=lambda: Path("edge_detections.json"))
    min_confidence: float = 0.6
    _cache: List[Dict[str, object]] = field(default_factory=list)
    _cache_ts: float = 0.0
    ttl_s: float = 1.0

    def reload(self) -> List[Dict[str, object]]:
        now = time.time()
        if now - self._cache_ts < self.ttl_s:
            return self._cache
        self._cache_ts = now
        self._cache = self._read()
        return self._cache

    def _read(self) -> List[Dict[str, object]]:
        path = Path(self.detections_path)
        if not path.exists():
            return []
        try:
            raw = json.loads(path.read_text(encoding="utf-8"))
        except (json.JSONDecodeError, OSError):
            return []
        dets = raw.get("detections", []) if isinstance(raw, dict) else []
        out = []
        for d in dets:
            conf = float(d.get("confidence", 0.0))
            label = str(d.get("label", ""))
            if conf < self.min_confidence:
                continue
            out.append({"label": label, "glyph": LABEL_TO_GLYPH.get(label, label.lower()),
                        "confidence": conf, "bbox": d.get("bbox", {})})
        return out

    def detected_glyphs(self) -> List[str]:
        """Glifos detectados no último lote (para o agente linguístico)."""
        return [d["glyph"] for d in self.reload()]

    def last_detection(self) -> Optional[Dict[str, object]]:
        dets = self.reload()
        return dets[0] if dets else None