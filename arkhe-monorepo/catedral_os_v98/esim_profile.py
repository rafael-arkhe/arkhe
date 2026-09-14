"""
esim_profile.py — Leitura de ICCID a partir de perfil eSIM virtual (SGP.22).

O eSIM é VIRTUAL (sem chip físico): o ICCID é lido do armazenamento do perfil
(arquivo JSON), e NÃO de um leitor de cartão USB (pySim). O módulo espelha o
Substrato 211 (agi_core.pl) para manter a consistência da cadeia soberana.
"""
import hashlib
import json
import os
import re
import time
from pathlib import Path
from typing import Any, Dict, Optional, Tuple

DEFAULT_PROFILE_FILE = "virtual_esim_profile.json"

# Espelha issuer_iin/2 do Substrato 211 (agi_core.pl)
ISSUERS = {
    "89450": ("Denmark", "Telia Sonera A/S"),
    "89491": ("Italy", "TIM"),
    "89441": ("Germany", "Globalplay"),
    "89650": ("Brazil", "Vivo"),
    "89550": ("Brazil", "Claro"),
    "89551": ("Brazil", "Claro"),
    "89510": ("Brazil", "TIM"),
    "89505": ("Brazil", "Oi"),
}


def luhn_valid(iccid: str) -> bool:
    """Valida ICCID via Luhn (ISO/IEC 7812-1), algoritmo idêntico ao Prolog."""
    if not iccid or not iccid.isdigit() or len(iccid) < 18:
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


def identify_issuer(iccid: str) -> Dict[str, str]:
    """Identifica a operadora a partir do prefixo IIN do ICCID (espelha substrato 211)."""
    s = str(iccid)
    for length in (7, 6, 5, 4):
        iin = s[:length]
        if iin in ISSUERS:
            country, company = ISSUERS[iin]
            return {"iin": iin, "country": country, "company": company}
    return {"iin": "Unknown", "country": "Unknown", "company": "Unknown"}


class VirtualESIM:
    """
    Perfil eSIM virtual (SGP.22) sem chip físico.
    O ICCID é lido do perfil armazenado (JSON), nunca de um leitor de cartão.
    """

    def __init__(self, profile_file: Optional[str] = None):
        self.profile_file = profile_file or DEFAULT_PROFILE_FILE

    def _load(self) -> Dict[str, Any]:
        path = Path(self.profile_file)
        if not path.exists():
            raise FileNotFoundError(f"Perfil eSIM virtual não encontrado: {self.profile_file}")
        with open(path, "r", encoding="utf-8") as f:
            data = json.load(f)
        return data

    @staticmethod
    def _extract_iccid(data: Dict[str, Any]) -> Optional[str]:
        if isinstance(data, dict):
            for key in ("iccid", "ICCID"):
                if key in data:
                    return data[key]
            for value in data.values():
                found = VirtualESIM._extract_iccid(value)
                if found:
                    return found
        return None

    def read_iccid(self) -> Optional[str]:
        """Lê o ICCID do perfil eSIM virtual armazenado (sem hardware físico)."""
        return self._extract_iccid(self._load())

    def validate(self) -> Dict[str, Any]:
        """Valida o perfil eSIM virtual (testar esim)."""
        profile = self._load()
        esim = profile.get("esim", {})
        iccid = self._extract_iccid(profile)
        luhn_ok = luhn_valid(iccid) if iccid else False
        issuer = identify_issuer(iccid) if iccid else identify_issuer("")
        state = profile.get("profile", {}).get("state", "unknown")
        valid = (
            luhn_ok
            and issuer["iin"] != "Unknown"
            and state == "installed"
            and esim.get("deployment") == "virtual"
            and esim.get("physical_chip") is False
        )
        return {
            "iccid": iccid,
            "luhn_valid": luhn_ok,
            "issuer": issuer,
            "state": state,
            "eid": profile.get("profile", {}).get("eid"),
            "virtual": esim.get("deployment") == "virtual",
            "physical_chip": esim.get("physical_chip"),
            "status": "valid" if valid else "invalid",
        }

    def manifest_profile(
        self, timestamp: Optional[str] = None, nonce: Optional[str] = None
    ) -> Dict[str, Any]:
        """
        Gera a âncora soberana do perfil (equivalente a iccid_register/2).
        Hash SHA-256 do blob {ICCID}:{timestamp}:{nonce}; content-addressed.
        """
        iccid = self.read_iccid()
        if not iccid or not luhn_valid(iccid):
            return {"status": "invalid_luhn", "iccid": iccid}
        ts = timestamp or time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
        nonce_val = nonce or os.urandom(8).hex()
        issuer = identify_issuer(iccid)
        raw = f"{iccid}:{ts}:{nonce_val}"
        manifest_hash = hashlib.sha256(raw.encode("utf-8")).hexdigest()
        return {
            "iccid": iccid,
            "timestamp": ts,
            "nonce": nonce_val,
            "hash": manifest_hash,
            "issuer": issuer,
            "status": "sovereign_anchor",
            "substrate": "211",
            "version": "9.5",
        }


