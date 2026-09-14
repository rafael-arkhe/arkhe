#!/usr/bin/env python3
"""
AVALON HONEYCOMB KURAMOTO - ENTROPY PRODUCTION v2.1
====================================================
Fase 2 specification: rigorous computation of stochastic entropy
production Sigma for the honeycomb Kuramoto network.

v2.1 CRITICAL FIXES:
- INV-SIGMA-01: Entropy production now uses Seifert medium entropy:
    dSigma = Sum_i F_i(theta) * dtheta_i / T
  NOT the noise quadratic variation (which grows as N*t and makes TUT vacuous).
- INV-DFT-01: Explicit dft_assumption_verified flag.
- Phase unwrapping: Q uses unwrapped phases to preserve time-antisymmetry.
- I6 Sanity Test: J=0 equilibrium simulation must yield Sigma ~= 0.
- INV-OPT-02: temperature and horizon are distinct parameters.

STATUS: Ready for execution.
"""

from __future__ import annotations

import numpy as np
import networkx as nx
from typing import Tuple, Optional
from dataclasses import dataclass


@dataclass
class KuramotoResult:
    """Results from a single trajectory."""
    Q: float
    Sigma: float
    theta_final: np.ndarray
    trajectory: Optional[np.ndarray] = None


class HoneycombKuramotoSigma:
    """
    Honeycomb network of Kuramoto oscillators with thermal noise.
    Computes the physical entropy production from the Langevin dynamics.

    Hamiltonian:
        H = -J Sum_<i,j> cos(theta_j - theta_i) + Sum_i omega_i theta_i

    Dynamics (overdamped Langevin):
        dtheta_i = [omega_i + J Sum_j A_ij sin(theta_j - theta_i)] dt + sqrt(2T) dW_i

    Entropy production (Seifert, medium entropy):
        dSigma = (1/T) Sum_i F_i(theta) * dtheta_i

    where F_i(theta) = omega_i + J Sum_j A_ij sin(theta_j - theta_i) is the deterministic drift.

    CRITICAL: For J=0 (equilibrium), F=0, therefore dSigma=0.
    The old formula noise^2/(2T*dt) would give dSigma = N per step regardless of J.
    """

    def __init__(
        self,
        graph: nx.Graph,
        J: float = 1.0,
        temperature: float = 1.0,  # INV-OPT-02: renamed from T
        omega_std: float = 0.1,
        seed: Optional[int] = None,
    ):
        self.graph = graph
        self.N = graph.number_of_nodes()
        self.adj = nx.to_numpy_array(graph)
        self.J = J
        self.temperature = max(temperature, 1e-6)
        self.rng = np.random.default_rng(seed)
        self.omega = self.rng.normal(0, omega_std, self.N)

    def drift(self, theta: np.ndarray) -> np.ndarray:
        """Compute deterministic drift term F_i(theta)."""
        diff = theta[None, :] - theta[:, None]
        coupling = self.J * np.sum(self.adj * np.sin(diff), axis=1)
        return self.omega + coupling

    def divergence_drift(self, theta: np.ndarray) -> float:
        """
        Compute div*F for Ito correction.
        For Kuramoto: div*F = -J Sum_{i,j} A_ij cos(theta_j - theta_i)
        """
        diff = theta[None, :] - theta[:, None]
        return -self.J * float(np.sum(self.adj * np.cos(diff)))

    def run_trajectory(
        self,
        theta0: np.ndarray,
        t_max: float = 10.0,      # horizon (simulation time)
        dt: float = 0.05,
        store_trajectory: bool = False,
    ) -> KuramotoResult:
        """
        Run a single Langevin trajectory and compute Sigma.

        Uses Euler-Maruyama with Ito correction for entropy production:
            dSigma = (1/T) [Sum_i F_i * dtheta_i - (1/2)(div*F) dt]
        """
        n_steps = int(t_max / dt)
        theta = theta0.copy()
        theta_unwrapped = theta0.copy()  # Track unwrapped for Q
        Sigma = 0.0

        traj = np.zeros((n_steps, self.N)) if store_trajectory else None
        if store_trajectory:
            traj[0] = theta

        noise_factor = np.sqrt(2 * self.temperature * dt)

        for i in range(n_steps - 1):
            F = self.drift(theta)
            noise = noise_factor * self.rng.standard_normal(self.N)

            # Update
            dtheta = F * dt + noise
            theta_new = theta + dtheta

            # Wrap for dynamics (mod 2*pi)
            theta_wrapped = np.mod(theta_new + np.pi, 2 * np.pi) - np.pi

            # Track unwrapped for Q (preserves winding number)
            theta_unwrapped = theta_unwrapped + dtheta

            # --- CORRECAO I6: Entropia de meio (Seifert) ---
            # dSigma = (1/T) [F * dtheta - (1/2)(div*F) dt]   (Ito form)
            dot_product = np.sum(F * dtheta)
            div_F = self.divergence_drift(theta)
            dSigma = (dot_product - 0.5 * div_F * dt) / self.temperature
            Sigma += dSigma
            # -------------------------------------------------

            theta = theta_wrapped
            if store_trajectory:
                traj[i + 1] = theta

        # Charge: unwrapped mean phase displacement (time-antisymmetric)
        Q = np.mean(theta_unwrapped - theta0)

        return KuramotoResult(
            Q=Q,
            Sigma=Sigma,
            theta_final=theta,
            trajectory=traj,
        )

    def ensemble(
        self,
        n_traj: int = 100,
        t_max: float = 10.0,
        dt: float = 0.05,
        seed: int = 42,
    ) -> Tuple[np.ndarray, np.ndarray]:
        """Run ensemble of trajectories."""
        self.rng = np.random.default_rng(seed)
        Q_vals = []
        Sigma_vals = []

        for _ in range(n_traj):
            theta0 = self.rng.uniform(-np.pi, np.pi, self.N)
            result = self.run_trajectory(theta0, t_max, dt)
            Q_vals.append(result.Q)
            Sigma_vals.append(result.Sigma)

        return np.array(Q_vals), np.array(Sigma_vals)


