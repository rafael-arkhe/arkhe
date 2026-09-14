"""
Interface de captura AntSDR (Ettus/B200-class) via SoapySDR.

Backends:
  - `real`  : SoapySDR stream → IQ (requer soapysdr + driver AntSDR instalados).
  - `sim`   : Síntese numérica com passagem LEO determinística — o fallback
              usado até o hardware chegar e para testes/bateria de detecção.

Modo de uso:
    with AntSDRCapture(center_freq=1992.5e6, sample_rate=20e6,
                       duration=0.5) as sdr:
        iq = sdr.capture()
        feat = sdr.capture_features()
"""

import json
import time
from pathlib import Path

import numpy as np

from . import config as _config
from .features import extract_features as _extract_features


_LEO_SAT_TONE_OFFSETS_HZ = (0.0, +38.0e3, -52.0e3)   # cluster OFDM-like
_LEO_SAT_TONE_DB = (-90.0, -100.0, -100.0)             # centro domina
_LEO_TERR_TONE_OFFSETS_HZ = (+120.0e3, -180.0e3)       # interferência fixa


def _amp_for_peak_db(peak_db: float, fft_size: int) -> float:
    """Amplitude tonal que produz um pico de PSD ~peak_db (fft normalizada)."""
    return float(10 ** ((peak_db - 10 * np.log10(fft_size * 0.667)) / 20.0))


class _SimLeoPass:
    """Gerador determinístico de IQ com passagem LEO sintética."""

    def __init__(
        self,
        sample_rate: float,
        fft_size: int,
        pass_offset_s: float | None = None,
        seed: int | None = None,
    ):
        self.sample_rate = sample_rate
        self.rng = np.random.default_rng(seed)
        self.elapsed_s = 0.0
        # Início num ponto do S-curve com slope mensurável (~0.85 kHz/s):
        # entra na rampa ascendente logo antes do zênite. Para demo confiável.
        if pass_offset_s is None:
            pass_offset_s = (
                _config.SIM_PASS_TOTAL_S / 2.0 - 0.4 * _config.SIM_PASS_TAU_S
            )
        self.pass_offset_s = float(pass_offset_s)

        self.sat_amps = [
            _amp_for_peak_db(db, fft_size) for db in _LEO_SAT_TONE_DB
        ]
        self.terr_amps = [
            _amp_for_peak_db(_config.SIM_TERRESTRIAL_POWER_DBF, fft_size)
            for _ in _LEO_TERR_TONE_OFFSETS_HZ
        ]
        self.noise_std = float(10 ** (_config.SIM_NOISE_FLOOR_DBF / 20.0))

    def doppler_at(self, t: float) -> float:
        """Desvio Doppler da componente radial no instante t (S-curve)."""
        tau = _config.SIM_PASS_TAU_S
        dmax = _config.SIM_PASS_DOPPLER_MAX_HZ
        return float(dmax * np.tanh((t - _config.SIM_PASS_TOTAL_S / 2.0) / tau))

    def synthesize(self, n_samples: int) -> np.ndarray:
        t_global = self.pass_offset_s + self.elapsed_s
        self.elapsed_s += n_samples / self.sample_rate

        n = int(n_samples)
        tt = np.arange(n) / self.sample_rate
        iq = np.zeros(n, dtype=np.complex128)

        # Satélite: cluster de tons seguindo o S-curve Doppler.
        fd = self.doppler_at(t_global)
        for off, amp in zip(_LEO_SAT_TONE_OFFSETS_HZ, self.sat_amps):
            phase = self.rng.uniform(0, 2 * np.pi)
            iq += amp * np.exp(1j * (2 * np.pi * (fd + off) * tt + phase))

        # Terrestres: tons fixos (não seguem Doppler).
        for off, amp in zip(_LEO_TERR_TONE_OFFSETS_HZ, self.terr_amps):
            phase = self.rng.uniform(0, 2 * np.pi)
            iq += amp * np.exp(1j * (2 * np.pi * off * tt + phase))

        iq += self.noise_std * (
            self.rng.standard_normal(n) + 1j * self.rng.standard_normal(n)
        )
        return iq.astype(np.complex64)[: int(n_samples)]

    @property
    def time_s(self) -> float:
        return self.pass_offset_s + self.elapsed_s