def read_iccid_from_virtual_esim(profile_file: Optional[str] = None) -> Optional[str]:
    """Conveniência: lê o ICCID do perfil eSIM virtual."""
    return VirtualESIM(profile_file).read_iccid()


# ============================================================================
# VIA FÍSICA REAL — lpac (LPA compatível com SGP.22 v2.2.2) e comandos AT
# ============================================================================
# Estas funções NÃO dependem do perfil virtual: leem o ICCID real do modem
# eUICC via `lpac` (estkme-group/lpac) ou comandos AT (+CIMI).
#
# IMPORTANTE: requerem hardware real (modem/eUICC) e o binário `lpac`
# instalado. Em ambiente sem hardware, todas as chamadas levantam LPACError
# com mensagem clara — nunca retornam sucesso falso.
# ============================================================================

import subprocess as _subprocess


# Backends APDU suportados pelo lpac (selecionados via LPAC_APDU).
LPAC_BACKENDS = ("pcsc", "qmi", "mbim", "at", "gbinder")
_LPAC_DEFAULT_BACKEND = "pcsc"


class LPACError(RuntimeError):
    """Erro ao operar o lpac/eUICC físico (binário ausente, falha ou sem perfil)."""


def _validate_backend(backend: Optional[str]) -> Optional[str]:
    """Normaliza e valida o backend APDU. Levanta LPACError se desconhecido."""
    if backend is None:
        return None
    b = str(backend).strip().lower()
    if not b:
        return None
    if b not in LPAC_BACKENDS:
        raise LPACError(
            f"Backend APDU desconhecido: {backend}. "
            f"Válidos: {', '.join(LPAC_BACKENDS)}."
        )
    return b


def _parse_lpac_payload(raw: str) -> Dict[str, Any]:
    """
    Interpreta a resposta do lpac no envelope SGP.22:
        {"type":"lpa","payload":{"code":0,"message":"success","data":[...]}}
    Retorna payload.data; levanta LPACError quando code != 0.
    """
    try:
        doc = json.loads(raw)
    except json.JSONDecodeError as e:
        raise LPACError(f"Resposta do lpac não é JSON válido: {e}") from e
    if not isinstance(doc, dict) or doc.get("type") != "lpa":
        raise LPACError(f"Envelope lpac inesperado: {raw[:200]}")
    payload = doc.get("payload") or {}
    code = payload.get("code")
    if code != 0:
        raise LPACError(f"lpac: {payload.get('message') or 'erro desconhecido'} (code={code})")
    return payload.get("data")


def _run_lpac(args, runner=None, backend: Optional[str] = None) -> Dict[str, Any]:
    """
    Executa o binário `lpac` com os argumentos e retorna payload.data.

    `runner` injetável para testes (deve imitar subprocess.run).
    `backend` seleciona o backend APDU (pcsc/qmi/mbim/at/gbinder) via LPAC_APDU.
    """
    b = _validate_backend(backend)
    run = runner or _subprocess.run
    kwargs = {"capture_output": True, "text": True, "timeout": 30}
    if b:
        kwargs["env"] = {**os.environ.copy(), "LPAC_APDU": b}
    try:
        result = run(["lpac", *args], **kwargs)
    except FileNotFoundError as e:
        raise LPACError("lpac não encontrado. Instale estkme-group/lpac.") from e
    except _subprocess.TimeoutExpired as e:
        raise LPACError(f"lpac excedeu o tempo limite: {args}") from e
    if result.returncode != 0:
        raise LPACError(
            f"lpac retornou {result.returncode}: {(result.stderr or result.stdout or '').strip()[:300]}"
        )
    return _parse_lpac_payload(result.stdout or "")


