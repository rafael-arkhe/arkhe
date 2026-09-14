#!/usr/bin/env python3
"""
frequency_guard.py — Validação de faixas de frequência com base no PDFF (Res. 789/2026).
Inclui faixas de aviação, satélite, astronomia, TV 3.0 e 5G.
"""
from decimal import Decimal, ROUND_HALF_EVEN
from typing import Union, Optional, List, Tuple, Dict, Any
from dataclasses import dataclass
import csv
from pathlib import Path


@dataclass
class Classification:
    allowed: bool
    reason: str


class FrequencyGuard:
    """
    Guardião de frequências baseado no PDFF brasileiro.
    Suporta carregamento de faixas de CSV e validação com precisão de 1 kHz.
    """

    # Faixas padrão do PDFF (Res. 789/2026)
    DEFAULT_BANDS = [
        # ---- Emergência e Aviação (intocáveis) ----
        (Decimal("121.500"), Decimal("121.500"), "emergency_121_5"),
        (Decimal("108.000"), Decimal("137.000"), "aviation_108_137"),

        # ---- COSPAS-SARSAT ----
        (Decimal("406.000"), Decimal("406.100"), "cospas_ul_406"),
        (Decimal("1544.000"), Decimal("1545.000"), "cospas_dl_1544_1545"),

        # ---- GNSS/RNSS (GPS, Galileo, GLONASS) ----
        (Decimal("1559.000"), Decimal("1610.000"), "gnss_rnss_1559_1610"),

        # ---- Astronomia rádio (ITU RR 5.340) ----
        (Decimal("1400.000"), Decimal("1427.000"), "astronomy_rr_5_340"),

        # ---- MSS (Móvel por Satélite) ----
        (Decimal("1525.000"), Decimal("1559.000"), "mss_downlink_1525_1559"),
        (Decimal("1626.500"), Decimal("1660.500"), "mss_uplink_1626_1660"),

        # ---- TV 3.0 (faixa de 300 MHz) ----
        (Decimal("250.000"), Decimal("322.000"), "tv_30_250_322"),

        # ---- 5G (faixas leiloadas no Brasil) ----
        (Decimal("2300.000"), Decimal("2400.000"), "5g_2300_2400"),   # 2.3 GHz
        (Decimal("3300.000"), Decimal("3800.000"), "5g_3300_3800"),   # 3.5 GHz (n78)
    ]

    def __init__(self, bands: Optional[List[Tuple[Decimal, Decimal, str]]] = None):
        """
        Inicializa o guardião com uma lista de faixas.
        Se bands for None, usa DEFAULT_BANDS.
        """
        self.bands = bands if bands is not None else self.DEFAULT_BANDS

    @staticmethod
    def _to_quantized_mhz(value: Union[int, float, str, Decimal]) -> Optional[Decimal]:
        """Converte para Decimal e arredonda a 0.001 MHz (1 kHz)."""
        try:
            dec = Decimal(str(value))
        except Exception:
            return None
        if not dec.is_finite():
            return None
        return dec.quantize(Decimal("0.001"), rounding=ROUND_HALF_EVEN)

    def classify(self, value: Union[int, float, str, Decimal]) -> Classification:
        """Classifica uma frequência, retornando se é permitida e a razão."""
        freq = self._to_quantized_mhz(value)
        if freq is None:
            return Classification(False, "invalid_input")

        # Primeiro match (ordem de prioridade definida pela lista)
        for low, high, reason in self.bands:
            if low <= freq <= high:
                return Classification(False, reason)

        return Classification(True, "allowed")

    def is_allowed(self, value: Union[int, float, str, Decimal]) -> bool:
        """Retorna True se a frequência não estiver em nenhuma faixa restrita."""
        return self.classify(value).allowed

    def get_reason(self, value: Union[int, float, str, Decimal]) -> str:
        """Retorna a razão da restrição, se houver."""
        return self.classify(value).reason

    @classmethod
    def from_csv(cls, csv_path: Union[str, Path]) -> "FrequencyGuard":
        """
        Carrega faixas de um arquivo CSV no formato:
        low_freq,high_freq,reason
        (valores em MHz, com precisão decimal)
        """
        bands = []
        with open(csv_path, 'r', encoding='utf-8') as f:
            reader = csv.reader(f)
            for row in reader:
                if len(row) >= 3:
                    try:
                        low = Decimal(row[0].strip())
                        high = Decimal(row[1].strip())
                        reason = row[2].strip()
                        bands.append((low, high, reason))
                    except Exception:
                        continue
        return cls(bands)

    def to_csv(self, csv_path: Union[str, Path]) -> None:
        """Exporta as faixas atuais para um arquivo CSV."""
        with open(csv_path, 'w', encoding='utf-8') as f:
            writer = csv.writer(f)
            writer.writerow(["low_freq", "high_freq", "reason"])
            for low, high, reason in self.bands:
                writer.writerow([str(low), str(high), reason])
