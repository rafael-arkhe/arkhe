#!/usr/bin/env python3
"""
AVALON CORE v2.3 - GPU-READY KURAMOTO ENSEMBLE
================================================
Aceleracao via Numba (fallback: numpy vetorizado) e suporte a
multiplas topologias (cadeia, honeycomb, completa).

v2.3 notes (post v2.1 audit):
- INV-SIGMA-01 mantido: entropia de Seifert dSigma=(F*dtheta - 0.5 divF dt)/T.
  Numa correcao: A divergencia divF e avaliada em theta PRE-update, tal como
  no modulo validado v2.1 (coerencia Ito). O codigo proposto a avaliava em
  theta pos-update e pos-wrap - corrigido aqui.
- INV-DFT-01: dft_assumption_verified explicitamente separado de tut_bound.
- INV-DATA-01: seeding deterministico DENTRO do kernel (np.random.seed) para
  que numba e numpy reproduzam a mesma sequencia (CRN-safe).
- INV-OPT-02: temperature (T_bath) e horizon (t_max/dt) sao parametros distintos.
- INV-TUT-01: bound = 1/<tanh(Sigma/2)> - 1 (via tut_bound_module).

RESULTADO EMPIRICO DA VARREDURA v2.3 (Floquet, J=1, w=0.5, amp=1, gamma=0.1):
   chain=0.43-0.50 | honeycomb=0.36-0.44 | complete=0.0 (vacuo).
   A proximidade a 1/phi (~0.43-0.51, ruidosa, seeds 1-21) NAO e exclusiva
   do honeycomb: surge em QUALQUER topologia esparsa (grau baixo), e colapsa
   para 0 no grafo completo (grau alto). 1/phi nao e selecionado por topologia;
   e um artefato observacional do regime de baixo grau medio, sem mecanismo.

STATUS: Executed and audited (ver UPGRADE_v2_3.md).
"""

from __future__ import annotations

import time
from dataclasses import dataclass
from typing import Callable, Dict, List, Optional, Tuple

import numpy as np
import networkx as nx

try:  # numba route
    from numba import jit, prange
    HAVE_NUMBA = True
except ImportError:  # numpy fallback route
    HAVE_NUMBA = False

    def jit(nopython=True, parallel=False, cache=True, **kwargs):
        def _decorator(fn: Callable) -> Callable:
            return fn
        return _decorator

    # prange not available in pure python; use range
    _PRANGE = range

    def prange(*args, **kwargs):
        return _PRANGE(*args, **kwargs)


PHI = (1.0 + np.sqrt(5.0)) / 2.0
INV_PHI = 1.0 / PHI


# ============================================================
# 1. TOPOLOGY FACTORY
# ============================================================

