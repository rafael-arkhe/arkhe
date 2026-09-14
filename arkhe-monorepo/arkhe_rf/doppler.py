"""
Rastreamento e correção Doppler para sinais de satélites LEO.

Física:
  Δf = (v_radial / c) × f₀

  LEO (~550 km): v_orbital ≈ 7589 m/s
  A 1,9 GHz: Δf_max ≈ ±50 kHz (horizonte) → 0 Hz (zênite)

  O pipeline aceita qualquer valor real — a janela de busca
  de ±300 kHz cobre tanto a estimativa teórica quanto valores
  reportados em documentos que precisam de validação empírica.

Constantes usadas nos cálculos independentes (não dependem do documento).
"""

import numpy as np
from collections import deque
from dataclasses import dataclass, field

from . import config as _config


@dataclass
class DopplerState:
    """Estado atual do rastreador Doppler."""

    slope_hz_per_s: float = 0.0
    slope_khz_per_s: float = 0.0
    r_squared: float = 0.0
    n_points: int = 0
    intercept_hz: float = 0.0
    is_satellite: bool = False
    confidence: str = "INSUFICIENTE"  # INSUFICIENTE | ESTATICO | POSSIVEL | PROVAVEL | SATELITE


class DopplerTracker:
    """
    Rastrea a frequência de pico ao longo do tempo e estima
    a inclinação Doppler (kHz/s) para distinguir sinais LEO
    de sinais terrestres estáticos.
    """

    def __init__(
        self,
        window_size: int = 60,
        slope_min_hz: float = 500.0,    # 0.5 kHz/s — mínimo para LEO
        slope_max_hz: float = 3000.0,   # 3.0 kHz/s — máximo esperado
        r_squared_min: float = 0.7,     # ajuste linear mínimo
    ):
        self.window_size = window_size
        self.slope_min = slope_min_hz
        self.slope_max = slope_max_hz
        self.r_squared_min = r_squared_min
        self.freq_history: deque = deque(maxlen=window_size)
        self.time_history: deque = deque(maxlen=window_size)

    def update(self, peak_freq: float, timestamp: float) -> DopplerState:
        """Adiciona uma medição e retorna o estado atual."""
        self.freq_history.append(peak_freq)
        self.time_history.append(timestamp)

        state = DopplerState(n_points=len(self.freq_history))

        if len(self.freq_history) < 5:
            return state

        t = np.array(self.time_history)
        f = np.array(self.freq_history)

        # Remover NaNs (frames sem pico detectado)
        valid = ~np.isnan(f)
        n_valid = int(valid.sum())
        state.n_points = n_valid

        if n_valid < 5:
            return state

        t_v = t[valid]
        f_v = f[valid]

        # Ajuste linear (mínimos quadrados)
        coeffs = np.polyfit(t_v, f_v, 1)
        slope = coeffs[0]
        intercept = coeffs[1]

        state.slope_hz_per_s = float(slope)
        state.slope_khz_per_s = float(slope / 1e3)
        state.intercept_hz = float(intercept)

        # R²
        f_pred = np.polyval(coeffs, t_v)
        ss_res = np.sum((f_v - f_pred) ** 2)
        ss_tot = np.sum((f_v - np.mean(f_v)) ** 2) + 1e-20
        state.r_squared = 1.0 - ss_res / ss_tot

        # Classificação
        abs_slope = abs(slope)
        if state.r_squared >= self.r_squared_min and self.slope_min <= abs_slope <= self.slope_max:
            state.is_satellite = True
            if state.r_squared > 0.9 and abs_slope > 1000:
                state.confidence = "SATELITE"
            elif state.r_squared > 0.8:
                state.confidence = "PROVAVEL"
            else:
                state.confidence = "POSSIVEL"
        else:
            state.is_satellite = False
            if abs_slope < 100:
                state.confidence = "ESTATICO"  # provavelmente terrestre
            else:
                state.confidence = "INSUFICIENTE"

        return state

    def reset(self) -> None:
        """Limpa o histórico."""
        self.freq_history.clear()
        self.time_history.clear()

    def get_predicted_freq(self, t_future: float) -> float | None:
        """Extrapola a frequência para um momento futuro."""
        if len(self.freq_history) < 5:
            return None
        t = np.array(self.time_history)
        f = np.array(self.freq_history)
        valid = ~np.isnan(f)
        if int(valid.sum()) < 5:
            return None
        coeffs = np.polyfit(t[valid], f[valid], 1)
        return float(np.polyval(coeffs, t_future))