def build_honeycomb(radius: int = 2) -> nx.Graph:
    """Build a finite honeycomb graph."""
    G = nx.hexagonal_lattice_graph(radius, radius, periodic=False)
    G = nx.convert_node_labels_to_integers(G)
    return G


def compute_tut_for_honeycomb(
    J: float = 1.0,
    temperature: float = 1.0,
    t_max: float = 10.0,
    dt: float = 0.05,
    n_traj: int = 200,
    seed: int = 42,
    dft_assumption_verified: bool = False,  # INV-DFT-01
) -> dict:
    """
    Full pipeline: build honeycomb -> simulate ensemble -> compute TUT.
    """
    from tut_bound_module import TUTBound

    G = build_honeycomb(radius=2)
    model = HoneycombKuramotoSigma(G, J=J, temperature=temperature, seed=seed)
    Q_vals, Sigma_vals = model.ensemble(n_traj=n_traj, t_max=t_max, dt=dt, seed=seed)

    tut = TUTBound(bootstrap_samples=500)
    result = tut.compute(Sigma_vals, Q_vals)

    return {
        "graph_nodes": G.number_of_nodes(),
        "graph_edges": G.number_of_edges(),
        "J": J,
        "temperature": temperature,
        "t_max": t_max,
        "dt": dt,
        "n_traj": n_traj,
        "eps_sq_tut": result.eps_sq_tut,
        "eps_sq_observed": result.eps_sq_observed,
        "avg_sigma": result.avg_sigma,
        "bound_satisfied": result.bound_satisfied,
        "bootstrap_std": result.bootstrap_std,
        "dft_assumption_verified": dft_assumption_verified,
    }


# =============================================================================
# I6 SANITY TEST: Equilibrium must yield Sigma ~= 0
# =============================================================================
def test_i6_equilibrium_entropy() -> bool:
    """
    INV-SIGMA-01 / I6 Sanity Test:
    For J=0, omega=0, the system is in equilibrium. The drift F=0, therefore
    the medium entropy production must be approximately zero.

    The old (buggy) formula would give Sigma ~ N * t / dt (thousands).
    """
    G = build_honeycomb(radius=2)
    N = G.number_of_nodes()
    model = HoneycombKuramotoSigma(
        graph=G,
        J=0.0,           # No coupling -> equilibrium
        temperature=1.0,
        omega_std=0.0,   # No frequency disorder -> equilibrium
        seed=42,
    )

    # Run single trajectory
    theta0 = np.zeros(N)
    result = model.run_trajectory(theta0, t_max=10.0, dt=0.05)

    print("=" * 60)
    print("I6 SANITY TEST: Equilibrium Entropy Production")
    print("=" * 60)
    print(f"Nodes (N):        {N}")
    print(f"Steps:            {int(10.0 / 0.05)}")
    print(f"J:                0.0 (equilibrium)")
    print(f"omega_std:        0.0 (equilibrium)")
    print(f"Sigma computed:   {result.Sigma:.6f}")
    print(f"Expected:         ~ 0 (equilibrium)")

    if abs(result.Sigma) < 1.0:
        print("I6 TEST PASSED - Equilibrium yields Sigma ~ 0")
        return True
    else:
        print("I6 TEST FAILED - Equilibrium yields large Sigma (bug detected)")
        return False


if __name__ == "__main__":
    print("Avalon Honeycomb Kuramoto - Entropy Production v2.1")
    print("=" * 55)

    # Run I6 sanity test first
    i6_passed = test_i6_equilibrium_entropy()
    print()

    if not i6_passed:
        print("ABORTING: I6 sanity test failed. Do not use this module for TUT computation.")
        exit(1)

    # Example: non-equilibrium run
    print("Running non-equilibrium example (J=1.0, T=1.0, n_traj=500)...")
    results = compute_tut_for_honeycomb(J=1.0, temperature=1.0, n_traj=500)
    print()
    for key, val in results.items():
        if isinstance(val, float):
            print(f"  {key:25s}: {val:.6f}")
        else:
            print(f"  {key:25s}: {val}")