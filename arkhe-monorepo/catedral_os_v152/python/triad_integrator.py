# python/triad_integrator.py
"""
Integrador da Triade Hibrida + Undulator (Percepcao -> Inferencia -> Prova ->
Decaimento).

Três componentes assíncronos (event-driven, I192):
  - `_on_perception`: recebe handover da T-Camera e encadeia.
  - `_inference_loop`: quando ha nova percepcao, infere via Z1T (ou sim).
  - `_proof_loop`: quando ha nova inferencia, gera prova + handover do
    Undulator.

Sem hardware real, `Z1TBridge` degrada para simulacao determinística (G11);
a T-Camera usa `TCameraPipeline` em modo simulado — documentado, nao
falso-verde.
"""

from __future__ import annotations

import logging
import threading
import time
from typing import Dict, Optional

import numpy as np

from core.temporal_chain import TemporalChain, Handover
from hardware.tcamera_pipeline import TCameraPipeline, FrameHandover
from hardware.undulator import RangingDelta, UndulatorNode
from hardware.z1t_bridge import Z1TBridge

logger = logging.getLogger(__name__)


class TriadIntegrator:
    """Integrador com eventos: prova so e gerada quando novos dados chegam."""

    def __init__(self, target_phi: float = 0.95, decay_rate: float = 0.001,
                 port: str = "COM3", baud: int = 115200,
                 ranging_us: int = 1000):
        self.target_phi = target_phi
        self.decay_rate = decay_rate
        self.port = port
        self.baud = baud

        self.tcamera = TCameraPipeline()
        self.z1t = Z1TBridge(port=self.port, baud=self.baud)
        self.chain = TemporalChain(target_phi, decay_rate)
        self.undulator = UndulatorNode(RangingDelta(ranging_us))

        self.running = False
        self._new_perception = threading.Event()
        self._new_inference = threading.Event()
        self._latest_perception: Optional[Handover] = None
        self._latest_inference: Optional[Handover] = None

        self.stats: Dict[str, int] = {
            "perception_count": 0,
            "inference_count": 0,
            "proof_count": 0,
            "undulator_count": 0,
        }

    # ------------------------------------------------------------------ #
    # Ciclo de vida
    # ------------------------------------------------------------------ #
    def start(self) -> None:
        self.running = True
        if not self.z1t.connect():
            logger.warning("Z1T nao conectado — usando simulacao")
        self.tcamera.set_handover_callback(self._on_perception)
        self.tcamera.start()

        self._inference_thread = threading.Thread(target=self._inference_loop, daemon=True)
        self._inference_thread.start()
        self._proof_thread = threading.Thread(target=self._proof_loop, daemon=True)
        self._proof_thread.start()
        logger.info("Triade + Undulator iniciada (event-driven)")

    def stop(self) -> None:
        self.running = False
        self.tcamera.stop()
        self.z1t.disconnect()
        for name in ("_inference_thread", "_proof_thread"):
            t = getattr(self, name, None)
            if t is not None:
                t.join(timeout=2.0)
        logger.info("Triade encerrada")

    # ------------------------------------------------------------------ #
    # Percepcao
    # ------------------------------------------------------------------ #
    def _on_perception(self, frame_handover: FrameHandover) -> None:
        if not self.running:
            return
        handover = Handover(
            phi=frame_handover.phi,
            source="TCameraS3",
            timestamp=frame_handover.timestamp,
            metadata=frame_handover.metadata,
        )
        self.chain.add_block(handover)
        self._latest_perception = handover
        self.stats["perception_count"] += 1
        self._new_perception.set()

    # ------------------------------------------------------------------ #
    # Inferencia
    # ------------------------------------------------------------------ #
    def _inference_loop(self) -> None:
        while self.running:
            if not self._new_perception.wait(timeout=1.0):
                continue
            self._new_perception.clear()

            latest = self._latest_perception
            if latest is None:
                continue
            try:
                evidence = np.full(16, latest.phi, dtype=np.float32)
                evidence += np.random.normal(0, 0.01, 16)
                evidence = np.clip(evidence, 0, 1)

                # Z1TBridge degrada para simulacao deterministica quando nao
                # ha hardware (G11).
                resp = self.z1t.send_command(evidence.astype("<f4").tobytes())
                phi_inference = self.z1t.decode_inference(resp) if resp else float(evidence.mean())

                handover = Handover(
                    phi=float(phi_inference),
                    source="Z1T",
                    timestamp=time.time(),
                    metadata={"posterior": evidence.tolist()},
                )
                self.chain.add_block(handover)
                self._latest_inference = handover
                self.stats["inference_count"] += 1
                self._new_inference.set()
            except Exception as exc:  # pragma: no cover - fluxo de erro
                logger.error("Erro na inferencia: %s", exc)

    # ------------------------------------------------------------------ #
    # Prova + Undulator
    # ------------------------------------------------------------------ #
    def _proof_loop(self) -> None:
        while self.running:
            if not self._new_inference.wait(timeout=2.0):
                continue
            self._new_inference.clear()
            try:
                proof = self.chain.to_proof()

                if proof["total"] > 0:
                    proof_handover = Handover(
                        phi=proof["total"],
                        source="CatedralOS",
                        timestamp=time.time(),
                        metadata=proof,
                    )
                    self.chain.add_block(proof_handover)
                    self.stats["proof_count"] += 1

                # Handover do Undulator (decaimento) — o nó aplica o Zeno Veto.
                u_handover = self.undulator.receive_handover(
                    proof["total"], source="CatedralOS"
                )
                if u_handover is not None:
                    chain_handover = Handover(
                        phi=u_handover.decayed_phi,
                        source="Undulator",
                        timestamp=u_handover.timestamp,
                        metadata={
                            "decayed": True,
                            "decay_rate": self.undulator.stats()["decay_rate"],
                        },
                    )
                    self.chain.add_block(chain_handover)
                    self.stats["undulator_count"] += 1

                if self.chain._achieved:
                    logger.info(
                        "OBJETIVO ALCANCADO! PHI=%.4f", proof["total"]
                    )
            except Exception as exc:  # pragma: no cover - fluxo de erro
                logger.error("Erro na prova: %s", exc)

    # ------------------------------------------------------------------ #
    # Estado
    # ------------------------------------------------------------------ #
    def get_stats(self) -> Dict:
        return {
            **self.stats,
            "achieved": self.chain._achieved,
            "total_coherence": self.chain.get_total_coherence(),
            "z1t_simulating": self.z1t.is_simulating,
            "undulator": self.undulator.stats(),
        }

    def save_checkpoint(self, path: str) -> None:
        self.chain.save(path)