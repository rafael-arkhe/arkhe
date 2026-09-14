"""
Extração de características espectrais — IQ → vetor de features (68-dim).

O pipeline opera em bandabase: o SDR entrega IQ centrado em `center_freq`,
então o espectro cobre [−fs/2, +fs/2] em torno do centro. As features são
devolvidas com unidades físicas (Hz, dB) para alimentar o rastreamento
Doppler e um vetor normalizado para classificação/ML.
"""

import numpy as np
from scipy.signal import find_peaks

from . import config as _config
from .doppler import estimate_leo_doppler_max


def compute_spectrum(
    iq: np.ndarray,
    sample_rate: float,
    fft_size: int | None = None,
    window: str = "hann",
) -> tuple[np.ndarray, np.ndarray]:
    """
    Calcula o PSD (em dB) de um bloco IQ complexo, double-sided e centrado.

    Args:
        iq: Amostras complexas (bandabase centrada em 0 Hz).
        sample_rate: Taxa de amostragem em Hz.
        fft_size: Tamanho da FFT por segmento (média de Welch manual).
        window: Janela aplicada a cada segmento ("hann" ou quadrada).

    Returns:
        (freqs, psd_db): eixos de frequência (Hz) e potência (dB).
    """
    iq = np.asarray(iq, dtype=np.complex128)
    if fft_size is None:
        fft_size = int(_config.FFT_SIZE)

    fft_size = int(min(fft_size, max(len(iq), 1)))
    n = len(iq)
    if n < fft_size:
        iq = np.pad(iq, (0, fft_size - n))
        n_seg = 1
    else:
        n_seg = n // fft_size
        iq = iq[: n_seg * fft_size]

    if window == "hann" and fft_size > 1:
        w = np.hanning(fft_size)
    else:
        w = np.ones(fft_size)

    psd = np.zeros(fft_size, dtype=np.float64)
    for k in range(n_seg):
        seg = iq[k * fft_size : (k + 1) * fft_size] * w
        spec = np.fft.fft(seg, n=fft_size)
        psd += np.abs(spec) ** 2

    # Normaliza por número de segmentos e pela potência da janela.
    psd = psd / (n_seg * np.sum(w**2) + 1e-30)

    freqs = np.fft.fftfreq(fft_size, 1.0 / sample_rate)
    psd = np.fft.fftshift(psd)
    freqs = np.fft.fftshift(freqs)
    psd_db = 10.0 * np.log10(psd + 1e-30)
    return freqs, psd_db


def _detect_peaks(
    freqs: np.ndarray,
    psd_db: np.ndarray,
    noise_floor_db: float,
    min_prominence_db: float = 6.0,
    min_separation_hz: float = 20e3,
) -> tuple[np.ndarray, np.ndarray]:
    """Picos acima do piso de ruído na PSD. Retorna (indices, absolutos Hz)."""
    bin_width = abs(freqs[1] - freqs[0]) if len(freqs) > 1 else 1.0
    distance = max(1, int(round(min_separation_hz / bin_width)))
    indices, props = find_peaks(
        psd_db,
        prominence=min_prominence_db,
        distance=distance,
    )
    above_floor = psd_db[indices] > noise_floor_db + min_prominence_db
    indices = indices[above_floor]
    return indices, freqs[indices]