def list_profiles_lpac(runner=None, backend: Optional[str] = None) -> list:
    """Lista os perfis eUICC reais ({iccid, profileState, ...}) via `lpac profile list`."""
    data = _run_lpac(["profile", "list", "--json"], runner=runner, backend=backend)
    profiles = data if isinstance(data, list) else []
    return [dict(p) for p in profiles if isinstance(p, dict)]


def read_iccid_from_lpac(
    active_only: bool = True,
    match: Optional[str] = None,
    runner=None,
    backend: Optional[str] = None,
) -> Optional[str]:
    """
    Lê o ICCID REAL de um perfil eUICC via `lpac profile list --json`.

    Parâmetros:
        active_only: True -> prefere o perfil em perfilState 'enabled'.
        match:       se informado, retorna o ICCID do perfil cujo ICCID/AID/Nome
                     contenha a string (case-insensitive).
        backend:     backend APDU (pcsc/qmi/mbim/at/gbinder) via LPAC_APDU.
                     None usa o backend padrão do lpac.
    """
    profiles = list_profiles_lpac(runner=runner, backend=backend)
    if not profiles:
        raise LPACError("Nenhum perfil eUICC encontrado no lpac profile list.")

    if match:
        needle = match.lower()
        for p in profiles:
            for field in ("iccid", "isdpAid", "profileName", "profileNickname"):
                val = str(p.get(field) or "")
                if needle in val.lower():
                    return p.get("iccid")
        raise LPACError(f"Nenhum perfil corresponde a: {match}")

    if active_only:
        for p in profiles:
            if str(p.get("profileState") or "").lower() == "enabled":
                return p.get("iccid")

    return profiles[0].get("iccid")


def enable_profile(iccid: str, refresh_flag: int = 1, runner=None, backend: Optional[str] = None) -> bool:
    """Ativa o perfil eUICC real: `lpac profile enable <iccid> [refreshFlag]`."""
    _run_lpac(
        ["profile", "enable", str(iccid), str(refresh_flag)],
        runner=runner,
        backend=backend,
    )
    return True


def disable_profile(iccid: str, refresh_flag: int = 1, runner=None, backend: Optional[str] = None) -> bool:
    """Desativa o perfil eUICC real: `lpac profile disable <iccid> [refreshFlag]`."""
    _run_lpac(
        ["profile", "disable", str(iccid), str(refresh_flag)],
        runner=runner,
        backend=backend,
    )
    return True


def delete_profile(iccid: str, runner=None, backend: Optional[str] = None) -> bool:
    """Remove o perfil eUICC real: `lpac profile delete <iccid>`."""
    _run_lpac(["profile", "delete", str(iccid)], runner=runner, backend=backend)
    return True


def read_iccid_from_at_cimi(port: str, baudrate: int = 115200, runner=None) -> Optional[str]:
    """
    Alternativa via comando AT: lê o ICCID real pelo AT+CIMI sobre serial.

    Requer pySerial (`pip install pyserial`). Recebe o ICCID exatamente como o
    modem responde a `AT+CIMI` (o mesmo número da chave eUICC).
    Levanta RuntimeError se pyserial não estiver disponível.
    """
    try:
        import serial  # type: ignore
    except ImportError as e:
        raise RuntimeError("pySerial não instalado. `pip install pyserial`.") from e

    def _default(port_arg, **kwargs):
        with serial.Serial(port_arg, baudrate, timeout=3) as ser:
            ser.write(b"AT+CIMI\r")
            line = ser.readline().decode("ascii", "replace").strip()
            # linha pode vir na forma "+CIMI: 8944..." ou direto o número
            iccid = re.sub(r"^\+CIMI:\s*", "", line)
            return iccid

    read = runner or _default
    iccid = read(port)
    if not iccid or not iccid.isdigit():
        raise LPACError(f"AT+CIMI não retornou ICCID válido em {port}.")
    return iccid


# Preferência documentada: via VIRTUAL por padrão; física (lpac/AT) sob demanda.
# Ex.: read_iccid(mode="lpac") ainda pode ser adicionado se necessário no futuro.
