"""
Loop ELF passivo — ressonâncias de Schumann (7,83 Hz e modos de cavidade).

Backends:
  - `serial`: ADC 24-bit (ex.: ADS1256) lendo linhas de tensão numérica
             via UART. É o caminho de hardware.
  - `sim`   : síntese determinística e filtrada da banda 5–35 Hz — usada
             até o loop físico estar montado e para teste de integração.

Nota física: 14,3/20,8/27,3/33,8 Hz são eigenfrequências da cavidade
Terra-ionosfera (razões ≈ 1,83/2,66/3,49/4,32), não harmónicos inteiros.
A simulação usa os valores reais das eigenfrequências com amplitudes
decaindo por ordem; é aproximação, não física exata da cavidade.

O sinal de saída sempre passa pelo filtro passa-banda Butterworth 5–35 Hz.
As features de Schumann (20-dim) alimentam o autoencoder multi-modal.
"""

import numpy as np
from scipy.signal import butter, filtfilt

from . import config as _config

__all__ = ["ELFLoop", "schumann_feature_vector_size"]


def schumann_feature_vector_size() -> int:
    """Dimensão do vetor de features ELF (invariante para o autoencoder)."""
    return _config.ELF_FEATURE_DIM


class _SimSchumannSource:
    """Síntese determinística de ressonâncias de Schumann degradadas."""

    def __init__(self, sample_rate: float, seed: int | None = None):
        self.fs = float(sample_rate)
        self.rng = np.random.default_rng(seed)

    def synthesize(self, n_samples: int, phase_offset: float = 0.0) -> np.ndarray:
        n = int(n_samples)
        tt = np.arange(n) / self.fs
        sig = np.zeros(n)
        # Harmónicos com decaimento 1/h; envelope lento (ionosfera).
        env = 1.0 + 0.05 * np.sin(2 * np.pi * tt / 600.0)
        for h, f in enumerate(_config.SCHUMANN_FREQS_HZ, start=1):
            amp = 1.0 / h
            phi = self.rng.uniform(0, 2 * np.pi)
            sig += amp * np.sin(2 * np.pi * f * tt + phi + phase_offset)
        sig *= env
        sig += self.rng.standard_normal(n) * 0.05       # ruído ionosférico
        sig += 0.02 * np.sin(2 * np.pi * tt / 120.0)    # deriva lenta
        return sig

    @property
    def seed(self):
        return self.rng


