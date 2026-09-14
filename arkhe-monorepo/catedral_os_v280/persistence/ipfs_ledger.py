# persistence/ipfs_ledger.py
"""
L1 — Ledger distribuido no IPFS (v280.0).

Encadeia cada entrada por hash (mesmas regras do ledger local) e, quando um
no IPFS esta acessivel, publica cada bloco como um objeto imutavel no IPFS
via `ipfshttpclient`. O "last hash" local passa a ser o CID do bloco
publicado; o fallback local preserva a imutabilidade e a verificabilidade.

Nenhuma distribuicao e fingida: sem `ipfshttpclient` ou sem daemon, o modo
local e explicitamente reportado em `stats().mode`.
"""

from __future__ import annotations

import logging
from typing import Dict, Optional

from persistence.local_ledger import LocalLedger

logger = logging.getLogger(__name__)

try:
    import ipfshttpclient  # type: ignore

    IPFS_AVAILABLE = True
except ImportError:  # pragma: no cover - ambiente sem binding
    IPFS_AVAILABLE = False

try:
    from ipfshttpclient.exceptions import ConnectionError as IPFSConnError  # type: ignore

    IPFS_EXC = IPFSConnError
except Exception:  # pragma: no cover - binding ausente
    IPFS_EXC = Exception


class IPFSLedger:
    """Ledger imutavel distribuido com fallback local (I255)."""

    DEFAULT_GATEWAY = "/ip4/127.0.0.1/tcp/5001/http"

    def __init__(self, gateway: str = DEFAULT_GATEWAY, use_ipfs: bool = True,
                 local_path: Optional[str] = None, timeout: float = 2.0,
                 persist: bool = True):
        self.gateway = gateway
        self.use_ipfs = use_ipfs and IPFS_AVAILABLE
        self.timeout = timeout
        self.client = None
        self.ipfs_ready = False

        self.local = LocalLedger(path=local_path or "ledger_chain.jsonl",
                                 persist=persist)

        if self.use_ipfs:
            try:
                self.client = ipfshttpclient.connect(gateway)
                self.ipfs_ready = bool(self.client.id())
                logger.info("Conectado ao IPFS em %s", gateway)
            except Exception as exc:  # pragma: no cover - rede indisponivel
                logger.warning("Falha ao conectar ao IPFS: %s. Modo local.", exc)
                self.use_ipfs = False

    # ------------------------------------------------------------------ #
    def append(self, entry: Dict) -> str:
        """Adiciona uma entrada; retorna o CID IPFS ou o hash local.

        O fluxo: assina e encadeia localmente (mesma regra do fallback),
        depois publica o bloco assinado como objeto imutavel no IPFS. O CID
        torna-se a ancora observavel; a cadeia local preserva a verificacao.
        """
        local_hash = self.local.append(entry)
        record = self.local.chain[-1]

        if self.ipfs_ready and self.client is not None:
            try:
                res = self.client.add_json(record)
                cid = res["Hash"]
                logger.debug("Bloco %s publicado no IPFS: %s", local_hash[:12], cid[:12])
                return cid
            except Exception as exc:  # pragma: no cover - daemon caiu
                logger.error("Falha ao publicar no IPFS: %s. Fallback local.", exc)
                self.ipfs_ready = False
                self.use_ipfs = False
                self.client = None

        return local_hash

    # ------------------------------------------------------------------ #
    def verify_chain(self) -> bool:
        return self.local.verify_chain()

    def is_chain_ok(self) -> bool:
        return self.local.is_chain_ok()

    def tamper(self, index: int, key: str, value: object) -> None:
        self.local.tamper(index, key, value)

    # ------------------------------------------------------------------ #
    def stats(self) -> Dict:
        stat = self.local.stats()
        stat["ipfs_available"] = IPFS_AVAILABLE
        stat["ipfs_ready"] = self.ipfs_ready
        stat["mode"] = "ipfs" if self.ipfs_ready else "local"
        return stat

    @property
    def last_cid(self) -> str:
        return self.local.last_hash