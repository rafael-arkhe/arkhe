#!/usr/bin/env python3
"""
anatel_band_guard.py — Validação de faixas de frequência conforme Res. 772/2025
"""
import csv
from pathlib import Path
from typing import List, Tuple, Optional, Dict, Any

# ============================================================================
# 1. Faixas restritas do PDFF (Res. 772/2025 e complementares)
# ============================================================================
# Fonte: ANATEL — Plano de Atribuição, Destinação e Distribuição de Faixas
# Nota: faixas críticas da aviação são intocáveis; outras possuem restrições
# ============================================================================

RESTRICTED_BANDS = [
    # ---- Aviação (intocáveis) ----
    (108.0, 118.0),      # Radionavegação (VOR/ILS)
    (118.0, 137.0),      # Comunicação ar-solo VHF
    (121.5, 121.5),      # Frequência de emergência (ponto)

    # ---- Busca e Salvamento (406 MHz) ----
    (406.0, 406.1),      # Satélite de busca e salvamento (COSPAS-SARSAT)

    # ---- Radioamador (faixas secundárias com restrições) ----
    (144.0, 146.0),      # VHF (secundário em algumas regiões)
    (430.0, 440.0),      # UHF (secundário)
    (1240.0, 1300.0),    # 23 cm (secundário)

    # ---- Radiodifusão AM/FM ----
    (0.526, 1.605),      # Ondas médias (AM)
    (87.8, 108.0),       # FM (88-108 MHz, com 87.8-88 MHz marginal)

    # ---- Serviço Móvel (faixas restritas) ----
    (150.0, 174.0),      # VHF Móvel (compartilhado)
    (450.0, 470.0),      # UHF Móvel (compartilhado)

    # ---- Satélite (downlink) ----
    (1525.0, 1559.0),    # Satélite (MSS)
    (1626.5, 1660.5),    # Satélite (MSS uplink)

    # ---- Serviço de Radiolocalização ----
    (290.0, 310.0),      # Radiolocalização

    # ---- Astronomia (faixas protegidas) ----
    (1400.0, 1427.0),    # Astronomia rádio (protegida)

    # ---- Serviço Amador (faixas primárias) ----
    (50.0, 54.0),        # 6 m (primário)
    (70.0, 70.5),        # 4 m (secundário)
    (146.0, 148.0),      # 2 m (primário)
]

# ============================================================================
# 1.1 Faixas ISM/SRD permitidas em caráter secundário (Res. 772/2025)
# ============================================================================
# A porção 433 MHz é permitida para aplicações ISM/SRD (ex.: LoRa) em caráter
# secundário, conforme regulamentação ANATEL, mesmo estando dentro da faixa
# restrita de radioamador UHF (430-440 MHz). Frequências dentro destas
# sub-faixas são consideradas PERMITIDAS e têm precedência sobre a restrição.
# ============================================================================

ISM_EXEMPT_BANDS = [
    (433.0, 434.79),     # ISM/SRD 433 MHz (secundário) — LoRa/telemetria
]

def _exempt(freq_mhz: float) -> bool:
    for low, high in ISM_EXEMPT_BANDS:
        if low <= freq_mhz <= high:
            return True
    return False

def is_frequency_allowed(freq_mhz: float) -> bool:
    """
    Verifica se uma frequência está liberada (não está em faixa restrita).
    Retorna True se permitida, False se proibida/restrita.
    """
    if _exempt(freq_mhz):
        return True
    for low, high in RESTRICTED_BANDS:
        if low <= freq_mhz <= high:
            return False
    return True

def get_restriction_reason(freq_mhz: float) -> str:
    """
    Retorna a razão pela qual a frequência é restrita, se aplicável.
    """
    if _exempt(freq_mhz):
        return "Faixa ISM/SRD permitida em caráter secundário"
    if 121.0 <= freq_mhz <= 122.0:
        return "Frequência de emergência (121.5 MHz)"
    for low, high in RESTRICTED_BANDS:
        if low <= freq_mhz <= high:
            if 108 <= freq_mhz <= 137:
                return "Aviação (VOR/ILS ou VHF COM)"
            elif 406.0 <= freq_mhz <= 406.1:
                return "Satélite de busca e salvamento (COSPAS-SARSAT)"
            elif 0.526 <= freq_mhz <= 1.605:
                return "Radiodifusão AM"
            elif 87.8 <= freq_mhz <= 108.0:
                return "Radiodifusão FM"
            elif 144 <= freq_mhz <= 148:
                return "Radioamador (faixa secundária com restrições)"
            elif 430 <= freq_mhz <= 440:
                return "Radioamador (faixa secundária)"
            elif 50 <= freq_mhz <= 54:
                return "Radioamador (faixa primária)"
            elif 1400 <= freq_mhz <= 1427:
                return "Astronomia rádio (protegida)"
            else:
                return "Faixa restrita por resolução ANATEL"
    return ""

# ============================================================================
# 2. Consulta a CSV externo (PDFF)
# ============================================================================

