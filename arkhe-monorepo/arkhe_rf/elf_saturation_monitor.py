"""
Monitor de saturação ionosférica (DPDM — arXiv:2510.13956).

Detecta a assinatura não linear da conversão ressonante na cavidade
Terra-ionosfera: quando o bombeamento ressonante satura o plasma, a energia
em excesso é transferida para os modos superiores de cavidade (eigenmodes
de Schumann) e a razão entre o fundamental de 7,83 Hz e esses modos cresce —
essa razão é a métrica de saturação observável pelo loop ELF.

O monitor é agnóstico de backend: recebe qualquer bloco de sinal ELF (vindo
de `ELFLoop.capture` ou de um ADC serial) e devolve um dicionário com a
pontuação de saturação, atualizando um histórico de `window_size` janelas.
"""

import numpy as np

from . import config as _config

__all__ = ["IonosphericSaturationMonitor"]


class IonosphericSaturationMonitor:
    """
    Detecta saturação da ionosfera pela amplitude da ressonância Schumann.

    Args:
        threshold_amp: Amplitude de referência do fundamental (V) — normaliza
            a pontuação para ser independente do ganho do front-end.
        window_size: Número de janelas mantidas no histórico.
    """

    def __init__(self, threshold_amp: float = 1.0, window_size: int = 10):
        self.threshold_amp = float(threshold_amp)
        self.window_size = int(window_size)
        self.history: list[dict] = []

    def check_saturation(
        self,
        elf_signal: np.ndarray,
        fs: float = _config.ELF_SAMPLE_RATE_HZ,
    ) -> dict:
        """
        Pontua a saturação de um bloco ELF.

        score = (amp_fundamental / threshold) * (1 + razão de modos).

        Uma razão de modos alta (fundamental com energia vazando para os
        modos superiores) indica alargamento espectral por saturação não
        linear — o canal retrocausal pode estar mascarado.
        """
        signal = np.asarray(elf_signal, dtype=np.float64).reshape(-1)
        n = len(signal)
        if n < 2:
            return {
                "amp_fundamental": 0.0,
                "eigenmode_amps": {f"{f:.1f}": 0.0 for f in _config.SCHUMANN_EIGENMODES_HZ},
                "harmonic_ratio": 0.0,
                "saturation_score": 0.0,
                "saturated": False,
            }

        spec = np.fft.rfft(signal)
        mag = np.abs(spec) / max(n, 1)  # amplitude por amostra (V)
        freqs = np.fft.rfftfreq(n, 1.0 / float(fs))

        def amp_at(freq_hz: float) -> float:
            idx = int(np.argmin(np.abs(freqs - freq_hz)))
            return float(mag[idx])

        amp_fund = amp_at(_config.SCHUMANN_FUNDAMENTAL_HZ)
        mode_amps = [amp_at(f) for f in _config.SCHUMANN_EIGENMODES_HZ]
        harmonic_ratio = float(np.sum(mode_amps) / (amp_fund + 1e-12))
        saturation_score = float(
            (amp_fund / (self.threshold_amp + 1e-12)) * (1.0 + harmonic_ratio)
        )
        saturated = bool(saturation_score > 2.0)

        entry = {
            "amp_fundamental": amp_fund,
            "eigenmode_amps": dict(
                zip([f"{f:.1f}" for f in _config.SCHUMANN_EIGENMODES_HZ], mode_amps)
            ),
            "harmonic_ratio": harmonic_ratio,
            "saturation_score": saturation_score,
            "saturated": saturated,
        }
        self.history.append(entry)
        if len(self.history) > self.window_size:
            self.history.pop(0)
        return entry


if __name__ == "__main__":
    from .elf_loop import ELFLoop

    monitor = IonosphericSaturationMonitor()
    with ELFLoop(sample_rate=250.0, seed=1) as loop:
        sig = loop.capture(duration=10.0)
        res = monitor.check_saturation(sig)
        print(f"score={res['saturation_score']:.3f} "
              f"harm={res['harmonic_ratio']:.3f} "
              f"saturated={res['saturated']}")