class ELFLoop:
    """
    Receptor passivo da banda ELF com resposta Schumann.

    Args:
        port: Porta serial (ex.: "COM3", "/dev/ttyUSB0"). Vazia = simulação.
        sample_rate: Taxa efetiva do ADC em Hz.
        band: Banda passa-banda (Hz).
        baud: Baudrate do ADC.
        seed: Semente do gerador no modo simulação.
    """

    def __init__(
        self,
        port: str = "",
        sample_rate: float = _config.ELF_SAMPLE_RATE_HZ,
        band: tuple[float, float] = _config.ELF_BAND_HZ,
        baud: int = _config.ELF_BAUD,
        seed: int | None = None,
    ):
        self.port = port or _config.ELF_PORT
        self.fs = float(sample_rate)
        self.band = band
        self.baud = baud
        self.mode = "sim"
        self._ser = None
        self._sim = _SimSchumannSource(self.fs, seed=seed)
        self._b, self._a = butter(4, list(band), btype="band", fs=self.fs)
        self.last_timestamps: list[float] = []

        if self.port:
            self._open_serial(required=True)

    # ------------------------------------------------------------------
    def _open_serial(self, required: bool = False) -> bool:
        try:
            import serial  # type: ignore

            self._ser = serial.Serial(self.port, self.baud, timeout=1)
            self.mode = "serial"
            return True
        except Exception as exc:  # noqa: BLE001 — backend opcional
            if required:
                raise RuntimeError(
                    f"ADC serial indisponível em '{self.port}': {exc}"
                ) from exc
            return False

    # ------------------------------------------------------------------
    def capture(
        self,
        duration: float = _config.ELF_DEFAULT_DURATION_S,
        gps_time: float | None = None,
        phase_offset: float = 0.0,
    ) -> np.ndarray:
        """
        Captura `duration` segundos da banda ELF (sinal filtrado).

        Args:
            duration: Janela de captura em segundos.
            gps_time: Timestamp Unix (epoch) absoluto do início do bloco;
                      anota `last_timestamps` quando fornecido.
            phase_offset: Fase adicional (V) — útil para simular deriva.

        Returns:
            Sinal flutuante (volts, filtrado 5–35 Hz), comprimento N.
        """
        n = int(round(duration * self.fs))
        ts_start = gps_time if gps_time is not None else getattr(self, "_t_sim", 0.0)
        if self._ser is not None:
            raw = np.zeros(n)
            for i in range(n):
                line = self._ser.readline().decode(errors="replace").strip()
                try:
                    raw[i] = float(line)
                except ValueError:
                    continue  # linha inválida → amostra zero
            signal = filtfilt(self._b, self._a, raw)
        else:
            signal = filtfilt(
                self._b, self._a, self._sim.synthesize(n, phase_offset)
            )
            self._t_sim = ts_start + n / self.fs

        self.last_timestamps = [ts_start, ts_start + n / self.fs]
        return signal

    # ------------------------------------------------------------------
    def extract_schumann_features(
        self,
        signal: np.ndarray,
    ) -> dict:
        """
        Extrai amplitude, fase e harmónicos das ressonâncias de Schumann.

        Retorna `feature_vector` de 20 dimensões:
          [amp_7.83, amp_14.3, amp_20.8, amp_27.3, amp_33.8,
           phase_7.83..phase_33.8,
           snr_est, crest_factor, bandpow_low, bandpow_high,
           freq_delta_7, fidelity_ratio, std_norm, prominence_7,
           phase_slope, peak_width_norm]
        """
        n = len(signal)
        spec = np.fft.rfft(signal)
        mag = np.abs(spec) / max(n, 1)
        freqs = np.fft.rfftfreq(n, 1.0 / self.fs)

        amps, phases, idxs = [], [], []
        for f_h in _config.SCHUMANN_FREQS_HZ:
            idx = int(np.argmin(np.abs(freqs - f_h)))
            idxs.append(idx)
            amps.append(float(mag[idx]))
            phases.append(float(np.angle(spec[idx])))

        # Frequência estimada do fundamental pelo bin de maior magnitude.
        peak = np.argmax(mag) if n else 0
        f_est = float(freqs[peak])
        freq_delta_7 = float(f_est - _config.SCHUMANN_FUNDAMENTAL_HZ)

        amp_7 = amps[0]
        fidelity_ratio = float(
            np.mean([amps[h] / (amp_7 + 1e-12) for h in range(1, len(amps))])
        )
        crest_factor = float(np.max(np.abs(signal)) / (np.std(signal) + 1e-12))
        bandpow_low = float(np.mean(mag[1:int(n * 10 / self.fs)]))   # 0–10 Hz
        bandpow_high = float(np.mean(mag[int(n * 10 / self.fs):]))   # 10–35 Hz
        std_norm = float(np.std(signal) / (amp_7 + 1e-12))
        prominence_7 = float(
            mag[idxs[0]] - 0.5 * (mag[max(0, idxs[0] - 8)] + mag[min(len(mag) - 1, idxs[0] + 8)])
        )
        phase_slope = float(
            (phases[4] - phases[0]) / max(np.pi, 1.0)  # envoltória de fase
        )
        # Largura (~6 dB) do pico fundamental em Hz.
        half = mag[idxs[0]] / 2.0
        k = 0
        while (
            idxs[0] - k - 1 >= 0 and mag[idxs[0] - k - 1] > half and k < 40
        ):
            k += 1
        peak_width_hz = float(
            (2 * k + 1) * abs(freqs[1] - freqs[0]) if len(freqs) > 1 else 0.0
        )

        scale = max(1.0, amp_7)
        feature_vector = np.array(
            [
                *[a / scale for a in amps],          # 5
                *[p / np.pi for p in phases],        # 5
                amp_7 / (np.std(signal) + 1e-12),    # snr_est
                crest_factor,
                bandpow_low / (amp_7 + 1e-12),
                bandpow_high / (amp_7 + 1e-12),
                freq_delta_7,
                fidelity_ratio,
                std_norm,
                prominence_7 / (amp_7 + 1e-12),
                phase_slope,
                peak_width_hz / max(1.0, freq_delta_7 + 1e-6),  # 20º
            ],
            dtype=np.float64,
        )
        assert feature_vector.shape[0] == _config.ELF_FEATURE_DIM

        return {
            "amp_fundamental": amp_7,
            "phase_fundamental": phases[0],
            "amplitudes": dict(zip([f"{f:.1f}" for f in _config.SCHUMANN_FREQS_HZ], amps)),
            "phases": dict(zip([f"{f:.1f}" for f in _config.SCHUMANN_FREQS_HZ], phases)),
            "harmonics": {
                f_h: a for f_h, a in zip(_config.SCHUMANN_HARMONICS, amps[1:])
            },
            "snr_est": float(amp_7 / (np.std(signal) + 1e-12)),
            "frequency_est_hz": f_est,
            "freq_delta_7_hz": freq_delta_7,
            "band_power": {"low_db": bandpow_low, "high_db": bandpow_high},
            "feature_vector": feature_vector,
        }

    # ------------------------------------------------------------------
    def describe(self) -> dict:
        """Informações do backend ativo."""
        return {
            "mode": self.mode,
            "sample_rate_hz": self.fs,
            "band_hz": list(self.band),
            "port": self.port or None,
            "feature_dim": schumann_feature_vector_size(),
        }

    def close(self) -> None:
        if self._ser is not None:
            try:
                self._ser.close()
            except Exception:  # noqa: BLE001
                pass
            self._ser = None

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc, tb):
        self.close()
        return False


if __name__ == "__main__":
    with ELFLoop(seed=1) as loop:
        print("backend:", loop.describe())
        sig = loop.capture(duration=10.0)
        feats = loop.extract_schumann_features(sig)
        print(f"fundamental: {feats['amp_fundamental']:.4f} V @ "
              f"{feats['frequency_est_hz']:.2f} Hz | snr={feats['snr_est']:.1f} "
              f"| harmónicos: {[round(a,3) for a in feats['amplitudes'].values()]}")
        print(f"feature_vector dim: {feats['feature_vector'].shape}")