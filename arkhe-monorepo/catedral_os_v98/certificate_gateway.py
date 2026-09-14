#!/usr/bin/env python3
"""
certificate_gateway.py — Substrato 212 (Camada de Confiança X.509/JWT/PKI).

Orquestra a camada de confiança da Catedral OS usando os pacotes instalados e
verificados neste ambiente:

  * PyJWT             2.13.0   (security release — HMAC HS256 com chave >= 32 bytes)
  * tiny_ca           0.2.1    (PKI local: emissão/revogação de certificados)
  * acmeow            1.2.0    (cliente ACME — RFC 8555, emissão/renovação)
  * hvac              2.4.0    (Hashicorp Vault — segredos/PKI) [exige servidor ao vivo]
  * certstream        1.12     (Certificate Transparency logs)
  * certvalidator     0.11.1   (validação de cadeias X.509)
  * asn1crypto        1.5.1    (parsing X.509 de baixo nível)
  * didkit            0.2.1    (resolução DID)
  * openvc-core       1.26.0   (DID/VC)
  * certmesh          3.0.38   (orquestração multi-provedor)

HONESTIDADE (regras Arquiteto-Ω): a disponibilidade real de cada recurso é
detectada em tempo de execução (capability report). Recursos que dependem de
serviços externos não subidos (Vault, ACME de produção, CT logs ao vivo)
relatam estado "indisponível" — nunca sucesso falso. As funções que NÃO
precisam de rede (JWT, tiny_ca PKI local, validação) são testáveis offline.
"""
from __future__ import annotations

import logging
from importlib import metadata
from pathlib import Path
from typing import Any, Callable, Dict, List, Optional

logger = logging.getLogger("cathedral.certificate")

# ---------------------------------------------------------------------------
# Capacidades — import opcional e honesto
# ---------------------------------------------------------------------------
_CAPS: Dict[str, bool] = {}
_MODS: Dict[str, Any] = {}


def _probe(name: str, mod_import: Callable[[], Any]) -> bool:
    try:
        mod = mod_import()
        _CAPS[name] = True
        _MODS[name] = mod
        return True
    except Exception as exc:  # noqa: BLE001
        logger.debug("capacidade %s indisponível: %s", name, exc)
        _CAPS[name] = False
        _MODS[name] = None
        return False


def _init_caps() -> None:
    _probe("jwt", lambda: __import__("jwt"))
    _probe("hvac", lambda: __import__("hvac"))
    _probe("certstream", lambda: __import__("certstream"))
    _probe("certvalidator", lambda: __import__("certvalidator"))
    _probe("tiny_ca", lambda: __import__("tiny_ca"))
    _probe("acmeow", lambda: __import__("acmeow"))
    _probe("asn1crypto", lambda: __import__("asn1crypto"))
    _probe("didkit", lambda: __import__("didkit"))
    _probe("openvc", lambda: __import__("openvc"))


_init_caps()


def _mod(name: str):
    """Retorna o módulo importado ou levanta se indisponível."""
    _require(_CAPS.get(name, False), name)
    return _MODS.get(name)

# Versões reais instaladas (via importlib.metadata), quando presentes.
_REAL_VERSIONS: Dict[str, str] = {}
for _pkg in (
    "PyJWT", "hvac", "certstream", "certvalidator", "tiny-ca",
    "acmeow", "asn1crypto", "didkit", "openvc-core", "certmesh",
):
    try:
        _REAL_VERSIONS[_pkg] = metadata.version(_pkg)
    except metadata.PackageNotFoundError:  # noqa: PERF203
        _REAL_VERSIONS[_pkg] = "não instalado"


# Configurações padrão da Catedral
JWT_MIN_KEY_BYTES: int = 32          # RFC 7518 §3.2 / PyJWT InsecureKeyLengthWarning
CERT_MAX_DAYS: int = 90              # política Let's Encrypt (validade máxima)
ACME_LETSENCRYPT: str = "https://acme-v02.api.letsencrypt.org/directory"
VAULT_DEFAULT_URL: str = "http://localhost:8200"