def extract_features(
    iq: np.ndarray,
    sample_rate: float | None = None,
    center_freq: float = 0.0,
    fft_size: int | None = None,
    noise_percentile: float = 15.0,
    min_prominence_db: float = 6.0,
) -> dict:
    """
    Extrai features espectrais de um bloco IQ.

    Args:
        iq: Amostras complexas em bandabase centrada em `center_freq`.
        sample_rate: Taxa de amostragem (padrão: config.SAMPLE_RATE_DEFAULT).
        center_freq: Frequência de sintonia do SDR (Hz) — converte offsets
                     para frequências absolutas.
        fft_size: Tamanho da FFT.
        noise_percentile: Percentil da PSD usado como piso de ruído.
        min_prominence_db: Prominência mínima para considerar um pico.

    Returns:
        Dict com key físicas e o vetor de features de 68 dimensões:
        `feature_vector` = 64 bins de PSD média + [snr_db, num_peaks,
        peak_offset_norm, band_flatness].
    """
    if sample_rate is None:
        sample_rate = _config.SAMPLE_RATE_DEFAULT

    freqs, psd_db = compute_spectrum(iq, sample_rate, fft_size=fft_size)
    n_bins = len(freqs)

    noise_floor_db = float(np.percentile(psd_db, noise_percentile))

    idx, peak_freqs_rel = _detect_peaks(
        freqs, psd_db, noise_floor_db, min_prominence_db=min_prominence_db
    )

    peaks = []
    for fi in idx:
        peaks.append(float(freqs[fi] + center_freq))
    peaks.sort()

    i_main = int(np.argmax(psd_db))
    peak_freq_offset = float(freqs[i_main])
    peak_freq = float(peak_freq_offset + center_freq)
    peak_power_db = float(psd_db[i_main])
    num_peaks = len(peaks)

    snr_db = float(peak_power_db - noise_floor_db)

    # Potência por 8 sub-bandas lineares + total.
    n_sub = 8
    subband_idx = np.linspace(0, n_bins, n_sub + 1).astype(int)
    subbands_db = [
        float(np.mean(psd_db[subband_idx[i] : subband_idx[i + 1]]))
        for i in range(n_sub)
    ]
    band_flatness = float(np.std(subbands_db))

    # Vetor de features: 64 bins de PSD relativa ao piso + 4 estatísticas.
    coarse = np.array(
        [float(np.mean(psd_db[a:b])) for a, b in
         zip(np.linspace(0, n_bins, 65).astype(int)[:-1],
             np.linspace(0, n_bins, 65).astype(int)[1:])]
    )
    relative = coarse - noise_floor_db
    half_bw = sample_rate / 2.0
    peak_offset_norm = float(np.clip(peak_freq_offset / half_bw, -1.0, 1.0))
    stats = np.array(
        [snr_db, float(num_peaks), peak_offset_norm, band_flatness],
        dtype=np.float64,
    )
    feature_vector = np.concatenate([relative, stats])

    band_power = {
        "total_db": float(np.mean(psd_db)),
        "subbands_db": subbands_db,
        "band_flatness_db": band_flatness,
        "noise_floor_db": noise_floor_db,
        "peak_offset_hz": peak_freq_offset,
    }

    return {
        "snr_db": snr_db,
        "peak_freq": peak_freq,
        "peak_freq_offset": peak_freq_offset,
        "peak_power_db": peak_power_db,
        "noise_floor_db": noise_floor_db,
        "num_peaks": num_peaks,
        "all_peaks": peaks,
        "band_power": band_power,
        "feature_vector": feature_vector,
        "psd_db": psd_db,
        "freqs": freqs,
        "fft_size": n_bins,
    }


def feature_vector_size() -> int:
    """Dimensão do vetor de features (invariante para ML/algoritmos)."""
    return 68


def inject_synthetic_doppler(
    features: dict,
    doppler_hz: float,
    center_freq: float,
) -> dict:
    """
    Corrige as features absolutas de um frame para compensar Doppler.

    Útil para validar o rastreador Doppler com dados gravados:
    desloca `peak_freq`/`all_peaks` por meio de `doppler_hz` e recompõe
    o vetor de features de forma consistente.
    """
    from copy import deepcopy

    out = deepcopy(features)
    out["peak_freq"] -= doppler_hz
    out["peak_freq_offset"] -= doppler_hz
    out["all_peaks"] = [p - doppler_hz for p in out["all_peaks"]]
    hb = _config.SAMPLE_RATE_DEFAULT / 2.0
    out["feature_vector"][66] = float(
        np.clip(out["peak_freq_offset"] / hb, -1.0, 1.0)
    )
    out["band_power"]["peak_offset_hz"] = out["peak_freq_offset"]
    return out


if __name__ == "__main__":
    # Sanidade rápida: tom em +250 kHz → pico detectado em +250 kHz.
    sr = 10e6
    fs_hz = 250e3
    n = sr * 0.01
    t = np.arange(n) / sr
    iq = np.exp(2j * np.pi * fs_hz * t) * 0.5 + 0.01 * (
        np.random.randn(n) + 1j * np.random.randn(n)
    )
    feat = extract_features(iq, sample_rate=sr, center_freq=1992.5e6)
    print(f"SNR={feat['snr_db']:.1f} dB  pico rel={feat['peak_freq_offset']/1e3:.1f} kHz "
          f"abs={feat['peak_freq']/1e6:.1f} MHz  peaks={feat['num_peaks']} "
          f"feat_dim={len(feat['feature_vector'])}")

    est = estimate_leo_doppler_max(1992.5e6)
    print(f"Doppler LEO @1.9925 GHz: ±{est['delta_f_max_khz']:.2f} kHz, "
          f"taxa ~{est['doppler_rate_khz_per_s']:.3f} kHz/s")