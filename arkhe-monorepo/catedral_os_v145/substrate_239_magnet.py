#!/usr/bin/env python3
"""
Substrato 239 — Kilotesla Magnet Generator (Flux Compression)

Modelo de gerador de campo magnético via compressão de fluxo. O princípio:
compressão rápida de um campo inicial capturado entre uma armadura condutora
e o estator conserva (aproximadamente) o fluxo Φ = B*A. Com a área caindo pelo
fator de compressão, o campo sobe.

    B_final ≈ B_inicial * compression_ratio          (conservação de fluxo ideal)
    Pressão magnética: P = B^2 / (2 * mu0)

NOTA DE HONESTIDADE (auditoria): na prática a compressão de fluxo tem perdas
(resistivas/difusivas) e atingir kilotesla (>1 kT) exige implosão de cilindros
com correntes multi-MA (máquinas como FC-1, VNIIEF). O modelo aqui é a
conservação de fluxo ideal sem perdas — limite teórico superior, útil para
ordens de grandeza e integração, não uma garantia de máquina real.
"""

import numpy as np
from typing import Dict, Optional

MU0 = 4e-7 * np.pi  # 4π×10⁻⁷ H/m


class KiloteslaMagnet:
    """Gerador de campo por compressão de fluxo (modelo escalar)."""

    def __init__(self):
        self.pulses: list = []

    def generate_pulse(self, initial_field: float = 10.0,
                       compression_ratio: float = 100.0,
                       record: bool = True) -> Dict:
        """
        Gera um pulso: campo final por conservação de fluxo e pressão magnética.
        """
        if compression_ratio <= 0:
            raise ValueError("compression_ratio deve ser > 0")

        final_field = initial_field * compression_ratio
        # pressão magnética P = B^2 / (2 mu0), em Pa
        pressure = (final_field ** 2) / (2.0 * MU0)

        result = {
            "initial_field_T": float(initial_field),
            "compression_ratio": float(compression_ratio),
            "final_field_T": float(final_field),
            "pressure_Pa": float(pressure),
            "pressure_GPa": float(pressure / 1e9),
        }

        if record:
            self.pulses.append(result)
        return result

    def best_field(self) -> float:
        """Maior campo (T) entre os pulsos registrados."""
        if not self.pulses:
            return 0.0
        return max(p["final_field_T"] for p in self.pulses)


if __name__ == "__main__":
    mg = KiloteslaMagnet()
    r = mg.generate_pulse()
    print(f"Magnet: B={r['final_field_T']/1000:.2f} kT "
          f"P={r['pressure_GPa']:.1f} GPa")