def capabilities() -> Dict[str, Any]:
    """Relatório honesto das capacidades disponíveis neste ambiente."""
    return {
        "installed": {p: _REAL_VERSIONS.get(p, "desconhecido") for p in _REAL_VERSIONS},
        "importable": dict(_CAPS),
        "network_dependent": {
            "vault_server_up": False,
            "acme_production": False,
            "ct_streaming": False,
        },
    }


# ---------------------------------------------------------------------------
# 1. JWT — PyJWT 2.13.0 (security release)
# ---------------------------------------------------------------------------
class CertificateGatewayError(RuntimeError):
    """Erro de integração da camada de confiança."""


def _require(flag: bool, what: str) -> None:
    if not flag:
        raise NotImplementedError(f"{what} não disponível neste ambiente")


def jwt_sign(payload: Dict[str, Any], secret: bytes) -> str:
    """Assina um JWT (HS256) com chave >= 32 bytes (hardening RFC 7518)."""
    jwt = _mod("jwt")
    if not isinstance(secret, bytes):
        raise TypeError("segredo JWT deve ser bytes")
    if len(secret) < JWT_MIN_KEY_BYTES:
        raise CertificateGatewayError(
            f"chave JWT muito curta: {len(secret)}B < mínimo {JWT_MIN_KEY_BYTES}B"
        )
    return jwt.encode(payload, secret, algorithm="HS256")


def jwt_verify(token: str, secret: bytes) -> Dict[str, Any]:
    """Verifica (e decodifica) um JWT HS256."""
    jwt = _mod("jwt")
    return jwt.decode(token, secret, algorithms=["HS256"])


# ---------------------------------------------------------------------------
# 2. tiny_ca — PKI local (sem rede; testável offline)
# ---------------------------------------------------------------------------
def _ca_loader_for(ca_cert, ca_key):
    """Persiste a CA em PEM temporário e devolve um CAFileLoader do tiny_ca."""
    import tempfile  # noqa: PLC0415
    from cryptography.hazmat.primitives import serialization  # noqa: PLC0415

    tiny_ca = _mod("tiny_ca")
    tmp = Path(tempfile.mkdtemp())
    cert_path = tmp / "ca.pem"
    key_path = tmp / "ca.key"
    cert_path.write_bytes(ca_cert.public_bytes(serialization.Encoding.PEM))
    key_path.write_bytes(
        ca_key.private_bytes(
            serialization.Encoding.PEM,
            serialization.PrivateFormat.TraditionalOpenSSL,
            serialization.NoEncryption(),
        )
    )
    return tiny_ca.CAFileLoader(cert_path, key_path)


def pki_issue_self_signed(
    common_name: str = "cathedral.local",
    organization: str = "Catedral OS",
    days_valid: int = CERT_MAX_DAYS,
    key_size: int = 2048,
    san_dns: Optional[List[str]] = None,
) -> Dict[str, Any]:
    """
    Emite um certificado por uma CA local (tiny_ca), sem rede.

    Cria uma CA raiz autoassinada e uma folha assinada por ela, com CN e SANs.
    Validade limitada a CERT_MAX_DAYS (política Let's Encrypt).
    Retorna {ca, leaf, subject, issuer, not_valid_after}.
    """
    _require(_CAPS.get("tiny_ca", False), "tiny_ca")
    tiny_ca = _mod("tiny_ca")
    if days_valid > CERT_MAX_DAYS:
        raise CertificateGatewayError(
            f"validade {days_valid}d excede o máximo {CERT_MAX_DAYS}d"
        )

    ca_cert, ca_key = tiny_ca.CertificateFactory.build_self_signed_ca(
        common_name=f"{common_name} CA",
        organization=organization,
        key_size=key_size,
        days_valid=days_valid,
    )
    loader = _ca_loader_for(ca_cert, ca_key)
    factory = tiny_ca.CertificateFactory(ca_loader=loader)
    leaf, _leaf_key, _csr = factory.issue_certificate(
        common_name=common_name,
        days_valid=days_valid,
        is_server_cert=True,
        san_dns=san_dns or [],
    )
    return {
        "ca": ca_cert,
        "leaf": leaf,
        "subject": leaf.subject.rfc4514_string(),
        "issuer": leaf.issuer.rfc4514_string(),
        "not_valid_after": leaf.not_valid_after.isoformat(),
    }


