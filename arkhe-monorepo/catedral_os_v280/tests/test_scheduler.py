# catedral_os_v280/tests/test_scheduler.py
"""Testes do scheduler de multiplos tuneis paralelos (L4)."""

from core.decider import Decider
from core.state import SystemState
from persistence.ipfs_ledger import IPFSLedger
from scheduler.multi_tunnel import MultiTunnelScheduler
from validation.lean_validator import MiniLeanValidator


def test_scheduler_step_all_sync():
    ledger = IPFSLedger(use_ipfs=False, persist=False)
    validator = MiniLeanValidator()
    scheduler = MultiTunnelScheduler(num_tunnels=3, seed=7)
    for tid in range(3):
        decider = Decider(ledger=ledger, validator=validator,
                          phi_star=0.85, seed=10 + tid, episode=tid)
        scheduler.add_worker(decider, SystemState(phi_star=0.85))
    assert scheduler.count == 3

    for _ in range(20):
        scheduler.step_all(1)
    summary = scheduler.get_summary()
    assert summary["num_tunnels"] == 3
    assert summary["total_actions"] == 60
    assert ledger.verify_chain()


def test_scheduler_threads_share_ledger():
    ledger = IPFSLedger(use_ipfs=False, persist=False)
    validator = MiniLeanValidator()
    scheduler = MultiTunnelScheduler(num_tunnels=2, seed=5)
    for tid in range(2):
        decider = Decider(ledger=ledger, validator=validator,
                          phi_star=0.85, seed=20 + tid, episode=tid)
        scheduler.add_worker(decider, SystemState(phi_star=0.85))

    scheduler.start_all()
    import time

    time.sleep(0.5)
    scheduler.stop_all()

    assert scheduler.get_summary()["num_tunnels"] == 2
    assert ledger.verify_chain()
    assert ledger.stats()["entries"] > 0


def test_scheduler_worker_stops_cleanly():
    scheduler = MultiTunnelScheduler(num_tunnels=1, seed=1)
    scheduler.add_worker(Decider(ledger=IPFSLedger(use_ipfs=False, persist=False),
                                 seed=1), SystemState())
    scheduler.start_all()
    scheduler.stop_all()
    assert not any(w.running for w in scheduler.workers)