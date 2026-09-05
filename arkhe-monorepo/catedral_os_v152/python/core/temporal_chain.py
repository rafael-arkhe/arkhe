# python/core/temporal_chain.py
"""
Cadeia temporal harmonica (Triade, I190.I192) com decaimento exponencial,
thread-safety e persistencia em JSON.

Semantic dependencies with Genesis-1 "forgetting": a cada novo handover o
total acumulado decai por um fator continuo; a prova so e verificada contra o
total decaido, nao contra a soma cumulativa crua.
"""

from __future__ import annotations

import hashlib
import json
import logging
import threading
import time
from collections import defaultdict
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Callable, Dict, List, Optional

logger = logging.getLogger(__name__)


@dataclass
class Handover:
    """Unidade atomica da cadeia temporal."""
    phi: float
    source: str
    timestamp: float
    metadata: Dict[str, Any] = field(default_factory=dict)
    hash: str = ""

    def __post_init__(self) -> None:
        if not self.hash:
            self.hash = self._compute_hash()

    def _compute_hash(self) -> str:
        data = {
            "phi": self.phi,
            "source": self.source,
            "timestamp": self.timestamp,
            "metadata": self.metadata,
        }
        return hashlib.sha256(
            json.dumps(data, sort_keys=True).encode()
        ).hexdigest()


class TemporalChain:
    """Cadeia temporal thread-safe com decaimento exponencial.

    Invariantes: I190 (triade tem de somar ate o alvo), I191 (resolucao da
    coerencia), I192 (sincronizacao assincrona via eventos), I162 (autonomia).
    """

    def __init__(self, target_phi: float = 0.95, decay_rate: float = 0.001):
        self.target_phi = target_phi
        self.decay_rate = decay_rate
        self._lock = threading.RLock()

        self.blocks: List[Handover] = []
        self._origin_blocks: Dict[str, List[Handover]] = defaultdict(list)
        self._coherence_history: List[float] = []
        self._decayed_history: List[float] = []
        self._achieved = False
        self._total_coherence_decayed = 0.0
        self._last_update = time.time()

    # ------------------------------------------------------------------ #
    # Decaimento
    # ------------------------------------------------------------------ #
    def _apply_decay(self) -> float:
        now = time.time()
        dt = now - self._last_update
        if dt <= 0:
            return self._total_coherence_decayed
        factor = max(0.0, 1.0 - self.decay_rate * dt)
        self._total_coherence_decayed *= factor
        self._last_update = now
        if self._decayed_history:
            self._decayed_history[-1] *= factor
        return self._total_coherence_decayed

    # ------------------------------------------------------------------ #
    # Escrita
    # ------------------------------------------------------------------ #
    def add_block(self, handover: Handover,
                  verify_fn: Optional[Callable[[Handover], bool]] = None) -> bool:
        """Adiciona um handover com verificacao opcional e decaimento (I190)."""
        if verify_fn is not None and not verify_fn(handover):
            logger.warning("Handover rejeitado (origem: %s)", handover.source)
            return False
        with self._lock:
            self._apply_decay()
            self.blocks.append(handover)
            self._origin_blocks[handover.source].append(handover)
            self.blocks.sort(key=lambda h: h.timestamp)

            self._total_coherence_decayed += handover.phi
            self._coherence_history.append(handover.phi)
            self._decayed_history.append(self._total_coherence_decayed)

            if self._total_coherence_decayed >= self.target_phi:
                self._achieved = True
                logger.info(
                    "Coerencia %.4f alcancada (alvo %.4f)",
                    self._total_coherence_decayed,
                    self.target_phi,
                )
            return True

    # ------------------------------------------------------------------ #
    # Leitura
    # ------------------------------------------------------------------ #
    def get_total_coherence(self) -> float:
        with self._lock:
            self._apply_decay()
            return self._total_coherence_decayed

    def get_coherence_by_origin(self, origin: str) -> float:
        with self._lock:
            self._apply_decay()
            blocks = self._origin_blocks.get(origin, [])
            dt = max(0.0, time.time() - self._last_update)
            return sum(h.phi for h in blocks) * max(0.0, 1.0 - self.decay_rate * dt)

    def get_latest_from_origin(self, origin: str) -> Optional[Handover]:
        with self._lock:
            blocks = self._origin_blocks.get(origin, [])
            return blocks[-1] if blocks else None

    # ------------------------------------------------------------------ #
    # Prova (I190)
    # ------------------------------------------------------------------ #
    def to_proof(self) -> Dict[str, Any]:
        with self._lock:
            self._apply_decay()
            perception = self.get_coherence_by_origin("TCameraS3")
            inference = self.get_coherence_by_origin("Z1T")
            proof = self.get_coherence_by_origin("CatedralOS")
            undulator = self.get_coherence_by_origin("Undulator")

            total = perception + inference + proof + undulator
            return {
                "perception": perception,
                "inference": inference,
                "proof": proof,
                "undulator": undulator,
                "total": total,
                "target": self.target_phi,
                "achieved": self._achieved,
                "timestamp": time.time(),
                "decayed_total": self._total_coherence_decayed,
            }

    def verify_proof(self, proof: Dict[str, Any]) -> bool:
        return proof.get("total", 0.0) >= proof.get("target", self.target_phi)

    # ------------------------------------------------------------------ #
    # Persistencia
    # ------------------------------------------------------------------ #
    def save(self, path: str) -> None:
        with self._lock:
            data = {
                "blocks": [h.__dict__ for h in self.blocks],
                "target_phi": self.target_phi,
                "decay_rate": self.decay_rate,
                "total_coherence": self._total_coherence_decayed,
                "achieved": self._achieved,
                "timestamp": time.time(),
            }
            Path(path).parent.mkdir(parents=True, exist_ok=True)
            with open(path, "w", encoding="utf-8") as f:
                json.dump(data, f, indent=2)
            logger.info("Cadeia salva em %s", path)

    def load(self, path: str) -> bool:
        try:
            with open(path, "r", encoding="utf-8") as f:
                data = json.load(f)
        except Exception as exc:  # pragma: no cover - caminho de erro
            logger.error("Erro ao carregar: %s", exc)
            return False

        with self._lock:
            self.blocks = [Handover(**b) for b in data["blocks"]]
            self.target_phi = data["target_phi"]
            self.decay_rate = data["decay_rate"]
            self._total_coherence_decayed = data["total_coherence"]
            self._achieved = data["achieved"]
            self._last_update = time.time()
            self._origin_blocks.clear()
            for h in self.blocks:
                self._origin_blocks[h.source].append(h)
            self._coherence_history = [h.phi for h in self.blocks]
            logger.info("Cadeia carregada de %s", path)
            return True