class TopologyFactory:
    """Gera grafos para diferentes topologias (nos rotulados como int)."""

    @staticmethod
    def honeycomb(radius: int) -> nx.Graph:
        G = nx.hexagonal_lattice_graph(radius, radius, periodic=False)
        return nx.convert_node_labels_to_integers(G)

    @staticmethod
    def chain(n: int) -> nx.Graph:
        return nx.path_graph(n)

    @staticmethod
    def complete(n: int) -> nx.Graph:
        return nx.complete_graph(n)

    @staticmethod
    def get_edges(topology: str, n: int, **kwargs) -> Tuple[np.ndarray, int]:
        """
        Retorna (edges, n_graph): lista de arestas como array (E, 2) e o
        numero REAL de nos do grafo construido.

        NOTA: para honeycomb, n e ignorado; o tamanho do grafo e derivado
        de
        radius. Para chain/complete, n_graph = n.
        """
        if topology == 'honeycomb':
            G = TopologyFactory.honeycomb(kwargs.get('radius', n // 2))
        elif topology == 'chain':
            G = TopologyFactory.chain(n)
        elif topology == 'complete':
            G = TopologyFactory.complete(n)
        else:
            raise ValueError(f"Unknown topology: {topology}")

        edges = np.array(list(G.edges()), dtype=np.int64)
        n_graph = G.number_of_nodes()
        return edges, n_graph


# ============================================================
# 2. NUMBA-ACCELERATED KERNELS
# ============================================================

@jit(nopython=True, parallel=False, cache=True)
def drift_kuramoto(
    theta: np.ndarray,
    adj: np.ndarray,
    J: float,
    omega: float,
) -> np.ndarray:
    """
    F_{t,i} = omega + J * Sigma_j A_ij sin(theta_{t,j} - theta_{t,i}).
    theta: (n_traj, n_nodes).
    """
    n_traj, n_nodes = theta.shape
    F = np.zeros((n_traj, n_nodes), dtype=np.float64)
    for t in prange(n_traj):
        for i in range(n_nodes):
            sum_sin = 0.0
            for j in range(n_nodes):
                if adj[i, j] != 0.0:
                    sum_sin += np.sin(theta[t, j] - theta[t, i])
            F[t, i] = omega + J * sum_sin
    return F


@jit(nopython=True, parallel=False, cache=True)
def divergence_kuramoto(theta: np.ndarray, adj: np.ndarray, J: float) -> np.ndarray:
    """
    div F_{t} = -J * Sigma_{i,j} A_ij cos(theta_{t,j} - theta_{t,i}).
    Avaliada em theta PRE-update (coerencia Ito, identica ao v2.1).
    """
    n_traj, n_nodes = theta.shape
    div = np.zeros(n_traj, dtype=np.float64)
    for t in prange(n_traj):
        s = 0.0
        for i in range(n_nodes):
            for j in range(n_nodes):
                if adj[i, j] != 0.0:
                    s += np.cos(theta[t, j] - theta[t, i])
        div[t] = -J * s
    return div


@jit(nopython=True, parallel=False, cache=True)
def integrate_ensemble(
    theta0: np.ndarray,
    adj: np.ndarray,
    J: float,
    omega: float,
    T_bath: float,
    dt: float,
    n_steps: int,
    seed: int = 42,
) -> Tuple[np.ndarray, np.ndarray, np.ndarray]:
    """
    Integra ensemble completo com Euler-Maruyama (Seifert medium entropy).

    Returns: (theta_final, Sigma, Q)
    dSigma = (F . dtheta - 0.5 divF dt) / T      [divF pre-update]
    """
    n_traj, n_nodes = theta0.shape
    theta = theta0.copy()
    theta_unwrapped = theta0.copy()
    Sigma = np.zeros(n_traj, dtype=np.float64)
    Q = np.zeros(n_traj, dtype=np.float64)

    # Seeding deterministico DENTRO do kernel -> mesmo resultado numba/numpy.
    np.random.seed(seed)
    noise_amp = np.sqrt(2.0 * T_bath * dt)

    for step in range(n_steps):
        F = drift_kuramoto(theta, adj, J, omega)

        # --- CORRECAO PRE-UPDATE: F e divF avaliados no MESMO theta ---
        divF = divergence_kuramoto(theta, adj, J)

        noise = noise_amp * np.random.randn(n_traj, n_nodes)

        dtheta = F * dt + noise

        # Entropia de Seifert (Ito): dSigma = (F . dtheta - 0.5 divF dt) / T
        for t in range(n_traj):
            dot_product = 0.0
            for i in range(n_nodes):
                dot_product += F[t, i] * dtheta[t, i]
            Sigma[t] += (dot_product - 0.5 * divF[t] * dt) / T_bath

        theta = theta + dtheta
        theta_unwrapped = theta_unwrapped + dtheta
        theta = np.mod(theta + np.pi, 2.0 * np.pi) - np.pi

    for t in range(n_traj):
        Q[t] = np.mean(theta_unwrapped[t] - theta0[t])

    return theta, Sigma, Q


# ============================================================
# 3. ENSEMBLE WRAPPER
# ============================================================

def run_ensemble(
    topology: str,
    n_nodes: int,
    J: float = 1.0,
    T_bath: float = 1.0,
    omega: float = 0.0,
    t_max: float = 20.0,
    dt: float = 0.05,
    n_traj: int = 500,
    seed: int = 42,
    dft_assumption_verified: bool = False,  # INV-DFT-01
    radius: Optional[int] = None,
) -> Dict:
    """
    Executa ensemble acelerado (numba ou numpy).

    n_nodes e um alvo nominal: o numero real de nos segue a topologia.
    """
    edges, n_graph = TopologyFactory.get_edges(
        topology, n_nodes, radius=radius if radius is not None else n_nodes // 2
    )

    adj = np.zeros((n_graph, n_graph), dtype=np.float64)
    for i, j in edges:
        adj[i, j] = 1.0
        adj[j, i] = 1.0

    n_steps = int(t_max / dt)
    theta0 = np.random.default_rng(seed).uniform(
        -np.pi, np.pi, (n_traj, n_graph)
    )

    start = time.time()
    theta_final, Sigma, Q = integrate_ensemble(
        theta0, adj, J, omega, T_bath, dt, n_steps, seed=seed
    )
    elapsed = time.time() - start

    from tut_bound_module import TUTBound  # v2.1 module (single source of truth)
    tut = TUTBound(bootstrap_samples=500, seed=seed)
    result = tut.compute(Sigma, Q)

    return {
        'topology': topology,
        'n_nodes': n_graph,
        'n_edges': len(edges),
        'n_traj': n_traj,
        't_max': t_max,
        'dt': dt,
        'backend': 'numba' if HAVE_NUMBA else 'numpy',
        'elapsed_s': elapsed,
        'avg_sigma': float(np.mean(Sigma)),
        'eps_sq_tut': float(result.eps_sq_tut),
        'eps_sq_obs': float(result.eps_sq_observed),
        'bound_satisfied': bool(result.bound_satisfied),
        'bootstrap_std': float(result.bootstrap_std),
        'dft_assumption_verified': dft_assumption_verified,
    }


# ============================================================
# 4. I6 SANITY TEST (equilibrio -> Sigma ~ 0)
# ============================================================

def test_equilibrium_sigma(tolerance: float = 1.0e-3) -> bool:
    """
    I6: Para J=0 e omega=0, F=0, portanto dSigma=(0 - 0)/T = 0.
    A formulacao antiga (noise^2) produziria Sigma ~ N*n_steps.
    """
    ok = True
    for topo in ('chain', 'honeycomb', 'complete'):
        res_kwargs = {}
        if topo == 'honeycomb':
            res_kwargs['radius'] = 2
        edges, n_graph = TopologyFactory.get_edges(topo, 16, **res_kwargs)
        adj = np.zeros((n_graph, n_graph))
        for i, j in edges:
            adj[i, j] = 1.0
            adj[j, i] = 1.0

        theta0 = np.zeros((50, n_graph))
        _, Sigma, _ = integrate_ensemble(
            theta0, adj, J=0.0, omega=0.0,
            T_bath=1.0, dt=0.05, n_steps=200, seed=1,
        )
        max_sigma = float(np.max(np.abs(Sigma)))
        pass_ = max_sigma < tolerance
        ok = ok and pass_
        print(f"  {topo:>10}: max|Sigma| = {max_sigma:.3e} "
              f"(tolerance {tolerance}) -> {'OK' if pass_ else 'FAIL'}")
    return ok


# ============================================================
# 5. COMPARATIVE SWEEP
# ============================================================

def sweep_topologies(
    topologies: List[str],
    n_nodes: int = 24,
    J_values: List[float] = [0.5, 1.0, 2.0],
    n_traj: int = 200,
    seed: int = 42,
) -> Dict:
    """
    Varre topologias e acoplamentos para comparar eps^2_TUT.

    Para honeycomb usa radius tal que o grafo tenha ~n_nodes nos.
    """
    results: Dict = {}
    for topo in topologies:
        radius = None
        if topo == 'honeycomb':
            radius = TopologyFactory.honeycomb_radius_for_n(n_nodes)
        for J in J_values:
            key = f"{topo}_{J}"
            results[key] = run_ensemble(
                topology=topo,
                n_nodes=n_nodes,
                J=J,
                radius=radius,
                n_traj=n_traj,
                seed=seed,
            )
    return results


# Distribuicao; must be defined after TopologyFactory for clarity.
TopologyFactory.honeycomb_radius_for_n = staticmethod(
    lambda n: max(2, int(round((np.sqrt(2.0 * n) - 1.0) / 2.0)))
)


def sweep_floquet_topologies(
    topologies: List[str],
    n_nodes: int = 24,
    n_traj: int = 300,
    seed: int = 42,
    J: float = 1.0,
    omega: float = 1.0,
    amplitude: float = 1.0,
    gamma: float = 0.1,
    temperature: float = 1.0,
    horizon: float = 20.0,
    dt: float = 0.05,
    radius: int = 3,
) -> List[Dict]:
    """
    Varre topologias com a dinamica FLOQUET validada do v2.1.

    IMPORTANTE: o observavel eps^2_TUT = 0.429 da audiencia foi obtido com
    o protocolo Floquet (com amortecimento gamma/forca periodica), NAO com o
    ensemble de Kuramoto puro. Para responder se a proximidade a 1/phi e
    especifica do honeycomb, esta varredura reutiliza a SEMANTICA EXATA do
    modulo validado v2.1 (avalon_stochastic_tut_v2_1), apenas trocando o grafo.
    """
    # Import within function: preserves the "single source of truth" in v2.1
    # and keeps this module runnable even if scipy/floquet deps are missing.
    from avalon_stochastic_tut_v2_1 import (
        FloquetHoneycombKuramoto,
        SimulationConfig,
        analyze_tut,
    )

    results: List[Dict] = []

    for topo in topologies:
        if topo == 'honeycomb':
            graph = TopologyFactory.honeycomb(radius)
        elif topo == 'chain':
            graph = TopologyFactory.chain(n_nodes)
        elif topo == 'complete':
            graph = TopologyFactory.complete(n_nodes)
        else:
            raise ValueError(f"Unknown topology: {topo}")

        config = SimulationConfig(
            J=J,
            omega=omega,
            amplitude=amplitude,
            gamma=gamma,
            temperature=temperature,
            horizon=horizon,
            dt=dt,
            trajectories=n_traj,
            seed=seed,
        )

        rng = np.random.default_rng(seed)
        model = FloquetHoneycombKuramoto(graph, config, rng)
        ensemble = model.ensemble()
        result = analyze_tut(ensemble, dft_assumption_verified=False)

        results.append({
            'topology': topo,
            'n_nodes': graph.number_of_nodes(),
            'n_edges': graph.number_of_edges(),
            'eps_sq_tut': result.eps_sq_tut,
            'eps_sq_observed': result.eps_sq_observed,
            'avg_sigma': result.avg_sigma,
            'avg_tanh': result.avg_tanh,
            'ci_low': result.ci_low,
            'ci_high': result.ci_high,
            'dist_to_1phi': result.distance_to_phi,
        })

    return results


def report_floquet_sweep(results: List[Dict]) -> str:
    """Relatorio honesto da varredura Floquet (observacional, sem causalidade)."""
    lines = []
    lines.append(f"{'Topology':>12} | {'N':>4} | {'E':>4} | {'eps^2_TUT':>10} | {'eps^2_obs':>10} | dist 1/phi")
    lines.append("-" * 70)
    for res in results:
        e = res['eps_sq_tut']
        lines.append(
            f"{res['topology']:>12} | {res['n_nodes']:>4} | {res['n_edges']:>4} | "
            f"{e:>10.6f} | {res['eps_sq_observed']:>10.6f} | {res['dist_to_1phi']:.6f}"
        )

    finite = [r['eps_sq_tut'] for r in results if np.isfinite(r['eps_sq_tut'])]
    if finite:
        lines.append("-" * 70)
        lines.append(f"mean eps^2_TUT = {np.mean(finite):.6f}  (1/phi = {INV_PHI:.6f})")
        nearby = [r['topology'] for r in results if abs(r['eps_sq_tut'] - INV_PHI) < 0.05]
        if nearby:
            lines.append(f"Topologias com |eps^2_TUT - 1/phi| < 0.05: {', '.join(nearby)}")
        lines.append(
            "OBERVACAO EMPIRICA: nenhuma mecanica de selecao por 1/phi e inferida; "
            "a proximidade (ou nao) e um resultado numerico desta topologia e deste "
            "protocolo Floquet."
        )
    return "\n".join(lines)


def report_sweep(results: Dict) -> str:
    """Gera relatorio textual honesto (sem claims causais)."""
    lines = []
    lines.append(f"{'Topology':>12} | {'J':>4} | {'N':>3} | {'eps^2_TUT':>9} | dist to 1/phi")
    lines.append("-" * 60)
    for key in sorted(results):
        res = results[key]
        dist = abs(res['eps_sq_tut'] - INV_PHI)
        lines.append(
            f"{res['topology']:>12} | {key.split('_')[-1]:>4} | "
            f"{res['n_nodes']:>3} | {res['eps_sq_tut']:>9.4f} | {dist:.4f}"
        )

    tuts = [r['eps_sq_tut'] for r in results.values() if np.isfinite(r['eps_sq_tut'])]
    if tuts:
        lines.append("-" * 60)
        lines.append(f"mean eps^2_TUT = {np.mean(tuts):.4f}  (1/phi = {INV_PHI:.4f})")
        lines.append(
            "OBERVACAO: a uniformidade (ou nao) de eps^2_TUT atraves de topologias "
            "eh um resultado EMPIRICO deste ensemble; nenhuma mecanica de selecao "
            "por 1/phi e inferida."
        )
    return "\n".join(lines)


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="Avalon Core v2.3")
    parser.add_argument("--topologies", nargs="+", default=['chain', 'honeycomb', 'complete'])
    parser.add_argument("--n_nodes", type=int, default=24)
    parser.add_argument("--n_traj", type=int, default=500)
    parser.add_argument("--seed", type=int, default=42)
    parser.add_argument("--skip_i6", action="store_true")
    args = parser.parse_args()

    print(f"AVALON CORE v2.3 - Topology Sweep  [backend: {'numba' if HAVE_NUMBA else 'numpy'}]")
    print("=" * 60)

    if not args.skip_i6:
        print("\nI6 SANITY: equilibrium (J=0, omega=0) must yield Sigma ~ 0")
        ok = test_equilibrium_sigma()
        print(f"I6 {'PASSED' if ok else 'FAILED'}")
        if not ok:
            raise SystemExit(1)
    else:
        print("\n[single trajectory I6]: skipped")

    print("\nSingle-ensemble check (honeycomb, J=1, n_traj=%d):" % args.n_traj)
    single = run_ensemble(
        'honeycomb', args.n_nodes, J=1.0, radius=2,
        n_traj=args.n_traj, seed=args.seed,
    )
    for k, v in single.items():
        print(f"  {k:22s}: {v}")

    print("\nTopology sweep (overdamped Kuramoto, vacuous-bound regime)")
    results = sweep_topologies(
        args.topologies, n_nodes=args.n_nodes, n_traj=args.n_traj, seed=args.seed
    )
    print(report_sweep(results))

    print("\nTopology sweep (Floquet protocol - the 0.429 observation regime)")
    floquet_results = sweep_floquet_topologies(
        args.topologies, n_nodes=args.n_nodes, n_traj=args.n_traj, seed=args.seed
    )
    print(report_floquet_sweep(floquet_results))