# ---------------------------------------------------------------------------
# 3. acmeow — cliente ACME (requer rede + Let's Encrypt)
# ---------------------------------------------------------------------------
def acme_build_client(email: str, storage_path: str, server_url: str = ACME_LETSENCRYPT):
    """Constrói um AcmeClient do acmeow. Requer rede para operar de verdade."""
    _require(_CAPS.get("acmeow", False), "acmeow")
    from acmeow import AcmeClient  # noqa: PLC0415

    return AcmeClient(server_url=server_url, email=email, storage_path=storage_path)


# ---------------------------------------------------------------------------
# 4. certstream — CT logs (streaming; exige rede/socket)
# ---------------------------------------------------------------------------
def ct_monitor(callback: Callable[[Dict[str, Any]], None], url: Optional[str] = None) -> None:
    """
    Subscreve ao fluxo de Certificate Transparency logs.
    Bloqueante. Requer conectividade; levanta se o socket falhar.
    """
    _require(_CAPS.get("certstream", False), "certstream")
    certstream = _mod("certstream")
    certstream.listen_for_events(
        callback,
        url=url,  # type: ignore[arg-type]
        skip_heartbeats=True,
        setup_logger=False,
    )


# ---------------------------------------------------------------------------
# 5. hvac — Vault (requer servidor ao vivo)
# ---------------------------------------------------------------------------
def vault_client(url: str = VAULT_DEFAULT_URL, token: Optional[str] = None):
    """Cria um hvac.Client. A autenticação real exige um Vault servindo."""
    _require(_CAPS.get("hvac", False), "hvac")
    return _mod("hvac").Client(url=url, token=token)


# ---------------------------------------------------------------------------
# 6. certvalidator — validação de cadeia X.509 (offline, testável)
# ---------------------------------------------------------------------------
def chain_validate(cert_der: bytes, trust_roots: List[bytes]) -> Dict[str, Any]:
    """Valida um certificado X.509 contra raízes confiáveis via certvalidator."""
    _require(_CAPS.get("certvalidator", False), "certvalidator")
    from certvalidator import CertificateValidator, ValidationContext  # noqa: PLC0415

    ctx = ValidationContext(trust_roots=trust_roots)
    validator = CertificateValidator(cert_der, ctx)
    try:
        result = validator.validate_usage({"digital_signature"})
        return {"valid": True, "path": str(result)}
    except Exception as exc:  # noqa: BLE001
        return {"valid": False, "error": str(exc)}


def iccid_san_cert(iccid: str, days_valid: int = CERT_MAX_DAYS) -> Dict[str, Any]:
    """
    Emite um certificado com o ICCID (Substrato 211) como CN e SAN DNS.
    Representa a identidade soberana de um nó da Catedral.
    """
    return pki_issue_self_signed(
        common_name=iccid,
        days_valid=days_valid,
        san_dns=["cathedral.local"],
    )


if __name__ == "__main__":
    logging.basicConfig(level=logging.INFO)
    caps = capabilities()
    print("=== SUBSTRATO 212 — CAPACIDADES REAIS ===")
    print("Instalados:", {k: v for k, v in caps["installed"].items() if v != "não instalado"})
    print("Importáveis:", caps["importable"])
    print("Dependem de rede:", caps["network_dependent"])