def doppler_shift_iq(
    iq: np.ndarray,
    shift_hz: float,
    sample_rate: float,
) -> np.ndarray:
    """
    Corrige o desvio Doppler em dados IQ misturando com
    uma exponencial complexa na frequência oposta.

    Args:
        iq: Array complexo de amostras IQ
        shift_hz: Desvio a corrigir em Hz (positivo = upshift)
        sample_rate: Taxa de amostragem em Hz

    Returns:
        IQ com Doppler corrigido
    """
    t = np.arange(len(iq), dtype=np.float64) / sample_rate
    mixer = np.exp(-2j * np.pi * shift_hz * t)
    return iq * mixer


def estimate_leo_doppler_max(
    freq_hz: float,
    altitude_km: float = 550.0,
) -> dict:
    """
    Estima o desvio Doppler máximo para um satélite LEO.

    Cálculo independente — não depende de documentos externos.

    Args:
        freq_hz: Frequência da portadora em Hz
        altitude_km: Altitude orbital em km

    Returns:
        Dict com parâmetros Doppler estimados
    """
    G = _config.GRAVITATIONAL_CONSTANT     # m³/(kg·s²)
    M_earth = _config.EARTH_MASS_KG        # kg
    R_earth = _config.EARTH_RADIUS_M       # m
    c = _config.SPEED_OF_LIGHT_MS          # m/s

    r = R_earth + altitude_km * 1e3
    v_orbital = np.sqrt(G * M_earth / r)

    # No horizonte, a componente radial pode atingir até v_orbital
    # (dependendo do azimute). Usamos o pior caso.
    delta_f_max = (v_orbital / c) * freq_hz

    # Taxa de variação: d(Δf)/dt ≈ v² / (r × c) × f
    # Ordem de grandeza: alguns kHz/s
    doppler_rate = (v_orbital ** 2 / (r * c)) * freq_hz

    return {
        "altitude_km": altitude_km,
        "v_orbital_m_s": round(v_orbital, 1),
        "delta_f_max_hz": round(delta_f_max, 1),
        "delta_f_max_khz": round(delta_f_max / 1e3, 2),
        "doppler_rate_hz_per_s": round(doppler_rate, 1),
        "doppler_rate_khz_per_s": round(doppler_rate / 1e3, 3),
        "carrier_freq_mhz": round(freq_hz / 1e6, 1),
        "note": "Componente radial máxima no horizonte. No zênite, Δf → 0.",
    }


def validate_doppler_claim(
    claimed_khz: float,
    freq_hz: float,
    altitude_km: float = 550.0,
) -> dict:
    """
    Compara um valor de Doppler reivindicado com a estimativa física.

    Returns:
        Dict com análise da discrepância
    """
    est = estimate_leo_doppler_max(freq_hz, altitude_km)
    claimed_hz = claimed_khz * 1e3
    ratio = claimed_hz / est["delta_f_max_hz"]

    if ratio < 1.5:
        verdict = "CONSISTENTE"
    elif ratio < 3.0:
        verdict = "ALTO — possivelmente inclui fatores adicionais"
    elif ratio < 6.0:
        verdict = "DISCREPANTE — verificar fonte e frequência"
    else:
        verdict = "INCONSISTENTE com física LEO a esta frequência"

    return {
        "claimed_khz": claimed_khz,
        "estimated_max_khz": est["delta_f_max_khz"],
        "ratio": round(ratio, 2),
        "verdict": verdict,
        "details": est,
    }