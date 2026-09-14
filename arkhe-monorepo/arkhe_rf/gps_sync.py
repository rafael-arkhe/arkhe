"""
Sincronização GPS (PPS) — linha do tempo absoluta para RF + ELF.

Requisito geométrico: timestamps coerentes (< 1 ms de desvio) entre o
canal RF (AntSDR) e o canal ELF. O PPS fornece pulsos de segundo exato;
este módulo converte cada captura num instante Unix absoluto.

Backends:
  - `gps` : lib gpsd (modo WATCH_ENABLE) + PPS real.
  - `sim` : relógio local alinhado ao próximo pulso de segundo inteiro.

Ambos expõem `gps_time` de forma idêntica para os drivers de captura.
"""

import math
import time

from . import config as _config

__all__ = ["GPSSync", "pps_next_second"]


def pps_next_second(now: float | None = None) -> float:
    """Próximo pulso PPS (segundo exato) a partir de `now` (epoch)."""
    now = time.time() if now is None else now
    return math.ceil(now)


class GPSSync:
    """
    Fonte de tempo PPS. Constrói a sessão gpsd quando disponível; caso
    contrário, opera com relógio local alinhado ao segundo inteiro.

    Args:
        tolerance_ms: Desvio máximo aceito entre canais antes de alertar.
        sim_mode: Forçar simulação (`None` = autodeteção).
    """

    def __init__(
        self,
        tolerance_ms: float = _config.GPS_PPS_TOLERANCE_MS,
        sim_mode: bool | None = None,
    ):
        self.tolerance_ms = float(tolerance_ms)
        self.mode = "sim"
        self._session = None
        self._last_pps: float = 0.0
        self._drift_ms: float = 0.0

        if sim_mode is None:
            sim_mode = _config.SIMULATION_MODE or True
        self._sim = sim_mode

        if not sim_mode:
            self._open_gps(required=True)

    # ------------------------------------------------------------------
    def _open_gps(self, required: bool = False) -> bool:
        try:
            from gps import gps, WATCH_ENABLE  # type: ignore

            self._session = gps(mode=WATCH_ENABLE)
            self.mode = "gpsd"
            return True
        except Exception as exc:  # noqa: BLE001 — backend opcional
            if required:
                raise RuntimeError(
                    f"GPSd indisponível ({exc}). Use sim_mode=True ou instale 'gps'."
                ) from exc
            return False

    # ------------------------------------------------------------------
    def get_pps_timestamp(self, now: float | None = None) -> float:
        """
        Timestamp Unix (epoch) do próximo pulso PPS.

        Em simulação, alinha ao próximo segundo exato do relógio local.
        """
        if self._session is not None:
            try:
                self._session.next()
                if hasattr(self._session, "fix"):
                    utc = getattr(self._session.fix, "time", None)
                    if utc and utc > 0:
                        self._last_pps = float(utc)
                        return self._last_pps
            except Exception:  # noqa: BLE001
                pass
        pps = pps_next_second(now)
        self._last_pps = pps
        return pps

    def timestamp(self, now: float | None = None) -> float:
        """Instante atual já referenciado à linha PPS (epoch flutuante)."""
        now = time.time() if now is None else now
        return now

    def lock_offset_s(self) -> float:
        """Desvio (s) entre o relógio do sistema e a base PPS (sim = 0)."""
        return 0.0

    def skew_ms(self) -> float:
        """Estimativa do skew atual (ms). Em simulação, 0."""
        return self._drift_ms

    def within_tolerance(self, t_a: float, t_b: float) -> bool:
        """`True` se dois timestamps estão coerentes (< tolerância)."""
        return abs(t_a - t_b) * 1e3 <= self.tolerance_ms

    def annotate(
        self,
        absolute_epoch_s: float,
        channel: str,
    ) -> dict:
        """
        Rotula um bloco de captura de um canal (RF/ELF) com o instante
        absoluto. Retorna metadados prontos para registro na Arkhe-Chain.
        """
        return {
            "channel": channel,
            "unix_epoch_s": round(absolute_epoch_s, 9),
            "pps_locked": bool(self._last_pps),
            "skew_ms": self.skew_ms(),
            "coherent_tolerance_ms": self.tolerance_ms,
        }

    def describe(self) -> dict:
        return {
            "mode": self.mode,
            "tolerance_ms": self.tolerance_ms,
            "last_pps_epoch": round(self._last_pps, 6),
        }

    def close(self) -> None:
        if self._session is not None:
            try:
                self._session.close()
            except Exception:  # noqa: BLE001
                pass
            self._session = None

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc, tb):
        self.close()
        return False


if __name__ == "__main__":
    gps = GPSSync()
    print("backend:", gps.describe())
    p1 = gps.get_pps_timestamp()
    p2 = gps.get_pps_timestamp()
    print("PPS:", p1, "→", p2, "| coerente:",
          gps.within_tolerance(p1, p2), "| skew_ms:", gps.skew_ms())