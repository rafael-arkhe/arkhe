"""
iccid_reader.py — Leitura de ICCID (via virtual + via física legada).

Via PREFERENCIAL: perfil eSIM VIRTUAL (sem chip físico) — sem hardware,
sem pySim-shell. A leitura delega ao SIMHarvester (que decide entre a via
virtual e a física).

Via física (legada, opcional): pySim-shell (Osmocom) — somente quando um
reader (leitor de cartão) for explicitamente solicitado.
"""
import subprocess
import json
import re
from typing import Optional


def read_iccid(reader: Optional[str] = None) -> Optional[str]:
    """
    Lê o ICCID.

    Por padrão usa a via VIRTUAL (perfil eSIM, sem chip físico). Se `reader`
    for fornecido, usa a via FÍSICA legada via pySim-shell.

    Parâmetros:
        reader: se informado, força a via física (leitor de cartão pySim).
    """
    if reader:
        return read_iccid_from_pysim(reader)

    # Via virtual (padrão, sem hardware)
    from cathedral_orchestrator import SIMHarvester
    result = SIMHarvester().harvest_iccid()
    if result.get("status") == "success":
        return result.get("iccid")
    return None


def read_iccid_from_pysim(reader: Optional[str] = None) -> Optional[str]:
    """
    Leitura FÍSICA legada via pySim-shell (Osmocom), apenas para hardware.

    A via preferencial é a virtual (esim_profile / SIMHarvester). Use esta
    função exclusivamente quando houver um leitor de cartão físico.
    """
    if not reader:
        print("[pySim] reader (leitor físico) não informado; use a via virtual.")
        return None
    try:
        cmd = ["pySim-shell.py", "--json", "export", "-p", reader]

        result = subprocess.run(cmd, capture_output=True, text=True, timeout=10)

        if result.returncode != 0:
            print(f"[pySim] Erro: {result.stderr}")
            return None

        try:
            data = json.loads(result.stdout)
            if "profile_info" in data:
                return data["profile_info"].get("iccid")
            if "iccid" in data:
                return data["iccid"]
            return _extract_iccid_from_json(data)
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


def _extract_iccid_from_json(data: dict) -> Optional[str]:
    """Extrai ICCID de um dicionário JSON."""
    if "profile_info" in data:
        return data["profile_info"].get("iccid")
    if "iccid" in data:
        return data["iccid"]
    for key, value in data.items():
        if isinstance(value, dict):
            result = _extract_iccid_from_json(value)
            if result:
                return result
    return None


def luhn_validate(iccid: str) -> bool:
    """Valida ICCID usando algoritmo de Luhn (ISO/IEC 7812-1)."""
    if not iccid.isdigit() or len(iccid) < 18:
        return False
    total = 0
    for i, digit in enumerate(reversed(iccid)):
        value = int(digit)
        if i % 2 == 1:
            value *= 2
            if value > 9:
                value -= 9
        total += value
    return total % 10 == 0
