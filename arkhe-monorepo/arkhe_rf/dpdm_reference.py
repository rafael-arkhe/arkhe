"""
DPDM reference — fóton escuro como teste de referência retrocausal.

Mapeia a massa do dark photon (DPDM, arXiv:2510.13956) para a frequência de
oscilação `f = m·c²/h` e gera um sinal sintético com a assinatura de
saturação não linear: geração de harmónicos e eficiência de conversão
ressonante degradada por δn/n. O sinal alimenta o pipeline RF/ELF como
benchmark controlado — se o pipeline marcar esses harmónicos como anomalia,
o modelo DPDM fornece a explicação física.

Para m = 1e-6 eV a frequência é ~241 MHz, banda VHF; o 8º harmónico
(~1,93 GHz) cai na faixa operacional do AntSDR.
"""

import numpy as np
from scipy.constants import c, e, epsilon_0, h

__all__ = ["DPDMReference"]

_MELECTRON_KG = 9.109e-31


class DPDMReference:
    """
    Gera sinais de referência baseados na física de fótons escuros (DPDM).

    Args:
        mass_ev: Massa do dark photon em eV.
    """

    def __init__(self, mass_ev: float = 1e-6):
        self.mass_ev = float(mass_ev)
        self.mass_kg = self.mass_ev * e / c**2
        self.frequency_hz = float(self.mass_kg * c**2 / h)
        self.period_s = float(1.0 / self.frequency_hz)

    def plasma_frequency(self, density_cm3: float = 1e6) -> float:
        """
        Frequência de plasma ω_p = sqrt(n·e²/(ε₀·m_e)).

        Args:
            density_cm3: Densidade eletrônica em cm⁻³.
        """
        n_m3 = float(density_cm3) * 1e6
        omega_p = np.sqrt(n_m3 * e**2 / (epsilon_0 * _MELECTRON_KG))
        return float(omega_p / (2 * np.pi))

    def conversion_efficiency(self, density_perturb: float = 0.1) -> float:
        """
        Eficiência de conversão ressonante com saturação não linear.

        Quanto maior δn/n, menor a eficiência — a ressonância ω = ω_p é
        quebrada pelas cavidades de densidade induzidas pela força
        ponderomotriz.
        """
        return float(1.0 / (1.0 + abs(float(density_perturb)) * 10.0))

    def generate_signal(
        self,
        duration_s: float = 10.0,
        fs: float = 250.0,
        density_perturb: float = 0.1,
    ) -> dict:
        """
        Sintetiza o sinal de conversão DPDM → oscilação + harmónicos.

        Args:
            duration_s: Duração da janela (s).
            fs: Taxa de amostragem (Hz).
            density_perturb: Inomogeneidade de densidade δn/n aplicada.

        Returns:
            Dicionário com `time`, `signal`, `frequency`, eficiência de
            conversão, `density_perturb` e flag `is_saturated`.
        """
        n = int(round(float(duration_s) * float(fs)))
        t = np.linspace(0.0, float(duration_s), n, endpoint=False)
        base = np.sin(2 * np.pi * self.frequency_hz * t)

        harmonics = np.zeros(n)
        for h in range(2, 6):
            eff = self.conversion_efficiency(density_perturb) / h
            harmonics += eff * np.sin(2 * np.pi * self.frequency_hz * h * t)

        signal = base + harmonics
        peak = float(np.max(np.abs(signal)))
        if peak > 0.0:
            signal /= peak

        return {
            "time": t,
            "signal": signal,
            "frequency": self.frequency_hz,
            "conversion_efficiency": self.conversion_efficiency(density_perturb),
            "density_perturb": float(density_perturb),
            "is_saturated": self.conversion_efficiency(density_perturb) < 0.5,
        }


if __name__ == "__main__":
    dpdm = DPDMReference(mass_ev=1e-6)
    print(f"Massa: {dpdm.mass_ev} eV -> Frequência: {dpdm.frequency_hz / 1e6:.2f} MHz")
    sig = dpdm.generate_signal(duration_s=1.0)
    print(f"Eficiência de conversão: {sig['conversion_efficiency']:.2f}")
    print(f"Saturado: {sig['is_saturated']}")
