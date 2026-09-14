# -*- coding: utf-8 -*-
"""siwe_flow.py — SIWE (EIP-4361) com verificação real via eth_account.

BLOCO 509 v69 (vetado) — C7/C8:
  * Autenticação descentralizada REAL: assinatura personal_sign recuperada por
    eth_account, comparada ao endereço declarado. Nada de "recovery simulado".
  * Nonce emitido pelo servidor, uso único, TTL de 5 min.
  * Fora de escopo (rejeitado): Flask/flask_sock (flask_sock ausente no
    ambiente), gRPC, rede.

Uso em selftest: cria um keypair fresco, assina, verifica, e prova que o
módulo REJEITA assinatura alheia e reuso de nonce.
"""

import secrets
import time
from dataclasses import dataclass, field
from typing import Dict, List, Optional

from eth_account import Account
from eth_account.messages import encode_defunct, SignableMessage

DOMAIN = "catedral.os"
CHAIN_ID = 8453
VERSION = 1
NONCE_TTL_S = 300


@dataclass
class SiweMessage:
    """Mensagem no formato EIP-4361 (SIWE)."""

    address: str
    uri: str
    nonce: str
    issued_at: Optional[str] = None
    domain: str = DOMAIN
    version: int = VERSION
    chain_id: int = CHAIN_ID
    statement: Optional[str] = None
    expiration_time: Optional[str] = None
    not_before: Optional[str] = None
    request_id: Optional[str] = None
    resources: List[str] = field(default_factory=list)

    def to_message(self) -> str:
        lines = [
            f"{self.domain} wants you to sign in with your Ethereum account:",
            self.address,
            "",
        ]
        if self.statement:
            lines += [self.statement, ""]
        lines += [
            f"URI: {self.uri}",
            f"Version: {self.version}",
            f"Chain ID: {self.chain_id}",
            f"Nonce: {self.nonce}",
            f"Issued At: {self.issued_at or ''}",
        ]
        if self.expiration_time:
            lines.append(f"Expiration Time: {self.expiration_time}")
        if self.not_before:
            lines.append(f"Not Before: {self.not_before}")
        if self.request_id:
            lines.append(f"Request ID: {self.request_id}")
        if self.resources:
            lines += ["Resources:"] + [f"- {r}" for r in self.resources]
        return "\n".join(lines)

    def message_hash(self) -> SignableMessage:
        return encode_defunct(text=self.to_message())


class SiweSessionStore:
    """Emitte nonces (uso único, TTL) e valida sessões."""

    def __init__(self) -> None:
        self._nonces: Dict[str, float] = {}
        self.sessions: Dict[str, Dict] = {}

    def issue_nonce(self) -> str:
        nonce = secrets.token_urlsafe(12)[:16]
        self._nonces[nonce] = time.time()
        return nonce

    def consume_nonce(self, nonce: str) -> bool:
        issued = self._nonces.pop(nonce, None)
        if issued is None:
            return False
        return (time.time() - issued) < NONCE_TTL_S

    def verify(self, message: SiweMessage, signature: str, expected_domain: str = DOMAIN) -> bool:
        if message.domain != expected_domain:
            return False
        if not self.consume_nonce(message.nonce):
            return False
        recovered = Account.recover_message(message.message_hash(), signature=signature)
        return recovered.lower() == message.address.lower()

    def create_session(self, address: str, ttl: int = 3600) -> Optional[str]:
        session_id = secrets.token_hex(16)
        self.sessions[session_id] = {
            "address": address,
            "created_at": time.time(),
            "expires_at": time.time() + ttl,
        }
        return session_id

    def session_address(self, session_id: str) -> Optional[str]:
        session = self.sessions.get(session_id)
        if session and time.time() < session["expires_at"]:
            return session["address"]
        self.sessions.pop(session_id, None)
        return None

    def authenticate(self, message: SiweMessage, signature: str,
                     expected_domain: str = DOMAIN) -> Optional[str]:
        if not self.verify(message, signature, expected_domain):
            return None
        return self.create_session(message.address)


def now_iso() -> str:
    return time.strftime("%Y-%m-%dT%H:%M:%S.000Z", time.gmtime())


def selftest() -> None:
    print("=== SIWE SELFTEST (EIP-4361 + eth_account, real) ===")

    store = SiweSessionStore()
    alice = Account.create()
    mallory = Account.create()

    nonce = store.issue_nonce()
    msg = SiweMessage(
        address=alice.address,
        uri="https://catedral.os/",
        nonce=nonce,
        issued_at=now_iso(),
        statement="Sign in to Catedral OS",
    )
    signed = alice.sign_message(msg.message_hash())
    sig_hex = signed.signature.hex()

    recovered = Account.recover_message(msg.message_hash(), signature=sig_hex)
    print(f"[SIW] recovered={recovered[:10]}... expected={alice.address[:10]}...")
    assert recovered.lower() == alice.address.lower()

    session = store.authenticate(msg, sig_hex)
    assert session is not None
    assert store.session_address(session) == alice.address
    print("[SIW] alice authenticated, session issued")

    same = SiweMessage(
        address=alice.address,
        uri="https://catedral.os/",
        nonce=nonce,
        issued_at=now_iso(),
        statement="Sign in to Catedral OS",
    )
    assert store.consume_nonce(nonce) is False
    print("[SIW] nonce single-use enforced (replay rejected)")

    nonce2 = store.issue_nonce()
    forged = SiweMessage(
        address=alice.address,
        uri="https://catedral.os/",
        nonce=nonce2,
        issued_at=now_iso(),
    )
    signed_m = mallory.sign_message(forged.message_hash())
    assert store.authenticate(forged, signed_m.signature.hex()) is None
    print("[SIW] forged identity (mallory key claiming alice) rejected")

    nonce3 = store.issue_nonce()
    phish = SiweMessage(
        address=alice.address,
        uri="https://evil.example/",
        nonce=nonce3,
        issued_at=now_iso(),
        domain="evil.example",
    )
    signed_a = alice.sign_message(phish.message_hash())
    assert store.authenticate(phish, signed_a.signature.hex()) is None
    print("[SIW] phishing domain rejected (origin binding enforced)")

    assert store.session_address("bogus") is None
    print("=== SIWE SELFTEST PASSED ===")


if __name__ == "__main__":
    selftest()