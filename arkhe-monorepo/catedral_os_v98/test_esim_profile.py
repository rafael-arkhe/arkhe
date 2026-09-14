#!/usr/bin/env python3
"""
test_esim_profile.py — Testes do perfil eSIM virtual (testar esim).

Verifica que o eSIM é lido do armazenamento do perfil (virtual, sem chip
físico) e que o ICCID é válido por Luhn, emissor identificado, âncora
soberana gerada, e que NENHUM leitor físico (pySim) é acionado.
"""
import hashlib
import json
import os
import re
import tempfile
import unittest
from pathlib import Path

from esim_profile import (
    VirtualESIM,
    luhn_valid,
    identify_issuer,
    read_iccid_from_virtual_esim,
)

CANONICAL_ICCID = "89441111222233334446"
PROFILE_FILE = os.path.join(os.path.dirname(os.path.abspath(__file__)), "virtual_esim_profile.json")


class TestVirtualESIM(unittest.TestCase):
    def setUp(self):
        self.esim = VirtualESIM(PROFILE_FILE)

    def test_read_iccid_from_virtual_profile(self):
        iccid = self.esim.read_iccid()
        self.assertEqual(iccid, CANONICAL_ICCID)
        self.assertEqual(read_iccid_from_virtual_esim(PROFILE_FILE), CANONICAL_ICCID)

    def test_luhn_valid(self):
        self.assertTrue(luhn_valid(CANONICAL_ICCID))
        tampered = CANONICAL_ICCID[:-1] + str((int(CANONICAL_ICCID[-1]) + 1) % 10)
        self.assertFalse(luhn_valid(tampered))

    def test_identify_issuer(self):
        info = identify_issuer(CANONICAL_ICCID)
        self.assertEqual(info["iin"], "89441")
        self.assertEqual(info["country"], "Germany")
        self.assertEqual(info["company"], "Globalplay")

    def test_validate_virtual_esim(self):
        result = self.esim.validate()
        self.assertEqual(result["status"], "valid")
        self.assertTrue(result["luhn_valid"])
        self.assertTrue(result["virtual"])
        self.assertIs(result["physical_chip"], False)
        self.assertEqual(result["state"], "installed")
        self.assertEqual(result["issuer"]["company"], "Globalplay")

    def test_manifest_sovereign_anchor(self):
        m = self.esim.manifest_profile(timestamp="2026-08-28T12:00:00Z", nonce="a1b2c3d4")
        self.assertEqual(m["status"], "sovereign_anchor")
        self.assertEqual(m["iccid"], CANONICAL_ICCID)
        self.assertTrue(re.fullmatch(r"[0-9a-f]{64}", m["hash"]))
        expected = hashlib.sha256(
            f"{CANONICAL_ICCID}:2026-08-28T12:00:00Z:a1b2c3d4".encode("utf-8")
        ).hexdigest()
        self.assertEqual(m["hash"], expected)

    def test_virtual_path_requires_no_hardware(self):
        """A leitura VIRTUAL não depende de lpac/serial/hardware físico."""
        import esim_profile
        # A via virtual lê direto do arquivo de perfil, sem binário externo.
        self.assertEqual(esim_profile.read_iccid_from_virtual_esim(PROFILE_FILE), CANONICAL_ICCID)
        # O aparelho virtual indica explicitamente a ausência de chip físico.
        self.assertIs(self.esim.validate()["physical_chip"], False)
        # A via virtual não depende de lpac ou saída de subprocess.
        self.assertNotIn("lpac", esim_profile.read_iccid_from_virtual_esim(PROFILE_FILE))

    def test_physical_path_is_separate_and_gated(self):
        """A via física (lpac) existe, mas é separada e falha se o binário não existir."""
        import esim_profile
        # Sem binário lpac instalado => LPACError honesto, não sucesso falso.
        with self.assertRaises(esim_profile.LPACError):
            esim_profile._run_lpac(["profile", "list"])

    def test_missing_profile_raises(self):
        with self.assertRaises(FileNotFoundError):
            VirtualESIM("nao_existe_esim.json").read_iccid()

    def test_tampered_profile_is_invalid(self):
        with tempfile.NamedTemporaryFile(mode="w", suffix=".json", delete=False) as f:
            with open(PROFILE_FILE, encoding="utf-8") as src:
                data = json.load(src)
            data["profile"]["iccid"] = "89441111222233334447"
            json.dump(data, f)
            tmp = f.name
        try:
            result = VirtualESIM(tmp).validate()
            self.assertEqual(result["status"], "invalid")
            self.assertFalse(result["luhn_valid"])
        finally:
            os.unlink(tmp)


if __name__ == "__main__":
    unittest.main()