def load_bands_from_csv(csv_path: str | Path) -> List[Tuple[float, float]]:
    """
    Carrega faixas restritas de um arquivo CSV no formato:
    low_freq,high_freq,description
    """
    bands = []
    with open(csv_path, 'r', encoding='utf-8') as f:
        reader = csv.reader(f)
        for row in reader:
            if len(row) >= 2:
                try:
                    low = float(row[0])
                    high = float(row[1])
                    bands.append((low, high))
                except ValueError:
                    continue
    return bands

def load_bands_from_anatel_pdff() -> List[Tuple[float, float]]:
    """
    Tenta carregar as faixas do PDFF a partir de um arquivo CSV padrão.
    Se não existir, retorna a lista RESTRICTED_BANDS.
    """
    csv_file = Path("anatel_pdff_faixas.csv")
    if csv_file.exists():
        return load_bands_from_csv(csv_file)
    return RESTRICTED_BANDS

# ============================================================================
# 3. Stubs honestos para SDR e LoRa
# ============================================================================

class SDRHardwareStub:
    """
    Stub honesto para SDR (HackRF/USRP). Não executa transmissão real.
    """
    def __init__(self, device_type: str = "hackrf"):
        self.device_type = device_type
        self.is_connected = False

    def connect(self) -> bool:
        self.is_connected = True
        return True

    def start_sweep(self, start_mhz: float, end_mhz: float) -> dict:
        raise NotImplementedError(
            f"SDR sweep requer hardware {self.device_type} e pacote soapy_power instalado. "
            f"Simulação não disponível para operações de RF reais."
        )

    def transmit(self, frequency_mhz: float, data: bytes) -> dict:
        raise NotImplementedError(
            f"Transmissão SDR requer hardware {self.device_type} configurado. "
            f"Nenhuma transmissão real pode ser realizada neste stub."
        )

class LoRaHardwareStub:
    """
    Stub honesto para LoRa (RFM95/RFM96). Não executa transmissão real.
    """
    def __init__(self, spi_channel: int = 0, cs_pin: int = 1):
        self.spi_channel = spi_channel
        self.cs_pin = cs_pin
        self.is_initialized = False

    def initialize(self) -> bool:
        self.is_initialized = True
        return True

    def send(self, message: bytes, retries: int = 2) -> dict:
        raise NotImplementedError(
            f"Transmissão LoRa requer hardware RFM95/RFM96 conectado via SPI (CS={self.cs_pin}). "
            f"Instale pyLoraRFM9x e conecte o módulo para operação real."
        )

    def receive(self, timeout: float = 1.0) -> Optional[bytes]:
        raise NotImplementedError(
            f"Recepção LoRa requer hardware RFM95/RFM96 configurado. "
            f"Nenhuma recepção real disponível no stub."
        )

# ============================================================================
# 4. Integração com pysim (leitura de ICCID via subprocesso)
# ============================================================================

import subprocess
import re
import json
from typing import Optional

def read_iccid_from_pysim(reader: Optional[str] = None) -> Optional[str]:
    """
    Leitura FÍSICA legada via pySim-shell.py (Osmocom) via subprocesso.

    NOTA: a via PREFERENCIAL é a virtual (perfil eSIM, sem chip físico) —
    use esim_profile / SIMHarvester. Mantida aqui apenas como alternativa
    explícita para hardware físico (leitor de cartão).
    Retorna o ICCID como string ou None em caso de erro.
    """
    try:
        cmd = ["pySim-shell.py", "--json", "export"]
        if reader:
            cmd.extend(["-p", reader])

        result = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
            timeout=10
        )

        if result.returncode != 0:
            print(f"[pySim] Erro: {result.stderr}")
            return None

        try:
            data = json.loads(result.stdout)
            from iccid import extract_iccid_from_json
            return extract_iccid_from_json(data)
        except json.JSONDecodeError:
            match = re.search(r'ICCID["\s:]+([0-9]{18,22})', result.stdout)
            if match:
                return match.group(1)
            print("[pySim] ICCID não encontrado na saída")
            return None

    except FileNotFoundError:
        print("[pySim] pySim-shell.py não encontrado. Instale pysim.")
        return None
    except subprocess.TimeoutExpired:
        print("[pySim] Timeout na leitura do SIM.")
        return None
    except Exception as e:
        print(f"[pySim] Erro inesperado: {e}")
        return None

# ============================================================================
# 5. Documentação de referência (Res. 772/2025)
# ============================================================================

def print_reference():
    """
    Exibe as principais faixas do PDFF para referência rápida.
    """
    print("""
ANATEL — Resolução nº 772/2025 (PDFF)
=======================================
Faixas críticas:
  108-118 MHz: Radionavegação (VOR/ILS) — INTOCÁVEL
  118-137 MHz: Comunicação ar-solo VHF — INTOCÁVEL
  121.5 MHz:   Emergência — INTOCÁVEL
  406-406.1 MHz: Satélite busca e salvamento — PROIBIDO

Faixas com restrições (radioamador, radiodifusão, etc.) estão listadas
no array RESTRICTED_BANDS.

Faixas ISM/SRD permitidas em caráter secundário (ex.: 433 MHz para LoRa)
estão no array ISM_EXEMPT_BANDS.

Consulte o PDFF completo em: https://www.gov.br/anatel
""")

if __name__ == "__main__":
    print_reference()
