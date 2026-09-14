#!/usr/bin/env python3
"""
Testes automatizados para Substrato 256 v3.0 (pytest) — Emergent Dynamics Advanced.
"""
import numpy as np
import pytest
from substrate_256 import (
    VARFitter,
    StateSpaceModel,
    GrassmannOptimizer,
    StructureFragmentationAnalyzer,
    BootstrapAnalyzer,
    Substrate256EmergentDynamicsV3,
)


def make_synthetic(T=120, N=6):
    """Gera dados sintéticos multivariados estáveis."""
    np.random.seed(7)
    A1 = np.random.randn(N, N) * 0.08
    A1 = A1 / (np.max(np.abs(np.linalg.eigvals(A1))) * 1.5)
    data = np.zeros((T, N))
    for t in range(1, T):
        data[t] = A1 @ data[t - 1] + 0.1 * np.random.randn(N)
    return data


@pytest.fixture
def data():
    return make_synthetic()


@pytest.fixture
def var_model(data):
    fitter = VARFitter(max_lag=4, criterion='aic')
    return fitter.fit(data)


@pytest.fixture
def ss_model(var_model):
    return VARFitter(max_lag=4, criterion='aic').to_state_space(var_model)


def test_var_fit_shapes(var_model):
    assert var_model.A.shape[1:] == (var_model.n, var_model.n)
    assert var_model.V.shape == (var_model.n, var_model.n)


def test_state_space_conversion(var_model):
    ss = VARFitter().to_state_space(var_model)
    assert ss.A_ss.shape == (ss.r, ss.r)
    assert ss.C.shape == (var_model.n, ss.r)
    assert ss.K.shape == (ss.r, var_model.n)
    assert ss.r == var_model.n * var_model.p


def test_grassmann_optimizer_single_run(ss_model):
    opt = GrassmannOptimizer(ss_model, n_macro=2, n_runs=1, max_iter=10)
    macro = opt.optimize_single_run(0)
    assert macro.M.shape == (2, ss_model.n)
    # M deve ser ortonormal: M M^T == I
    MmT = macro.M @ macro.M.T
    assert np.allclose(MmT, np.eye(2), atol=1e-4)
    assert np.isfinite(macro.dd)
    assert macro.di == pytest.approx(1.0 / (1.0 + abs(macro.dd)))
    assert len(macro.source_weights) == ss_model.n


def test_similarity_matrix_symmetric(ss_model):
    opt = GrassmannOptimizer(ss_model, n_macro=2, n_runs=5, max_iter=5)
    macros = opt.optimize()
    sim = StructureFragmentationAnalyzer.compute_similarity_matrix(macros)
    assert sim.shape == (5, 5)
    assert np.allclose(sim, sim.T)
    assert np.all((sim >= 0) & (sim <= 1 + 1e-9))


def test_sf_index_adaptive():
    sim = np.array([
        [1.0, 0.2, 0.3],
        [0.2, 1.0, 0.9],
        [0.3, 0.9, 1.0],
    ])
    sf, thr = StructureFragmentationAnalyzer.compute_sf_index(sim)
    assert isinstance(sf, float)
    assert isinstance(thr, float)
    # threshold adaptativo é o 5º percentil da triângulo superior
    upper = sim[np.triu_indices(3, k=1)]
    assert thr == np.percentile(upper, 5)


def test_bootstrap_dd():
    class FakeMacro:
        def __init__(self, dd):
            self.dd = dd

    macros = [FakeMacro(float(d)) for d in np.random.randn(50) * 0.1 + 5.0]
    res = BootstrapAnalyzer.bootstrap_dd(macros, n_bootstrap=200, ci=0.95)
    assert res.ci_lower <= res.mean <= res.ci_upper
    assert res.n_bootstrap == 200
    assert res.std >= 0


def test_analyze_and_sweet_spot(data):
    substrate = Substrate256EmergentDynamicsV3()
    results = substrate.analyze(data, scales=[2, 3], n_runs=3, max_iter=5)
    assert set(results.keys()) == {2, 3}
    assert substrate.current_sweet_spot in (2, 3)
    assert substrate.sweet_spot_score == pytest.approx(
        max(
            results[s]['sf_index'] * (1.0 - results[s]['mean_dd'] / (1.0 + abs(results[s]['mean_dd'])))
            for s in (2, 3)
        )
    )
    report = substrate.get_sweet_spot_report()
    assert report['sweet_spot_scale'] == substrate.current_sweet_spot


def test_rsi_fitness(data):
    # get_rsi_fitness usa n_runs=15/max_iter=200 por padrão; para um teste
    # rápido usamos dados minúsculos (N=3) — get_rsi_fitness escala com
    # min(8, N), reduzindo o custo de forma drástica.
    N = 3
    np.random.seed(7)
    A1 = np.random.randn(N, N) * 0.08
    A1 = A1 / (np.max(np.abs(np.linalg.eigvals(A1))) * 1.5)
    small = np.zeros((60, N))
    for t in range(1, 60):
        small[t] = A1 @ small[t - 1] + 0.1 * np.random.randn(N)

    substrate = Substrate256EmergentDynamicsV3()
    fitness = substrate.get_rsi_fitness(small)
    assert np.isfinite(fitness)
    assert fitness == substrate.sweet_spot_score


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
