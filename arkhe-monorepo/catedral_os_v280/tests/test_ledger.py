# catedral_os_v280/tests/test_ledger.py
"""Testes do ledger (L1): hash-chain local + fallback IPFS."""

import os

import pytest

from persistence.ipfs_ledger import IPFSLedger
from persistence.local_ledger import LocalLedger


def test_local_ledger_append_and_verify(tmp_path):
    ledger = LocalLedger(path=str(tmp_path / "chain.json"))
    h1 = ledger.append({"type": "test", "n": 1})
    h2 = ledger.append({"type": "test", "n": 2})
    assert h1 != h2
    assert ledger.verify_chain()
    assert ledger.stats()["entries"] == 2
    assert tmp_path.joinpath("chain.json").exists()


def test_local_ledger_reload(tmp_path):
    path = str(tmp_path / "chain.json")
    ledger = LocalLedger(path=path)
    ledger.append({"type": "a"})
    ledger2 = LocalLedger(path=path)
    assert ledger2.stats()["entries"] == 1
    assert ledger2.verify_chain()


def test_local_ledger_reload_then_append_keeps_continuity(tmp_path):
    path = str(tmp_path / "chain.json")
    ledger = LocalLedger(path=path)
    for i in range(10):
        ledger.append({"i": i})
    ledger2 = LocalLedger(path=path)
    ledger2.append({"i": "pos-reload"})
    assert ledger2.verify_chain()
    assert ledger2.last_hash == ledger2.chain[-1]["hash"]


def test_local_ledger_detects_tamper(tmp_path):
    ledger = LocalLedger(path=str(tmp_path / "chain.json"))
    ledger.append({"type": "a", "payload": "original"})
    ledger.tamper(0, "payload", "corrompido")
    assert not ledger.verify_chain()
    assert not ledger.is_chain_ok()


def test_ipfs_ledger_fallback_local(tmp_path, monkeypatch):
    caller = IPFSLedger(use_ipfs=True, local_path=str(tmp_path / "ipfs_local.json"))
    assert not caller.ipfs_ready
    cid = caller.append({"type": "acao", "phi": 0.6})
    assert cid and caller.stats()["mode"] == "local"
    assert caller.verify_chain()
    assert len(cid) == 64  # sha256 hex


def test_ipfs_ledger_rejects_zero_append(tmp_path):
    ledger = IPFSLedger(use_ipfs=False, local_path=str(tmp_path / "chain.json"))
    ledger.append({"type": "a"})
    ledger.append({"type": "b"})
    assert len(ledger.local.chain) == 2


def test_ledger_survives_chaotic_order(tmp_path):
    ledger = LocalLedger(path=str(tmp_path / "chain.json"))
    for i in range(20):
        ledger.append({"i": i})
    assert ledger.verify_chain()