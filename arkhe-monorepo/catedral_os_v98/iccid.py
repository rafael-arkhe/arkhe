#!/usr/bin/env python3
"""
iccid.py — Funções para validação e leitura de ICCID
"""
import hashlib
import re
from typing import Optional

def luhn_checksum(digits: str) -> bool:
    """Valida ICCID via algoritmo de Luhn (ISO/IEC 7812-1)"""
    if not digits.isdigit() or len(digits) < 18:
        return False
    total = 0
    for i, digit in enumerate(reversed(digits)):
        value = int(digit)
        if i % 2 == 1:
            value *= 2
            if value > 9:
                value -= 9
        total += value
    return total % 10 == 0

def extract_iccid_from_json(data: dict) -> Optional[str]:
    """Extrai ICCID de um dicionário JSON retornado pelo pySim-shell"""
    if "profile_info" in data:
        return data["profile_info"].get("iccid")
    if "iccid" in data:
        return data["iccid"]
    for key, value in data.items():
        if isinstance(value, dict):
            result = extract_iccid_from_json(value)
            if result:
                return result
    return None
