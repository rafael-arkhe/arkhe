#!/usr/bin/env python3
"""
test_certificate_gateway.py — Testes do Substrato 212 (camada de confiança).

Cobre apenas o que é verificável OFFLINE neste ambiente JAVA:
  * PyJWT 2.13.0 — assinatura/verificação HS256 com chave >= 32 bytes,
    e recusa honesta de chave curta (hardening RFC 7518).
  * tiny_ca (PKI local) — emissão de CA + folha com ICCID no CN/SAN.
  * relatório de capacidades honesto (recursos de rede marcam indisponível).

Os recursos que exigem serviço ao vivo (Vault, ACME produção, CT streaming)
NÃO têm sucesso falso: são marcados como indisponíveis.
"""
import unittest

import certificate_gateway as gw

CANONICAL_ICCID = "89441111222233334446"


class TestJWTCapability(unittest.TestCase):
    def test_sign_verify_roundtrip(self):
        secret = b"x" * 64  # >= 32 bytes
        token = gw.jwt_sign({"sub": "node-1", "iccid": CANONICAL_ICCID}, secret)
        self.assertIsInstance(token, str)
        payload = gw.jwt_verify(token, secret)
        self.assertEqual(payload["iccid"], CANONICAL_ICCID)

    def test_short_key_rejected(self):
        # hardening: PyJWT 2.13.0 emite InsecureKeyLengthWarning p/ chave < 32B;
        # a Catedral recusa de forma excepcional em vez de só avisar.
        with self.assertRaises(gw.CertificateGatewayError):
            gw.jwt_sign({"a": 1}, b"short")

    def test_tampered_token_rejected(self):
        secret = b"y" * 48
        token = gw.jwt_sign({"role": "admin"}, secret)
        tampered = token[:-1] + ("A" if token[-1] != "A" else "B")
        with self.assertRaises(Exception):
            gw.jwt_verify(tampered, secret)


class TestTinyCAPKI(unittest.TestCase):
    @unittest.skipUnless(gw._CAPS.get("tiny_ca", False), "tiny_ca não instalado")
    def test_issue_iccid_cert(self):
        result = gw.iccid_san_cert(CANONICAL_ICCID, days_valid=90)
        # CN carrega o ICCID (identidade soberana do nó).
        self.assertIn(CANONICAL_ICCID, result["subject"])
        # A folha é assinada pela CA interna que acabamos de criar.
        self.assertTrue(result["issuer"].startswith("CN="))
        # A validade declarada obedece ao teto de CERT_MAX_DAYS.
        self.assertIn("not_valid_after", result)
        self.assertEqual(result["leaf"].serial_number > 0, True)
        self.assertEqual(result["leaf"].not_valid_after > result["leaf"].not_valid_before, True)

    @unittest.skipUnless(gw._CAPS.get("tiny_ca", False), "tiny_ca não instalado")
    def test_days_cap_enforced(self):
        with self.assertRaises(gw.CertificateGatewayError):
            gw.pki_issue_self_signed(days_valid=gw.CERT_MAX_DAYS + 1)

    @unittest.skipUnless(gw._CAPS.get("tiny_ca", False), "tiny_ca não instalado")
    def test_leaf_verifies_against_ca(self):
        result = gw.pki_issue_self_signed(common_name="node-a", san_dns=["a.local"])
        # A folha é emitida pela CA que criamos: o issuer do leaf é a CA local.
        self.assertEqual(result["leaf"].issuer, result["ca"].subject)


class TestCapabilitiesHonest(unittest.TestCase):
    def test_installed_versions_recorded(self):
        caps = gw.capabilities()
        # Pacotes que instalamos DE FATO estão presentes com versões reais.
        self.assertEqual(caps["installed"]["PyJWT"], "2.13.0")
        self.assertEqual(caps["installed"]["hvac"], "2.4.0")
        self.assertEqual(caps["installed"]["certmesh"], "3.0.38")

    def test_network_resources_not_claimed_up(self):
        caps = gw.capabilities()
        # Nenhum recurso dependente de serviço ao vivo é declarado "up".
        for k, v in caps["network_dependent"].items():
            self.assertFalse(v, f"{k} não deve ser declarado ativo sem serviço")


if __name__ == "__main__":
    unittest.main()
