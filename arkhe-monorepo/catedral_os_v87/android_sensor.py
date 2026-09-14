#!/usr/bin/env python3
"""
Passive Android PLMN helpers.

MCC-MNC table is a partial Brazil snapshot, not GSMA official.
dumpsys field names are OEM-unstable. First match wins.
Roaming is only set from an explicit mRoaming=true|false token.
"""

from __future__ import annotations

import re
import shutil
import subprocess
from dataclasses import dataclass
from typing import Optional, List, Dict, Any

# -----------------------------------------------------------------------------
# MCC-MNC TABLE (Brazil - MCC 724)
# Fonte: Dados da Anatel e referências do setor
# -----------------------------------------------------------------------------
MCC_MNC_BR: Dict[str, str] = {
    # Claro / Nextel
    "72400": "Claro",
    "72405": "Claro",
    "72412": "Claro",
    "72438": "Claro",
    "72439": "Claro",
    # TIM
    "72402": "TIM",
    "72403": "TIM",
    "72404": "TIM",
    "72423": "TIM",
    # Vivo
    "72406": "Vivo",
    "72410": "Vivo",
    "72411": "Vivo",
    "72413": "Vivo",
    "72419": "Vivo",
    "72421": "Vivo",
    # Oi
    "72416": "Oi",
    "72424": "Oi",
    "72425": "Oi",
    "72427": "Oi",
    "72429": "Oi",
    "72431": "Oi",
    "72435": "Oi",
    "72441": "Oi",
    "72445": "Oi",
    "72448": "Oi",
    "72451": "Oi",
    "72453": "Oi",
    "72455": "Oi",
    "72457": "Oi",
    "72459": "Oi",
    # Algar Telecom
    "72415": "Algar",
    "72432": "Algar",
    "72433": "Algar",
    "72434": "Algar",
    # Outras
    "72401": "Sisteer",        # MVNO
    "72407": "Sercomtel",
    "72408": "Transatel",      # MVNO
    "72437": "Unifique",
    "72430": "Nextel",         # iDEN (descontinuado)
}

NETWORK_TYPE: Dict[int, str] = {
    0: "unknown",
    1: "GPRS",
    2: "EDGE",
    3: "UMTS",
    4: "CDMA",
    5: "EVDO_0",
    6: "EVDO_A",
    7: "1xRTT",
    8: "HSDPA",
    9: "HSUPA",
    10: "HSPA",
    11: "IDEN",
    12: "EVDO_B",
    13: "LTE",
    14: "EHRPD",
    15: "HSPAP",
    16: "GSM",
    17: "TD_SCDMA",
    18: "IWLAN",
    19: "LTE_CA",
    20: "NR",  # 5G
}


class AdbMissing(RuntimeError):
    pass


def operator_from_plmn(plmn: str) -> str:
    """Retorna o nome da operadora a partir do PLMN (MCC+MNC)."""
    return MCC_MNC_BR.get(plmn, plmn)


@dataclass
class NetworkInfo:
    """Informações de rede coletadas via ADB."""
    plmn_home: str = ""
    plmn_sim: str = ""
    plmn_network: str = ""
    operator_alpha: str = ""
    service_state: int = -1
    data_connection_state: int = -1
    network_type: str = ""
    is_roaming: Optional[bool] = None
    # Campos adicionais para dual-SIM e depuração
    phone_id: Optional[int] = None
    voice_operator_numeric: str = ""
    data_operator_numeric: str = ""

    @property
    def plmn(self) -> str:
        return self.plmn_network or self.plmn_home or self.plmn_sim

    @property
    def operator(self) -> str:
        if self.plmn in MCC_MNC_BR:
            return MCC_MNC_BR[self.plmn]
        # Se houver nome alfa da operadora, usa-o como fallback
        if self.operator_alpha:
            return self.operator_alpha
        return self.plmn

    def context_fragment(self) -> str:
        """Gera um fragmento de contexto para logs (não entra no compute_alpha)."""
        parts = []
        if self.operator:
            parts.append(f"operator={self.operator}")
        if self.network_type:
            parts.append(f"rat={self.network_type}")
        if self.plmn:
            parts.append(f"plmn={self.plmn}")
        if self.is_roaming is True:
            parts.append("roaming=true")
        if self.phone_id is not None:
            parts.append(f"sim={self.phone_id}")
        return " ".join(parts)