class AntSDRCapture:
    """
    Contexto de captura IQ. Tenta SoapySDR; sem driver, opera em simulação.

    Propriedades gráficas:
        ``center_freq`` / ``bandwidth`` podem ser alteradas entre capturas
        (o protocolo de caça rotaciona os alvos re-sintonizando suavemente).
    """

    def __init__(
        self,
        center_freq: float,
        sample_rate: float | None = None,
        bandwidth: float | None = None,
        duration: float = 1.0,
        driver: str | None = None,
        sim_mode: bool | None = None,
        seed: int | None = None,
        sim_pass_offset_s: float | None = None,
    ):
        self.center_freq = float(center_freq)
        self.sample_rate = float(
            sample_rate if sample_rate is not None else _config.SAMPLE_RATE_DEFAULT
        )
        self.bandwidth = float(
            bandwidth if bandwidth is not None else self.sample_rate
        )
        self.duration = float(duration)
        self.driver = driver or _config.SOAPY_DRIVER
        self.seed = seed
        self.mode = "sim"

        if sim_mode is None:
            sim_mode = _config.SIMULATION_MODE

        self._sdr = None
        self._n_samples = int(round(self.sample_rate * self.duration))
        self._sim = _SimLeoPass(
            self.sample_rate,
            int(_config.FFT_SIZE),
            pass_offset_s=sim_pass_offset_s,
            seed=seed,
        )

        if not sim_mode:
            self._open_soapy(required=True)

    # ------------------------------------------------------------------
    # SoapySDR (opcional; ausente nesta máquina → sempre modo simulação)
    # ------------------------------------------------------------------
    def _open_soapy(self, required: bool = False) -> bool:
        try:
            from SoapySDR import Device, DeviceError  # type: ignore

            args = self.driver.split(",")
            kwargs = dict(a.split("=", 1) for a in args if "=" in a)
            self._sdr = Device(kwargs)
            self._sdr.setSampleRate("RX", self.sample_rate)
            self._sdr.setFrequency("RX", self.center_freq)
            self._sdr.setBandwidth("RX", self.bandwidth)
            self.mode = "soapy"
            return True
        except Exception as exc:  # noqa: BLE001 — driver opcional
            if required:
                raise RuntimeError(
                    f"SoapySDR indisponível com driver '{self.driver}': {exc}"
                ) from exc
            return False

    # ------------------------------------------------------------------
    # API pública
    # ------------------------------------------------------------------
    def capture(self) -> np.ndarray:
        """Captura um frame de `duration` segundos → array complexo (samples)."""
        n = self._n_samples
        if self._sdr is not None:
            buf = np.zeros(n, dtype=np.complex64)
            rx = self._sdr.setupStream("RX", "CF32")
            self._sdr.activateStream(rx)
            read = 0
            while read < n:
                chunk = min(4096, n - read)
                got = self._sdr.readStream(rx, [buf[read : read + chunk]], chunk)[0]
                if got < 0:
                    continue  # fluxo não pronto — re-tenta
                read += got
            self._sdr.deactivateStream(rx)
            self._sdr.closeStream(rx)
            return buf
        return self._sim.synthesize(n)

    def capture_features(
        self,
        _iq: np.ndarray | None = None,
    ) -> dict:
        """Captura IQ e extrai features (frequências absolutas ancoradas)."""
        iq = self.capture() if _iq is None else _iq
        return _extract_features(
            iq,
            sample_rate=self.sample_rate,
            center_freq=self.center_freq,
        )

    def snapshot(
        self,
        iq: np.ndarray,
        features: dict,
        label: str,
        extra: dict | None = None,
    ) -> dict:
        """Salva IQ + metadados de detecção em `data/detections`."""
        out_dir = Path(_config.DETECTIONS_DIR)
        out_dir.mkdir(parents=True, exist_ok=True)
        ts = time.strftime("%Y%m%d_%H%M%S")
        path = out_dir / f"{label}_{ts}.npy"
        meta = {
            "label": label,
            "center_freq_hz": self.center_freq,
            "sample_rate_hz": self.sample_rate,
            "bandwidth_hz": self.bandwidth,
            "duration_s": self.duration,
            "timestamp_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
            "mode": self.mode,
            "n_samples": int(iq.shape[0]),
        }
        meta.update(features)
        if extra:
            meta.update(extra)
        np.save(path, iq)
        meta_path = path.with_suffix(".json")
        meta.setdefault("npy_file", str(path))
        with open(meta_path, "w", encoding="utf-8") as fh:
            json.dump(meta, fh, indent=2, ensure_ascii=False)
        return {"npy_file": str(path), "meta_file": str(meta_path), "meta": meta}

    def describe(self) -> dict:
        """Informações do backend ativo (para logs e relatórios)."""
        return {
            "mode": self.mode,
            "center_freq_hz": self.center_freq,
            "sample_rate_hz": self.sample_rate,
            "bandwidth_hz": self.bandwidth,
            "duration_s": self.duration,
            "n_samples_per_frame": self._n_samples,
            "driver": self.driver if self._sdr is not None else None,
            "sim_time_s": round(self._sim.time_s, 3),
        }

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc, tb):
        self.close()
        return False

    def close(self) -> None:
        if self._sdr is not None:
            try:
                self._sdr = None
            except Exception:  # noqa: BLE001
                pass


if __name__ == "__main__":
    # Sanidade rápida do modo simulação.
    with AntSDRCapture(
        center_freq=1992.5e6,
        sample_rate=1e6,
        duration=0.1,
        seed=7,
    ) as sdr:
        print("backend:", sdr.describe())
        feat = sdr.capture_features()
        print(
            f"SNR={feat['snr_db']:.1f} dB  peaks={feat['num_peaks']} "
            f"offset_rel={feat['peak_freq_offset']/1e3:+.1f} kHz  "
            f"abs={feat['peak_freq']/1e6:.3f} MHz"
        )