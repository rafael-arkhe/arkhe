"""siwe_verification_real.py — v77 VETADO — BLOCO 517 (V1)

Verificacao SIWE (EIP-4361) real com eth_account.

VETAGEM (consolida bloco 509/C7-C8):
  - A proposta v77 verificava apenas recovery + endereco na mensagem.
    O contrato do bloco 509 exige: nonce single-use + TTL + vinculo de
    dominio (anti-phishing). Aqui os tres sao aplicados.
  - recovery via encode_defunct (EIP-4361) — identidade forjada e reuso
    sao rejeitados (mesmos negativos do siwe_flow.py do bloco 509).

Modos:
  SIWEVerifier.verify(message, signature, expected_domain=None, store=None)
  returns (valid, recovered_address, metadata)
"""

from __future__ import annotations

import re
import time
from dataclasses import dataclass, field
from typing import Dict, Optional, Tuple

from eth_account import Account
from eth_account.messages import encode_defunct
from eth_utils import to_checksum_address


@dataclass
class NonceStore:
    """Emissao e consumo de nonces single-use com TTL (segundos)."""

    ttl: int = 300
    _seen: Dict[str, float] = field(default_factory=dict)

    def issue(self, weeks: int = 1, nonce: Optional[str] = None) -> str:
        if nonce is None:
            import secrets

            nonce = secrets.token_hex(16)[:16]
        self._seen[nonce] = time.time()
        return nonce

    def validate(self, nonce: str) -> bool:
        """Consome o nonce se existir, nao estiver expirado, e o remove."""
        now = time.time()
        ts = self._seen.get(nonce)
        if ts is None:
            return False
        del self._seen[nonce]  # single-use
        return (now - ts) <= self.ttl


def build_siwe_message(
    domain: str,
    address: str,
    uri: str,
    chain_id: int,
    nonce: str,
    issued_at: str,
) -> str:
    """Monta mensagem EIP-4361 canônica."""
    return (
        f"{domain} wants you to sign in with your Ethereum account:\n"
        f"{address}\n\n"
        f"Sign in with Ethereum to the app.\n\n"
        f"URI: {uri}\nVersion: 1\nChain ID: {chain_id}\n"
        f"Nonce: {nonce}\nIssued At: {issued_at}"
    )


class SIWEVerifier:
    def verify(
        self,
        message: str,
        signature: str,
        expected_domain: Optional[str] = None,
        store: Optional[NonceStore] = None,
    ) -> Tuple[bool, str, Dict[str, object]]:
        try:
            msg_hash = encode_defunct(text=message)
            recovered = Account.recover_message(msg_hash, signature=signature)

            domain_m = re.match(r"^([a-zA-Z0-9.-]+) wants you to sign in", message)
            if not domain_m:
                return False, "", {"error": "formato SIWE invalido"}
            domain = domain_m.group(1)
            if expected_domain is not None and domain.lower() != expected_domain.lower():
                return False, recovered, {"error": "dominio nao confiavel (phishing)"}

            addr_m = re.search(r"0x[a-fA-F0-9]{40}", message)
            if not addr_m:
                return False, "", {"error": "endereco ausente na mensagem"}
            expected = to_checksum_address(addr_m.group(0))

            if recovered.lower() != expected.lower():
                return False, recovered, {"error": "identidade forjada"}

            nonce_m = re.search(r"Nonce: ([a-zA-Z0-9]+)", message)
            if not nonce_m:
                return False, recovered, {"error": "nonce ausente"}
            if store is not None and not store.validate(nonce_m.group(1)):
                return False, recovered, {"error": "nonce reusado ou expirado"}

            issued_m = re.search(r"Issued At: (\S+)", message)
            return True, recovered, {
                "domain": domain,
                "issued_at": issued_m.group(1) if issued_m else "",
                "valid": True,
            }
        except Exception as exc:  # assinatura malformada etc.
            return False, "", {"error": str(exc)}


if __name__ == "__main__":
    import sys

    fail = 0
    PK = "0x4c0883a69102937d6231471b5dbb6204fe5129617082792ae468d01a3f362318"
    from datetime import datetime, timezone

    acct = Account.from_key(PK)

    store = NonceStore(ttl=300)
    nonce = store.issue()
    msg = build_siwe_message(
        "catedral.arkhe", acct.address, "https://catedral.arkhe/login",
        999, nonce, datetime.now(timezone.utc).isoformat().replace("+00:00", "Z"),
    )
    sig = acct.sign_message(encode_defunct(text=msg)).signature.hex()

    ok, rec, meta = SIWEVerifier().verify(msg, sig, expected_domain="catedral.arkhe", store=store)
    if not (ok and rec.lower() == acct.address.lower()):
        fail += 1
        print("[SIW] FAIL: auth real rejeitada")
    else:
        print(f"[SIW] auth real: recovered={rec}")

    # negativos esperados
    forged = Account.create()
    sigf = forged.sign_message(encode_defunct(text=msg)).signature.hex()
    ok2, _, m2 = SIWEVerifier().verify(msg, sigf)
    if ok2:
        fail += 1
        print("[SIW] FAIL: identidade forjada aceita")
    else:
        print(f"[SIW] identidade forjada rejeitada ({m2['error']})")

    ok3, _, m3 = SIWEVerifier().verify(msg, sig, expected_domain="evil.example")
    if ok3:
        fail += 1
        print("[SIW] FAIL: dominio phishing aceito")
    else:
        print(f"[SIW] dominio phishing rejeitado ({m3['error']})")

    ok4, _, m4 = SIWEVerifier().verify(msg, sig, expected_domain="catedral.arkhe", store=store)
    if ok4:
        fail += 1
        print("[SIW] FAIL: replay do nonce aceito")
    else:
        print(f"[SIW] replay do nonce rejeitado ({m4['error']})")

    store2 = NonceStore(ttl=-1)  # TTL vencido
    n2 = store2.issue()
    m5 = build_siwe_message(
        "catedral.arkhe", acct.address, "https://catedral.arkhe/login",
        999, n2, datetime.now(timezone.utc).isoformat().replace("+00:00", "Z"),
    )
    s5 = acct.sign_message(encode_defunct(text=m5)).signature.hex()
    ok5, _, _ = SIWEVerifier().verify(m5, s5, expected_domain="catedral.arkhe", store=store2)
    if ok5:
        fail += 1
        print("[SIW] FAIL: nonce expirado aceito")
    else:
        print("[SIW] nonce expirado rejeitado")

    print(f"[SIW] {'ALL CHECKS PASS' if fail == 0 else 'CHECKS FAILED'}")
    sys.exit(1 if fail else 0)