def parse_telephony_dump(dump: str) -> NetworkInfo:
    """
    Extrai informações do dump do comando:
        adb shell dumpsys telephony.registry

    Nota: Os campos podem variar entre OEMs e versões do Android.
    """
    info = NetworkInfo()

    def first(pat: str) -> Optional[str]:
        m = re.search(pat, dump)
        return m.group(1).strip() if m else None

    # Campos principais (já existentes)
    info.plmn_home = first(r"mHomePlmn=(\d+)") or ""
    info.plmn_sim = first(r"mSimOperatorNumeric=(\d+)") or ""
    info.plmn_network = first(r"mOperatorNumeric=(\d+)") or ""
    info.operator_alpha = first(r"mOperatorAlphaShort=([^\n]+)") or ""

    # Campos adicionais para maior precisão
    info.voice_operator_numeric = first(r"mVoiceOperatorNumeric=(\d+)") or ""
    info.data_operator_numeric = first(r"mDataOperatorNumeric=(\d+)") or ""

    # Se mOperatorNumeric não estiver disponível, tenta voice/data
    if not info.plmn_network:
        if info.voice_operator_numeric:
            info.plmn_network = info.voice_operator_numeric
        elif info.data_operator_numeric:
            info.plmn_network = info.data_operator_numeric

    # Estado do serviço
    ss = first(r"mServiceState=(\d+)")
    if ss is not None:
        info.service_state = int(ss)

    # Estado da conexão de dados
    ds = first(r"mDataConnectionState=(\d+)")
    if ds is not None:
        info.data_connection_state = int(ds)

    # Tipo de rede (ex: LTE, NR)
    nt = first(r"mNetworkType=(\d+)")
    if nt is not None:
        info.network_type = NETWORK_TYPE.get(int(nt), nt)

    # Roaming
    roam = first(r"mRoaming=(true|false)")
    if roam is not None:
        info.is_roaming = roam == "true"

    # Identificador do SIM (para dual-SIM)
    phone_id = first(r"Phone Id=(\d+)")
    if phone_id is not None:
        info.phone_id = int(phone_id)

    return info


def get_plmn(device_serial: Optional[str] = None, timeout: float = 2.0) -> Optional[str]:
    """
    Obtém o PLMN da rede atual via `getprop gsm.operator.numeric`.

    Args:
        device_serial: Serial do dispositivo (opcional, para múltiplos devices).
        timeout: Tempo máximo de espera para o comando ADB.

    Returns:
        PLMN (MCC+MNC) como string, ou None em caso de falha.
    """
    adb = shutil.which("adb")
    if adb is None:
        raise AdbMissing("adb not on PATH")

    cmd = [adb]
    if device_serial:
        cmd.extend(["-s", device_serial])
    cmd.extend(["shell", "getprop", "gsm.operator.numeric"])

    try:
        proc = subprocess.run(cmd, capture_output=True, text=True, timeout=timeout)
    except subprocess.TimeoutExpired:
        return None

    if proc.returncode != 0:
        return None

    return proc.stdout.strip() or None


def get_operator_name(device_serial: Optional[str] = None, timeout: float = 2.0) -> Optional[str]:
    """
    Obtém o nome da operadora via `getprop gsm.operator.alpha`.

    Args:
        device_serial: Serial do dispositivo (opcional).
        timeout: Tempo máximo de espera.

    Returns:
        Nome da operadora, ou None em caso de falha.
    """
    adb = shutil.which("adb")
    if adb is None:
        raise AdbMissing("adb not on PATH")

    cmd = [adb]
    if device_serial:
        cmd.extend(["-s", device_serial])
    cmd.extend(["shell", "getprop", "gsm.operator.alpha"])

    try:
        proc = subprocess.run(cmd, capture_output=True, text=True, timeout=timeout)
    except subprocess.TimeoutExpired:
        return None

    if proc.returncode != 0:
        return None

    return proc.stdout.strip() or None


def list_devices() -> List[str]:
    """
    Lista os dispositivos Android conectados via ADB.

    Returns:
        Lista de serials dos dispositivos.
    """
    adb = shutil.which("adb")
    if adb is None:
        raise AdbMissing("adb not on PATH")

    try:
        proc = subprocess.run([adb, "devices"], capture_output=True, text=True, timeout=5)
    except subprocess.TimeoutExpired:
        return []

    devices = []
    for line in proc.stdout.strip().split("\n")[1:]:
        if not line.strip():
            continue
        parts = line.split()
        if len(parts) >= 2 and parts[1] == "device":
            devices.append(parts[0])
    return devices


def get_network_info(device_serial: Optional[str] = None, timeout: float = 5.0) -> Optional[NetworkInfo]:
    """
    Obtém informações detalhadas de rede via `dumpsys telephony.registry`.

    Args:
        device_serial: Serial do dispositivo (opcional).
        timeout: Tempo máximo de espera.

    Returns:
        NetworkInfo com os dados extraídos, ou None em caso de falha.
    """
    adb = shutil.which("adb")
    if adb is None:
        raise AdbMissing("adb not on PATH")

    cmd = [adb]
    if device_serial:
        cmd.extend(["-s", device_serial])
    cmd.extend(["shell", "dumpsys", "telephony.registry"])

    try:
        proc = subprocess.run(cmd, capture_output=True, text=True, timeout=timeout)
    except subprocess.TimeoutExpired:
        return None

    if proc.returncode != 0:
        return None

    return parse_telephony_dump(proc.stdout)
