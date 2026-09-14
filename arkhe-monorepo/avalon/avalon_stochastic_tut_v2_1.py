#!/usr/bin/env python3
"""
AVALON STOCHASTIC TUT v2.1
============================

Stochastic honeycomb Kuramoto network with periodic Floquet driving.

v2.1 Changes:
- Common random numbers (CRN) for differential_evolution to prevent
  optimization of Monte Carlo noise.
- Explicit DFT assumption flag (INV-DFT-01).
- Temperature/horizon separation (INV-OPT-02).
- Objective is eps_sq_TUT alone, NO phi-forcing (INV-OPT-01).
- Post-optimization validation with independent seeds.
- WARNING: Sigma = beta*(W - DeltaF) is operational approximation.
  Full physical certification requires partition-function DeltaF.

Important:
    This module distinguishes:
      - stochastic dynamics,
      - protocol work,
      - entropy production,
      - TUT computation,
      - optimization,
      - phi comparison.

    It does NOT assume that a generic Kuramoto process automatically
    satisfies the Detailed Fluctuation Theorem.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Optional

import numpy as np
import networkx as nx
from scipy.optimize import differential_evolution


PHI = (1.0 + np.sqrt(5.0)) / 2.0
INV_PHI = 1.0 / PHI


@dataclass(frozen=True)
class SimulationConfig:
    """Immutable physical and numerical configuration."""

    J: float
    omega: float
    amplitude: float
    gamma: float
    temperature: float

    horizon: float = 20.0
    dt: float = 0.05
    trajectories: int = 1000

    honeycomb_radius: int = 2
    seed: int = 42

    @property
    def beta(self) -> float:
        if self.temperature <= 0.0:
            raise ValueError("temperature must be positive")
        return 1.0 / self.temperature


@dataclass
class EnsembleResult:
    """Raw simulation results."""

    sigma: np.ndarray
    charge: np.ndarray
    work: np.ndarray
    delta_free_energy: float

    @property
    def n(self) -> int:
        return len(self.sigma)


@dataclass
class TUTResult:
    """TUT analysis result."""

    eps_sq_tut: float
    eps_sq_observed: float
    avg_sigma: float
    avg_tanh: float

    ci_low: float
    ci_high: float

    dft_assumption_verified: bool

    phi_reference: float = INV_PHI

    @property
    def distance_to_phi(self) -> float:
        return abs(self.eps_sq_tut - self.phi_reference)


def build_honeycomb(radius: int) -> nx.Graph:
    """Construct a finite non-periodic honeycomb graph."""

    if radius < 1:
        raise ValueError("radius must be >= 1")

    graph = nx.hexagonal_lattice_graph(
        radius,
        radius,
        periodic=False,
    )

    return nx.convert_node_labels_to_integers(graph)


class FloquetHoneycombKuramoto:
    """
    Stochastic periodically driven Kuramoto network.

    Dynamics:
        dtheta_i =
            [omega + J Sum_j A_ij sin(theta_j - theta_i)
             - gamma theta_i
             + A sin(Omega t)] dt
            + sqrt(2T) dW_i

    This is a classical stochastic phase model.
    It is not by itself a quantum-mechanical model.
    """

    def __init__(
        self,
        graph: nx.Graph,
        config: SimulationConfig,
        rng: np.random.Generator,
    ):
        self.graph = graph
        self.config = config
        self.rng = rng

        self.n = graph.number_of_nodes()
        self.adj = nx.to_numpy_array(graph, dtype=float)

    def protocol(self, time: float) -> float:
        """Periodic external control."""

        return self.config.amplitude * np.sin(
            self.config.omega * time
        )

    def drift(self, theta: np.ndarray, time: float) -> np.ndarray:
        """Deterministic phase drift."""

        phase_difference = theta[None, :] - theta[:, None]

        coupling = (
            self.config.J
            * np.sum(
                self.adj * np.sin(phase_difference),
                axis=1,
            )
        )

        damping = -self.config.gamma * theta

        drive = self.protocol(time)

        return (
            self.config.omega
            + coupling
            + damping
            + drive
        )

    def potential(
        self,
        theta: np.ndarray,
        time: float,
    ) -> float:
        """
        Effective instantaneous potential.

        The protocol-dependent contribution is represented explicitly,
        allowing protocol work to be accumulated.
        """

        interaction = 0.0

        for i, j in self.graph.edges():
            interaction -= self.config.J * np.cos(
                theta[j] - theta[i]
            )

        confinement = (
            0.5
            * self.config.gamma
            * np.sum(theta ** 2)
        )

        drive = self.protocol(time)

        coupling_to_protocol = -drive * np.sum(theta)

        intrinsic_frequency = (
            self.config.omega * np.sum(theta)
        )

        return (
            interaction
            + confinement
            + coupling_to_protocol
            + intrinsic_frequency
        )

    def run_trajectory(
        self,
        theta0: np.ndarray,
    ) -> tuple[float, float, float]:
        """
        Run one Euler-Maruyama trajectory.

        Returns:
            charge,
            protocol work,
            final energy.
        """

        steps = int(
            round(self.config.horizon / self.config.dt)
        )

        theta = np.array(theta0, dtype=float, copy=True)
        theta_unwrapped = theta.copy()  # Track unwrapped for Q

        work = 0.0
        initial_energy = self.potential(theta, 0.0)

        previous_protocol = self.protocol(0.0)

        for step in range(steps):
            time = step * self.config.dt

            current_protocol = self.protocol(time)

            drift = self.drift(theta, time)

            noise = (
                np.sqrt(
                    2.0
                    * self.config.temperature
                    * self.config.dt
                )
                * self.rng.standard_normal(self.n)
            )

            dtheta = drift * self.config.dt + noise
            theta_new = theta + dtheta
            theta_unwrapped = theta_unwrapped + dtheta

            # Wrap for dynamics
            theta = np.mod(theta_new + np.pi, 2 * np.pi) - np.pi

            next_time = time + self.config.dt
            next_protocol = self.protocol(next_time)

            # Midpoint approximation to protocol work:
            # dW = dH/dL * dL
            delta_protocol = next_protocol - previous_protocol

            dH_dlambda = -np.sum(theta)

            work += dH_dlambda * delta_protocol

            previous_protocol = next_protocol

        final_energy = self.potential(
            theta,
            self.config.horizon,
        )

        charge = np.mean(theta_unwrapped - theta0)

        return charge, work, final_energy

    def ensemble(self) -> EnsembleResult:
        """Generate the independent trajectory ensemble."""

        sigma = np.empty(self.config.trajectories)
        charge = np.empty(self.config.trajectories)
        work = np.empty(self.config.trajectories)

        initial_states = self.rng.uniform(
            -np.pi,
            np.pi,
            size=(self.config.trajectories, self.n),
        )

        initial_energy = np.mean(
            [
                self.potential(theta, 0.0)
                for theta in initial_states
            ]
        )

        final_energy_values = np.empty(
            self.config.trajectories
        )

        for index, theta0 in enumerate(initial_states):
            q, w, final_energy = self.run_trajectory(theta0)

            charge[index] = q
            work[index] = w
            final_energy_values[index] = final_energy

        delta_free_energy = (
            np.mean(final_energy_values)
            - initial_energy
        )

        # Operational approximation: Sigma = beta * (W - DeltaF)
        # NOTE: Full physical certification requires partition-function DeltaF.
        sigma[:] = (
            self.config.beta
            * (work - delta_free_energy)
        )

        return EnsembleResult(
            sigma=sigma,
            charge=charge,
            work=work,
            delta_free_energy=delta_free_energy,
        )


def tut_bound(sigma: np.ndarray) -> tuple[float, float]:
    """
    Canonical TUT expression.

    Returns:
        bound,
        mean_tanh
    """

    sigma = np.asarray(sigma, dtype=float)

    if sigma.ndim != 1:
        raise ValueError("sigma must be one-dimensional")

    if not np.all(np.isfinite(sigma)):
        raise ValueError("sigma contains non-finite values")

    mean_tanh = float(
        np.mean(np.tanh(sigma / 2.0))
    )

    if mean_tanh <= 0.0:
        return np.inf, mean_tanh

    bound = 1.0 / mean_tanh - 1.0

    return float(bound), mean_tanh


def bootstrap_ci(
    sigma: np.ndarray,
    samples: int = 1000,
    confidence: float = 0.95,
    seed: int = 12345,
) -> tuple[float, float]:
    """Bootstrap percentile CI for the TUT bound."""

    if len(sigma) < 10:
        return np.nan, np.nan

    rng = np.random.default_rng(seed)

    values = []

    for _ in range(samples):
        sample = rng.choice(
            sigma,
            size=len(sigma),
            replace=True,
        )

        bound, _ = tut_bound(sample)

        if np.isfinite(bound):
            values.append(bound)

    if not values:
        return np.nan, np.nan

    alpha = 1.0 - confidence

    return (
        float(np.quantile(values, alpha / 2.0)),
        float(np.quantile(values, 1.0 - alpha / 2.0)),
    )


def analyze_tut(
    ensemble: EnsembleResult,
    dft_assumption_verified: bool = False,
) -> TUTResult:
    """Compute TUT and observed scaled variance."""

    bound, mean_tanh = tut_bound(ensemble.sigma)

    mean_q = np.mean(ensemble.charge)

    if abs(mean_q) < 1e-12:
        observed = np.inf
    else:
        observed = (
            np.var(ensemble.charge)
            / mean_q ** 2
        )

    ci_low, ci_high = bootstrap_ci(
        ensemble.sigma
    )

    return TUTResult(
        eps_sq_tut=bound,
        eps_sq_observed=float(observed),
        avg_sigma=float(np.mean(ensemble.sigma)),
        avg_tanh=mean_tanh,
        ci_low=ci_low,
        ci_high=ci_high,
        dft_assumption_verified=dft_assumption_verified,
    )


def run_config(
    config: SimulationConfig,
    dft_assumption_verified: bool = False,
) -> TUTResult:
    """Complete simulation + TUT pipeline."""

    rng = np.random.default_rng(config.seed)

    graph = build_honeycomb(
        config.honeycomb_radius
    )

    model = FloquetHoneycombKuramoto(
        graph,
        config,
        rng,
    )

    ensemble = model.ensemble()

    return analyze_tut(
        ensemble,
        dft_assumption_verified=dft_assumption_verified,
    )


def optimize(
    bounds: list[tuple[float, float]],
    base: SimulationConfig,
    seed: int = 20260816,
    crn_seed: int = 999999,  # Common Random Numbers seed
) -> object:
    """
    Differential-evolution search with Common Random Numbers (CRN).

    Parameter order:
        J, omega, amplitude, gamma, temperature

    Horizon remains fixed at base.horizon.

    CRN prevents DE from optimizing Monte Carlo noise rather than
    the actual physical system.
    """

    def objective(x: np.ndarray) -> float:

        J, omega, amplitude, gamma, temperature = x

        config = SimulationConfig(
            J=float(J),
            omega=float(omega),
            amplitude=float(amplitude),
            gamma=float(gamma),
            temperature=float(temperature),
            horizon=base.horizon,
            dt=base.dt,
            trajectories=base.trajectories,
            honeycomb_radius=base.honeycomb_radius,
            seed=crn_seed,  # FIXED CRN seed during exploration
        )

        result = run_config(config)

        if not np.isfinite(result.eps_sq_tut):
            return 1e12

        return result.eps_sq_tut

    return differential_evolution(
        objective,
        bounds=bounds,
        seed=seed,
        polish=True,
        workers=1,
        updating="immediate",
    )


def validate_winner(
    winner_params: np.ndarray,
    base: SimulationConfig,
    validation_seeds: list[int] | None = None,
) -> list[TUTResult]:
    """
    Re-evaluate the DE winner with independent random seeds.
    This separates the optimized point from the validation dataset.
    """
    if validation_seeds is None:
        validation_seeds = [101, 202, 303, 404, 505]

    J, omega, amplitude, gamma, temperature = winner_params
    results = []

    for vseed in validation_seeds:
        config = SimulationConfig(
            J=float(J),
            omega=float(omega),
            amplitude=float(amplitude),
            gamma=float(gamma),
            temperature=float(temperature),
            horizon=base.horizon,
            dt=base.dt,
            trajectories=base.trajectories,
            honeycomb_radius=base.honeycomb_radius,
            seed=vseed,
        )
        results.append(run_config(config))

    return results


def report(
    result: TUTResult,
) -> str:
    """Generate an explicitly non-teleological report."""

    if not np.isfinite(result.eps_sq_tut):
        interpretation = (
            "TUT bound undefined for this ensemble."
        )
    elif np.isclose(
        result.eps_sq_tut,
        INV_PHI,
        rtol=0.05,
    ):
        interpretation = (
            "Observed numerical proximity to 1/phi. "
            "This is an emergent numerical observation only; "
            "no physical selection mechanism is inferred."
        )
    else:
        interpretation = (
            "No numerical proximity to 1/phi. "
            "The measured distance is reported without "
            "forcing the result toward phi."
        )

    return f"""
AVALON STOCHASTIC TUT v2.1
==========================

epsilon^2_TUT : {result.eps_sq_tut:.8f}
95% CI        : [{result.ci_low:.8f}, {result.ci_high:.8f}]

<sigma>       : {result.avg_sigma:.8f}
<tanh(sigma/2)>: {result.avg_tanh:.8f}

epsilon^2_obs : {result.eps_sq_observed:.8f}

TUT condition :
    observed >= bound
    {result.eps_sq_observed >= result.eps_sq_tut}

DFT assumption independently verified:
    {result.dft_assumption_verified}

phi reference:
    1/phi = {INV_PHI:.12f}

distance:
    {result.distance_to_phi:.8f}

INTERPRETATION
--------------
{interpretation}

WARNING: Sigma = beta*(W - DeltaF) is an operational approximation.
Full physical certification requires partition-function DeltaF.
"""


if __name__ == "__main__":

    config = SimulationConfig(
        J=1.0,
        omega=1.0,
        amplitude=1.0,
        gamma=0.1,
        temperature=1.0,
        horizon=20.0,
        dt=0.05,
        trajectories=1000,
        seed=42,
    )

    result = run_config(config)

    print(report(result))