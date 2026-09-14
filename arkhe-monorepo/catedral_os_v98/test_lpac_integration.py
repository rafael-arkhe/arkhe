#!/usr/bin/env python3
"""
test_lpac_integration.py — Testes da via FÍSICA real (lpac / SGP.22).

Como não há hardware eUICC nem binário lpac neste ambiente, os testes
exercitam a lógica real de parsing/seleção/erro usando um runner injetado
que emula exatamente a saída JSON do `lpac profile list --json`.

A garantia de honestidade: quando `lpac` não está instalado, a via física
levanta LPACError (nunca sucesso falso).
"""
import json
import unittest
from types import SimpleNamespace

import esim_profile as esim
from esim_profile import LPACError

REAL_LPAC_LIST = {
    "type": "lpa",
    "payload": {
        "code": 0,
        "message": "success",
        "data": [
            {
                "iccid": "89353010000000000001",
                "isdpAid": "A0000005591010FFFFFFFF8900001000",
                "profileState": "disabled",
                "profileNickname": None,
                "serviceProviderName": "Vodafone IE",
                "profileName": "Vodafone IE eSIM",
                "iconType": "png",
                "icon": "iVBO...",
                "profileClass": "operational",
            },
            {
                "iccid": "89441111222233334446",
                "isdpAid": "A0000005591010FFFFFFFF8900001200",
                "profileState": "enabled",
                "profileNickname": None,
                "serviceProviderName": "Globalplay",
                "profileName": "Catedral",
                "iconType": "none",
                "icon": None,
                "profileClass": "operational",
            },
        ],
    },
}


def make_runner(result=None, returncode=0, stdout=None, stderr=""):
    r = result
    if r is None:
        r = SimpleNamespace(returncode=returncode, stdout=stdout or "", stderr=stderr)

    def _fake(args, **kwargs):
        return r

    return _fake


class TestLPACParsing(unittest.TestCase):
    def test_parse_envelope(self):
        raw = json.dumps(REAL_LPAC_LIST)
        data = esim._parse_lpac_payload(raw)
        self.assertEqual(len(data), 2)
        self.assertEqual(data[1]["iccid"], "89441111222233334446")

    def test_nonzero_code_raises(self):
        bad = {"type": "lpa", "payload": {"code": -1, "message": "no card"}}
        with self.assertRaises(LPACError):
            esim._parse_lpac_payload(json.dumps(bad))

    def test_invalid_envelope_raises(self):
        with self.assertRaises(LPACError):
            esim._parse_lpac_payload('{"hello": 1}')

    def test_active_only_prefers_enabled(self):
        iccid = esim.read_iccid_from_lpac(active_only=True, runner=make_runner(stdout=json.dumps(REAL_LPAC_LIST)))
        self.assertEqual(iccid, "89441111222233334446")

    def test_first_when_not_active_only(self):
        iccid = esim.read_iccid_from_lpac(active_only=False, runner=make_runner(stdout=json.dumps(REAL_LPAC_LIST)))
        self.assertEqual(iccid, "89353010000000000001")

    def test_match_by_provider(self):
        iccid = esim.read_iccid_from_lpac(match="Vodafone", runner=make_runner(stdout=json.dumps(REAL_LPAC_LIST)))
        self.assertEqual(iccid, "89353010000000000001")

    def test_match_not_found_raises(self):
        with self.assertRaises(LPACError):
            esim.read_iccid_from_lpac(match="Claro", runner=make_runner(stdout=json.dumps(REAL_LPAC_LIST)))

    def test_empty_profile_list_raises(self):
        empty = {"type": "lpa", "payload": {"code": 0, "message": "success", "data": []}}
        with self.assertRaises(LPACError):
            esim.read_iccid_from_lpac(runner=make_runner(stdout=json.dumps(empty)))


class TestLPACHErrors(unittest.TestCase):
    def test_binary_missing_raises(self):
        def _no_binary(args, **kw):
            raise FileNotFoundError("lpac")
        with self.assertRaises(LPACError) as ctx:
            esim._run_lpac(["profile", "list"], runner=_no_binary)
        self.assertIn("lpac", str(ctx.exception))

    def test_timeout_raises(self):
        import subprocess as sp

        def _slow(args, **kw):
            raise sp.TimeoutExpired(cmd=args, timeout=30)
        with self.assertRaises(LPACError):
            esim._run_lpac(["profile", "list"], runner=_slow)

    def test_disable_and_delete_issue_correct_args(self):
        seen = []

        def _spy(args, **kw):
            seen.append(list(args))
            return SimpleNamespace(returncode=0, stdout='{"type":"lpa","payload":{"code":0,"message":"success","data":null}}', stderr="")

        self.assertTrue(esim.disable_profile("8944111", runner=_spy))
        self.assertTrue(esim.delete_profile("8944111", runner=_spy))
        self.assertEqual(seen, [
            ["lpac", "profile", "disable", "8944111", "1"],
            ["lpac", "profile", "delete", "8944111"],
        ])

    def test_at_cimi_requires_pyserial(self):
        # Sem pySerial instalado => a via AT relança RuntimeError honesto
        # (nunca sucesso falso). Não podemos testar o runner injetado sem
        # o módulo serial instalado.
        with self.assertRaises(RuntimeError) as ctx:
            esim.read_iccid_from_at_cimi("/dev/ttyUSB0", runner=lambda port, **kw: "89441111222233334446")
        self.assertIn("pySerial", str(ctx.exception))

    def test_virtual_path_untouched(self):
        # A via lpac NÃO deve afetar a via virtual padrão.
        self.assertEqual(esim.read_iccid_from_virtual_esim("virtual_esim_profile.json"), "89441111222233334446")

    def test_backend_sets_lpac_apdu_env(self):
        seen = {}

        def _spy(args, **kw):
            seen["env_present"] = "env" in kw
            seen["env"] = kw.get("env")
            return SimpleNamespace(
                returncode=0,
                stdout=json.dumps({"type": "lpa", "payload": {"code": 0, "data": []}}),
                stderr="",
            )

        esim.list_profiles_lpac(runner=_spy, backend="mbim")
        self.assertTrue(seen["env_present"])
        self.assertEqual(seen["env"]["LPAC_APDU"], "mbim")

        seen.clear()
        esim.list_profiles_lpac(runner=_spy)
        # Sem backend explícito => nenhum env injetado.
        self.assertFalse(seen["env_present"])

    def test_invalid_backend_raises(self):
        with self.assertRaises(LPACError) as ctx:
            esim.list_profiles_lpac(runner=make_runner(), backend="usb")
        self.assertIn("Backend APDU desconhecido", str(ctx.exception))

    def test_backend_selection_reads_iccid(self):
        runner = make_runner(stdout=json.dumps(REAL_LPAC_LIST))
        # backend 'qmi' apenas define o env; a leitura ativa ainda funciona.
        self.assertEqual(
            esim.read_iccid_from_lpac(runner=runner, backend="qmi"),
            "89441111222233334446",
        )

    def test_all_backends_accepted(self):
        for b in esim.LPAC_BACKENDS:
            self.assertEqual(esim._validate_backend(b), b)
        # case-insensitive e vazio tratados
        self.assertEqual(esim._validate_backend("AT"), "at")
        self.assertIsNone(esim._validate_backend(""))
        self.assertIsNone(esim._validate_backend(None))


if __name__ == "__main__":
    unittest.main()
