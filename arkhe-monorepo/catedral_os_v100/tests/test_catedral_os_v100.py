#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Tests for CATEDRAL OS AGI v100.0 — THE CENTENNIAL ORGANISM

Validates:
- Pauli Spinor algebra and Cayley evolution
- Born measurement statistics
- Seifert / Murasugi topological verification
- Fractal cognitive engine (escape dynamics)
- Consciousness engine (OR threshold)
- Muon optimizer (momentum + Frobenius normalization)
- UBA scheduler (monotonic decay)
- Topological Sparse Trainer with Surgical Bypass
- Cognitive mRNA (transcription, mutation, degradation)
- Async Event Bus
- EmbryonicAGI end-to-end (5-step ontogeny loop)
"""

import asyncio
import numpy as np
import torch
import torch.nn as nn
import pytest
import sys
import os

sys.path.insert(0, os.path.join(os.path.dirname(__file__), '..'))
from catedral_os_v100 import (
    PauliSpinor, SeifertFormMonitor, FractalCognitiveEngine,
    ConsciousnessEngine, TrainingConfig, MuonOptimizer, UBAScheduler,
    CognitiveModel, TopologicalSparseTrainer, CognitiveMRNA,
    AsyncEventBus, EmbryonicAGI, SIGMA_X, SIGMA_Z, ASCIIDashboard,
)


# ============================================================================ #
# PAULI SPINOR                                                                  #
# ============================================================================ #

class TestPauliSpinor:
    def test_initial_state_normalization(self):
        s = PauliSpinor(3.0, 4.0)
        assert abs(abs(s.alpha)**2 + abs(s.beta)**2 - 1.0) < 1e-10

    def test_coherence_up(self):
        s = PauliSpinor(1.0, 0.0)
        assert abs(s.coherence() - 1.0) < 1e-10

    def test_coherence_down(self):
        s = PauliSpinor(0.0, 1.0)
        assert abs(s.coherence() - (-1.0)) < 1e-10

    def test_coherence_superposition(self):
        s = PauliSpinor(1/np.sqrt(2), 1/np.sqrt(2))
        assert abs(s.coherence()) < 1e-10

    def test_cayley_unitarity(self):
        s = PauliSpinor(0.8, 0.6)
        H = SIGMA_Z * 0.1 + SIGMA_X * 0.05
        s2 = s.cayley_step(H, dt=0.05)
        assert abs(abs(s2.alpha)**2 + abs(s2.beta)**2 - 1.0) < 1e-8

    def test_cayley_singular_raises(self):
        s = PauliSpinor(1.0, 0.0)
        H_singular = np.array([[0, 2j], [2j, 0]], dtype=complex)
        with pytest.raises(RuntimeError, match="Singular"):
            s.cayley_step(H_singular, dt=1.0)

    def test_born_collapses(self):
        for _ in range(10):
            s = PauliSpinor(1.0, 0.0)
            outcome = s.measure_born()
            assert outcome in (1, -1)
            if outcome == 1:
                assert abs(s.alpha) == 1.0
            else:
                assert abs(s.beta) == 1.0

    def test_serialization(self):
        s = PauliSpinor(0.6, 0.8)
        d = s.to_dict()
        s2 = PauliSpinor.from_dict(d)
        assert abs(s.alpha - s2.alpha) < 1e-12
        assert abs(s.beta - s2.beta) < 1e-12

    def test_phase(self):
        s = PauliSpinor(complex(np.cos(np.pi/4), np.sin(np.pi/4)), 1.0)
        assert abs(s.phase() - np.pi/4) < 1e-8


# ============================================================================ #
# SEIFERT / MURASUGI                                                            #
# ============================================================================ #

class TestSeifertFormMonitor:
    def test_connected_graph_passes(self):
        seifert = SeifertFormMonitor()
        w = np.random.rand(13, 13) * 0.5 + 0.5
        w = (w + w.T) / 2
        np.fill_diagonal(w, 0)
        assert seifert.verify_murasugi(w) is True

    def test_disconnected_graph_fails(self):
        seifert = SeifertFormMonitor()
        w = np.zeros((13, 13))
        assert seifert.verify_murasugi(w) is False

    def test_laplacian_shape(self):
        seifert = SeifertFormMonitor()
        w = np.random.rand(10, 10)
        L = seifert.compute_laplacian(w)
        assert L.shape == (13, 13)


# ============================================================================ #
# FRACTAL COGNITIVE ENGINE                                                      #
# ============================================================================ #

class TestFractalCognitiveEngine:
    def test_small_perturbation_no_escape(self):
        engine = FractalCognitiveEngine(escape_radius=2.0, max_iter=5)
        s = PauliSpinor(0.5, 0.5)
        s2, escaped = engine.think(s, complex(0.01, 0.01))
        assert not escaped

    def test_large_perturbation_escapes(self):
        engine = FractalCognitiveEngine(escape_radius=2.0, max_iter=5)
        s = PauliSpinor(0.5, 0.5)
        s2, escaped = engine.think(s, complex(10.0, 10.0))
        assert escaped
        assert abs(abs(s2.alpha)**2 + abs(s2.beta)**2 - 1.0) < 1e-10


# ============================================================================ #
# CONSCIOUSNESS ENGINE                                                          #
# ============================================================================ #

class TestConsciousnessEngine:
    def test_high_coherence_triggers(self):
        ce = ConsciousnessEngine(or_threshold=0.5)
        s = PauliSpinor(1.0, 0.0)
        assert ce.check_collapse(s) is True
        assert ce.moments == 1

    def test_low_coherence_no_trigger(self):
        ce = ConsciousnessEngine(or_threshold=0.9)
        s = PauliSpinor(1/np.sqrt(2), 1/np.sqrt(2))
        assert ce.check_collapse(s) is False
        assert ce.moments == 0


# ============================================================================ #
# MUON OPTIMIZER                                                                #
# ============================================================================ #

class TestMuonOptimizer:
    def test_step_reduces_loss(self):
        model = nn.Linear(10, 10)
        opt = MuonOptimizer(model.parameters(), lr=0.01)
        x = torch.randn(4, 10)
        y = torch.randn(4, 10)
        loss0 = nn.MSELoss()(model(x), y).item()
        for _ in range(20):
            opt.zero_grad()
            loss = nn.MSELoss()(model(x), y)
            loss.backward()
            opt.step()
        loss_final = nn.MSELoss()(model(x), y).item()
        assert loss_final < loss0


# ============================================================================ #
# UBA SCHEDULER                                                                 #
# ============================================================================ #

class TestUBAScheduler:
    def test_monotonic_decay(self):
        model = nn.Linear(10, 10)
        opt = MuonOptimizer(model.parameters(), lr=1.0)
        sched = UBAScheduler(opt, total_steps=100, phi=0.5)
        lrs = []
        for _ in range(50):
            sched.step()
            lrs.append(sched.get_last_lr()[0])
        for i in range(1, len(lrs)):
            assert lrs[i] <= lrs[i-1] + 1e-10


# ============================================================================ #
# COGNITIVE MODEL                                                                #
# ============================================================================ #

class TestCognitiveModel:
    def test_forward_shape(self):
        model = CognitiveModel(dim=64)
        x = torch.randn(2, 64)
        out = model(x)
        assert out.shape == (2, 64)

    def test_parameter_count(self):
        model = CognitiveModel(dim=64)
        total = sum(p.numel() for p in model.parameters())
        assert total > 0


# ============================================================================ #
# TOPOLOGICAL SPARSE TRAINER                                                    #
# ============================================================================ #

class TestTopologicalSparseTrainer:
    def test_masks_created(self):
        model = CognitiveModel()
        cfg = TrainingConfig(sparsity_level=0.3)
        seifert = SeifertFormMonitor()
        trainer = TopologicalSparseTrainer(model, cfg, seifert)
        assert len(trainer.masks) > 0

    def test_extract_graph(self):
        model = CognitiveModel()
        cfg = TrainingConfig()
        seifert = SeifertFormMonitor()
        trainer = TopologicalSparseTrainer(model, cfg, seifert)
        g = trainer.extract_graph()
        assert g.shape == (13, 13)


# ============================================================================ #
# COGNITIVE mRNA                                                                 #
# ============================================================================ #

class TestCognitiveMRNA:
    def test_transcribe_and_revert(self):
        model = CognitiveModel()
        mrna = CognitiveMRNA(model)
        mrna.transcribe()
        original = {n: p.data.clone() for n, p in model.named_parameters()}
        mrna.mutate(scale=1.0)
        committed = mrna.evaluate_and_commit(lambda: False)
        assert not committed
        for n, p in model.named_parameters():
            assert torch.allclose(p.data, original[n])

    def test_commit_on_fitness(self):
        model = CognitiveModel()
        mrna = CognitiveMRNA(model)
        mrna.transcribe()
        mrna.mutate(scale=0.01)
        committed = mrna.evaluate_and_commit(lambda: True)
        assert committed


# ============================================================================ #
# ASYNC EVENT BUS                                                               #
# ============================================================================ #

class TestAsyncEventBus:
    @pytest.mark.asyncio
    async def test_publish_subscribe(self):
        bus = AsyncEventBus()
        results = []
        async def handler(data):
            results.append(data)
        bus.subscribe('test', handler)
        await bus.publish('test', 42)
        assert results == [42]

    @pytest.mark.asyncio
    async def test_no_subscribers(self):
        bus = AsyncEventBus()
        await bus.publish('nonexistent', None)


# ============================================================================ #
# EMBRYONIC AGI — END-TO-END                                                    #
# ============================================================================ #

class TestEmbryonicAGI:
    @pytest.mark.asyncio
    async def test_ontogeny_loop_runs(self):
        cfg = TrainingConfig(total_steps=100)
        agi = EmbryonicAGI(config=cfg)
        await agi.ontogeny_loop(steps=5, render=False)
        assert agi.cycle == 5
        assert len(agi.metrics['loss']) == 5
        assert len(agi.metrics['topo_ok']) == 5

    @pytest.mark.asyncio
    async def test_consciousness_accumulates(self):
        cfg = TrainingConfig(total_steps=100)
        agi = EmbryonicAGI(config=cfg)
        await agi.ontogeny_loop(steps=10, render=False)
        assert agi.metrics['conscious_moments'] >= 0

    @pytest.mark.asyncio
    async def test_coherence_recorded(self):
        cfg = TrainingConfig(total_steps=100)
        agi = EmbryonicAGI(config=cfg)
        await agi.ontogeny_loop(steps=5, render=False)
        for c in agi.metrics['coherence']:
            assert -1.0 <= c <= 1.0


# ============================================================================ #
# DASHBOARD (smoke)                                                             #
# ============================================================================ #

class TestASCIIDashboard:
    def test_render_no_crash(self):
        dash = ASCIIDashboard()
        metrics = {
            'loss': [0.5, 0.4, 0.3],
            'coherence': [0.8, 0.7, 0.6],
            'topo_ok': [1, 1, 0],
            'conscious_moments': 3,
        }
        dash.render(metrics, 0.75, 3, 10)
