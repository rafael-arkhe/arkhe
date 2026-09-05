# catedral_os_v152/tests/test_temporal_chain.py
"""Testes da cadeia temporal (I190.I192) com decaimento."""

import math

import pytest

from core.entropy import constructive_entropy, spectral_entropy
from core.temporal_chain import Handover, TemporalChain


def test_handover_hash_is_stable():
    h1 = Handover(phi=0.5, source="Z1T", timestamp=123.0)
    h2 = Handover(phi=0.5, source="Z1T", timestamp=123.0)
    assert h1.hash == h2.hash


def test_chain_accumulates_and_achieves_target():
    chain = TemporalChain(target_phi=0.95, decay_rate=0.0)  # sem decaimento
    assert chain.add_block(Handover(phi=0.5, source="TCameraS3", timestamp=1.0))
    assert chain.add_block(Handover(phi=0.5, source="Z1T", timestamp=2.0))
    assert chain._achieved
    proof = chain.to_proof()
    assert proof["total"] == pytest.approx(1.0)
    assert chain.verify_proof(proof)


def test_proof_verification_rejects_short():
    chain = TemporalChain(target_phi=0.95, decay_rate=0.0)
    chain.add_block(Handover(phi=0.2, source="TCameraS3", timestamp=1.0))
    proof = chain.to_proof()
    assert not chain.verify_proof(proof)


def test_persistence_roundtrip(tmp_path):
    chain = TemporalChain(target_phi=0.9, decay_rate=0.001)
    chain.add_block(Handover(phi=0.5, source="Z1T", timestamp=1.0))
    chain.add_block(Handover(phi=0.5, source="CatedralOS", timestamp=2.0))
    path = str(tmp_path / "chain.json")
    chain.save(path)

    loaded = TemporalChain()
    assert loaded.load(path)
    assert loaded.target_phi == pytest.approx(0.9)
    assert len(loaded.blocks) == 2
    assert loaded.get_coherence_by_origin("Z1T") > 0.0


def test_entropy_finite_and_positive():
    spectrum = [0.3, 0.3, 0.4]
    e = constructive_entropy(spectrum, n_total=8, n_handovers=4)
    assert math.isfinite(e)
    assert e > 0.0


def test_spectral_entropy_uniform():
    # 4 estados uniformes + log2(16) = 2 + 4 = 6
    assert spectral_entropy([0.25, 0.25, 0.25, 0.25], 16) == pytest.approx(6.0)