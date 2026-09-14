#!/usr/bin/env python3
"""
AVALON TUT BOUND MODULE (tut_bound_module.py)
===============================================
Core TUT bound computation with bootstrap confidence intervals.

v2.1: Unchanged from v2.0 - physics is correct.
"""

import numpy as np
from dataclasses import dataclass
from typing import Optional


@dataclass
class TUTResult:
    """Container for TUT computation results."""
    eps_sq_tut: float
    eps_sq_observed: float
    avg_sigma: float
    bound_satisfied: bool
    bootstrap_std: float


class TUTBound:
    """
    Compute the Thermodynamic Uncertainty Theorem bound from
    an ensemble of entropy production (Sigma) and charge (Q) values.
    """

    def __init__(self, bootstrap_samples: int = 500, seed: int = 42):
        self.bootstrap_samples = bootstrap_samples
        self.rng = np.random.default_rng(seed)

    def compute(
        self,
        sigma_array: np.ndarray,
        q_array: np.ndarray,
    ) -> TUTResult:
        """
        Compute TUT bound and observed precision from ensemble data.

        Parameters
        ----------
        sigma_array : np.ndarray
            Array of medium entropy production values (one per trajectory).
        q_array : np.ndarray
            Array of time-antisymmetric charge values (one per trajectory).

        Returns
        -------
        TUTResult
            eps_sq_tut      : TUT bound 1/<tanh(Sigma/2)> - 1
            eps_sq_observed : <Q>^2 / Var(Q)
            avg_sigma       : mean Sigma
            bound_satisfied : observed >= bound
            bootstrap_std   : std dev of bound under bootstrap
        """
        sigma_array = np.asarray(sigma_array, dtype=float).flatten()
        q_array = np.asarray(q_array, dtype=float).flatten()

        if len(sigma_array) == 0 or len(q_array) == 0:
            raise ValueError("Input arrays must not be empty")
        if len(sigma_array) != len(q_array):
            raise ValueError("sigma_array and q_array must have same length")

        # TUT bound: 1 / <tanh(Sigma/2)> - 1
        avg_tanh = float(np.mean(np.tanh(sigma_array / 2.0)))
        if avg_tanh <= 0.0:
            eps_sq_tut = np.inf
        else:
            eps_sq_tut = 1.0 / avg_tanh - 1.0

        # Observed precision: <Q>^2 / Var(Q)
        mean_q = float(np.mean(q_array))
        var_q = float(np.var(q_array, ddof=1))

        if abs(mean_q) < 1e-12:
            eps_sq_observed = np.inf
        else:
            eps_sq_observed = mean_q ** 2 / var_q if var_q > 0 else np.inf

        # Bootstrap for uncertainty quantification
        bounds = []
        n = len(sigma_array)
        for _ in range(self.bootstrap_samples):
            idx = self.rng.choice(n, size=n, replace=True)
            sample_sigma = sigma_array[idx]
            avg_tanh_b = float(np.mean(np.tanh(sample_sigma / 2.0)))
            if avg_tanh_b > 0.0:
                bounds.append(1.0 / avg_tanh_b - 1.0)

        bootstrap_std = float(np.std(bounds)) if bounds else np.nan

        return TUTResult(
            eps_sq_tut=float(eps_sq_tut),
            eps_sq_observed=float(eps_sq_observed),
            avg_sigma=float(np.mean(sigma_array)),
            bound_satisfied=eps_sq_observed >= eps_sq_tut,
            bootstrap_std=bootstrap_std,
        )

    def compute_mean_tanh_bound(self, sigma_array: np.ndarray) -> float:
        """Standalone ensemble bound computation."""
        sigma_array = np.asarray(sigma_array, dtype=float).flatten()
        avg_tanh = float(np.mean(np.tanh(sigma_array / 2.0)))
        if avg_tanh <= 0.0:
            return np.inf
        return 1.0 / avg_tanh - 1.0