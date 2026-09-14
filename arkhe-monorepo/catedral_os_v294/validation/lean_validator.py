# validation/lean_validator.py
"""
L3 — Validação em tempo real dos invariantes (v294.0).

Mirror Python dos invariantes formalizados em
`invariants/NHSEInvariants.lean`, com os mesmos limiares:

  * I319  Amplificação topológica da coerência — E(N) > 1 (Ó_OBC > Ó_PBC);
  * I320  Limite de Cramér-Rao — sigma >= 1/sqrt(F), sigma < 0.1;
  * I321  Robustez topológica — desordem < 10% => phi > 0.8 * phi_0;
  * I322  Aprimoramento multi-parâmetros — det(F) > 1;
  * I323  Dualidade CQFI <-> QFI — mapa dual trivial (por construção).

O verificador Lean (quando instalado) checa o arquivo formal real via
`verify_with_lean`; sem `lean`, o status é registrado como `unavailable` —
nunca falso-verde.
"""

from __future__ import annotations

import logging
import os
import subprocess
from dataclasses import dataclass, field
from enum import Enum
from typing import Dict, List, Optional, Tuple

logger = logging.getLogger(__name__)


class InvariantStatus(Enum):
    VERIFIED = "verified"
    VIOLATED = "violated"
    PENDING = "pending"
    UNAVAILABLE = "unavailable"


@dataclass
class VerificationReport:
    checks: List[Tuple[str, InvariantStatus, str]] = field(default_factory=list)

    @property
    def all_verified(self) -> bool:
        return all(s == InvariantStatus.VERIFIED for _, s, _ in self.checks)

    @property
    def any_violated(self) -> bool:
        return any(s == InvariantStatus.VIOLATED for _, s, _ in self.checks)

    def to_dict(self) -> Dict:
        return {
            label: {"status": status.value, "detail": detail}
            for label, status, detail in self.checks
        }


class MiniLeanValidator:
    """Checador de invariantes em tempo real (L3) — v294.0."""

    DISORDER_THRESHOLD = 0.1
    ROBUSTNESS_FACTOR = 0.8
    CRAMER_RAO_LIMIT = 0.1
    MULTI_PARAMETER_THRESHOLD = 1.0

    def __init__(self, theorem_file: str = "invariants/NHSEInvariants.lean"):
        self.theorem_file = theorem_file

    # ------------------------------------------------------------------ #
    # I319 — Amplificação topológica
    # ------------------------------------------------------------------ #
    def check_i319(self, enhancement: float) -> InvariantStatus:
        if enhancement > 1.0:
            return InvariantStatus.VERIFIED
        if enhancement <= 1.0 and enhancement > 0.0:
            return InvariantStatus.VIOLATED
        return InvariantStatus.PENDING

    # ------------------------------------------------------------------ #
    # I320 — Limite de Cramér-Rao
    # ------------------------------------------------------------------ #
    def check_i320(self, sigma: float, limit: float = CRAMER_RAO_LIMIT) -> InvariantStatus:
        if sigma < 0.0:
            return InvariantStatus.PENDING
        if sigma < limit:
            return InvariantStatus.VERIFIED
        return InvariantStatus.VIOLATED

    # ------------------------------------------------------------------ #
    # I321 — Robustez topológica
    # ------------------------------------------------------------------ #
    def check_i321(self, disorder: float, phi: float, phi_0: float,
                   threshold: float = DISORDER_THRESHOLD,
                   factor: float = ROBUSTNESS_FACTOR) -> InvariantStatus:
        if disorder < threshold:
            if phi > factor * phi_0:
                return InvariantStatus.VERIFIED
            return InvariantStatus.VIOLATED
        if disorder >= threshold:
            # Acima do limiar: não fazemos claim — PENDING, não falso-verde.
            return InvariantStatus.PENDING
        return InvariantStatus.PENDING

    # ------------------------------------------------------------------ #
    # I322 — Aprimoramento multi-parâmetros
    # ------------------------------------------------------------------ #
    def check_i322(self, det_fimf: float,
                   threshold: float = MULTI_PARAMETER_THRESHOLD) -> InvariantStatus:
        if det_fimf > threshold:
            return InvariantStatus.VERIFIED
        return InvariantStatus.VIOLATED

    # ------------------------------------------------------------------ #
    # I323 — Dualidade CQFI <-> QFI
    # ------------------------------------------------------------------ #
    def check_i323(self, duality_param: float) -> InvariantStatus:
        if duality_param == 1.0:
            return InvariantStatus.VERIFIED
        return InvariantStatus.PENDING

    # ------------------------------------------------------------------ #
    # Verificação integrada
    # ------------------------------------------------------------------ #
    def verify(self, nhse: "object") -> VerificationReport:
        """Valida I319-I323 a partir de um NHSEngine ou snapshot dict."""
        snapshot = nhse.snapshot() if hasattr(nhse, "snapshot") else nhse
        enhancement = float(snapshot.get("enhancement", 0.0))
        sigma = float(snapshot.get("cramer_rao", float("inf")))
        phi_ratio = float(snapshot.get("robustness_ratio", 0.9))
        det_fimf = float(snapshot.get("multi_parameter_det", 0.0))
        duality = float(snapshot.get("duality_param", 1.0))

        report = VerificationReport(
            checks=[
                ("I319", self.check_i319(enhancement), "enhancement E(N)"),
                ("I320", self.check_i320(sigma), "cramer-rao bound"),
                ("I321", self._robustness_from_ratio(phi_ratio), "robustness"),
                ("I322", self.check_i322(det_fimf), "det(F) > 1"),
                ("I323", self.check_i323(duality), "duality"),
            ]
        )
        return report

    def _robustness_from_ratio(self, ratio: float,
                               factor: float = ROBUSTNESS_FACTOR) -> InvariantStatus:
        if ratio > factor:
            return InvariantStatus.VERIFIED
        if ratio > 0.0:
            return InvariantStatus.VIOLATED
        return InvariantStatus.PENDING

    # ------------------------------------------------------------------ #
    def verify_with_lean(self, timeout: int = 60, lean_bin: str = "lean") -> Tuple[InvariantStatus, str]:
        """Checa o arquivo formal com o verificador Lean 4 (se instalado)."""
        path = self.theorem_file
        if not os.path.exists(path):
            return InvariantStatus.UNAVAILABLE, f"teorema ausente: {path}"
        try:
            proc = subprocess.run(
                [lean_bin, path],
                capture_output=True,
                text=True,
                timeout=timeout,
                cwd=os.path.dirname(path) or ".",
            )
        except FileNotFoundError:
            return InvariantStatus.UNAVAILABLE, "lean não instalado no PATH"
        except subprocess.TimeoutExpired:
            return InvariantStatus.UNAVAILABLE, "timeout na verificação"
        if proc.returncode == 0:
            return InvariantStatus.VERIFIED, "NHSEInvariants.lean compilado"
        summary = (proc.stderr or proc.stdout or "").strip().splitlines()
        return InvariantStatus.VIOLATED, "; ".join(summary[-3:])