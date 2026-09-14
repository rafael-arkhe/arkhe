#!/usr/bin/env python3
"""
Substrato 237 — Plasma Railgun (Modelo Snowplow)

Modelo simplificado de propulsor de plasma (acelerador de trilho) usando o
modelo "snowplow": a pressão magnética (força de Lorentz) empurra o plasma.

Modelo físico (auditoria honesta):
  - Energia armazenada no capacitor: E = 0.5 * C * V^2
  - Indutância incremental por unidade de comprimento L' (H/m) do trilho.
  - Força sobre a armadura de plasma: F = 0.5 * L' * I^2
  - Corrente do banco de capacitores aproximada por RLC subamortecido de pico:
        I_peak ≈ V / sqrt(L_total / C)  (regime subamortecido, sem resistência)
  - Velocidade final por conservação: v = sqrt(2 * E_kin / m), assumindo
    conversão de uma fração da energia magnética em energia cinética.

Não é um simulador MHD completo; fornece estimativas escalares coerentes com
ordens de grandeza plausíveis para fins de integração/orquestração.
"""

import numpy as np
from typing import Dict, Optional


class PlasmaRailgunSimulator:
    """Simulador de railgun de plasma (modelo snowplow escalar)."""

    def __init__(self, inductance_per_m: float = 0.5e-6,
                 rail_length_m: float = 1.0, efficiency: float = 0.5,
                 plasma_mass_kg: float = 1e-3):
        self.Lprime = inductance_per_m   # H/m
        self.rail_length = rail_length_m
        self.efficiency = efficiency
        self.mass = plasma_mass_kg
        self.shots: list = []

    def fire_shot(self, voltage: float = 10e3, capacitance: float = 100e-6,
                  mass: Optional[float] = None,
                  record: bool = True) -> Dict:
        """Dispara um shot e retorna métricas físicas."""
        m = mass if mass is not None else self.mass
        V = voltage
        C = capacitance

        energy = 0.5 * C * V * V
        # Indutância total do trilho = L' * comprimento
        L_total = self.Lprime * self.rail_length
        # Corrente de pico (regime subamortecido)
        current = V / np.sqrt(L_total / C) if (L_total > 0 and C > 0) else V

        # Força de Lorentz: F = 0.5 * L' * I^2
        force = 0.5 * self.Lprime * current * current
        # Trabalho sobre a distância do trilho
        work = force * self.rail_length
        # Energia cinética = eficiência * trabalho magnético
        kin_energy = self.efficiency * work
        velocity = np.sqrt(2 * kin_energy / m) if m > 0 else 0.0

        result = {
            "voltage": V,
            "capacitance": C,
            "mass": m,
            "energy_J": float(energy),
            "peak_current_A": float(current),
            "force_N": float(force),
            "kinetic_energy_J": float(kin_energy),
            "velocity_m_s": float(velocity),
            "velocity_km_s": float(velocity / 1000.0),
        }

        if record:
            self.shots.append(result)
        return result

    def best_velocity(self) -> float:
        """Maior velocidade (m/s) entre os shots registrados."""
        if not self.shots:
            return 0.0
        return max(s["velocity_m_s"] for s in self.shots)


if __name__ == "__main__":
    sim = PlasmaRailgunSimulator()
    r = sim.fire_shot()
    print(f"Railgun: E={r['energy_J']:.1f}J I={r['peak_current_A']:.0f}A "
          f"v={r['velocity_km_s']:.2f} km/s")
