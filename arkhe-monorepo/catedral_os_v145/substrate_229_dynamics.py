#!/usr/bin/env python3
"""
Substrato 229 — Dinâmica Caótica (Sistema de Chen)

Implementa o sistema de Chen de 3 dimensões:

    dx/dt = a * (y - x)
    dy/dt = (c - a) * x - x*z + c*y
    dz/dt = x*y - b*z

com parâmetros clássicos de caos (a=35, b=3, c=28). Integração por Runge-Kutta
de 4ª ordem para trajetórias determinísticas, com análise de sensibilidade
(expoente de Lyapunov aproximado) e extração de métricas de complexidade usadas
pelo CGF (Substrato 172).

NOTA DE HONESTIDADE (auditoria): o expoente de Lyapunov aqui é uma estimativa
por divergência de trajetórias próximas — não é o espectro completo calculado
por métodos numéricos consolidados; serve para sinalizar comportamento caótico
(>0), não para valor quantitativo absoluto de referência.
"""

import numpy as np
from typing import Dict, List, Tuple, Optional


class DynamicsEngine:
    """Motor de dinâmica caótica baseado no sistema de Chen."""

    def __init__(self, a: float = 35.0, b: float = 3.0, c: float = 28.0,
                 dt: float = 0.001):
        self.a = a
        self.b = b
        self.c = c
        self.dt = dt

    def rhs(self, state: np.ndarray) -> np.ndarray:
        """Derivadas do sistema de Chen."""
        x, y, z = state
        dx = self.a * (y - x)
        dy = (self.c - self.a) * x - x * z + self.c * y
        dz = x * y - self.b * z
        return np.array([dx, dy, dz])

    def step_rk4(self, state: np.ndarray) -> np.ndarray:
        """Um passo de Runge-Kutta de 4ª ordem."""
        k1 = self.rhs(state)
        k2 = self.rhs(state + 0.5 * self.dt * k1)
        k3 = self.rhs(state + 0.5 * self.dt * k2)
        k4 = self.rhs(state + self.dt * k3)
        return state + (self.dt / 6.0) * (k1 + 2 * k2 + 2 * k3 + k4)

    def simulate(self, steps: int = 5000, initial_state: Optional[np.ndarray] = None,
                 discard_transient: int = 1000) -> Dict:
        """
        Simula a trajetória caótica.

        Retorna dicionário com a trajetória, o atrator de Chen e métricas.
        """
        if initial_state is None:
            state = np.array([0.1, 0.0, 0.0], dtype=float)
        else:
            state = np.asarray(initial_state, dtype=float)

        traj = np.zeros((steps, 3))
        for i in range(steps + discard_transient):
            state = self.step_rk4(state)
            if i >= discard_transient:
                traj[i - discard_transient] = state

        metrics = self.analyze_trajectory(traj)
        return {
            "trajectory": traj,
            "initial_state": np.asarray(initial_state if initial_state is not None else [0.1, 0.0, 0.0]),
            "parameters": {"a": self.a, "b": self.b, "c": self.c, "dt": self.dt},
            "metrics": metrics,
        }

    def lyapunov_estimate(self, steps: int = 5000, delta: float = 1e-8) -> float:
        """
        Estimativa do maior expoente de Lyapunov por divergência de trajetórias
        pequenas perturbadas. >0 indica sensibilidade a condições iniciais (caos).
        """
        ref = np.array([0.1, 0.0, 0.0], dtype=float)
        pert = ref + np.array([delta, 0.0, 0.0], dtype=float)
        # descarta transiente
        for _ in range(1000):
            ref = self.step_rk4(ref)
            pert = self.step_rk4(pert)

        total_log = 0.0
        for _ in range(steps):
            ref = self.step_rk4(ref)
            pert = self.step_rk4(pert)
            d = np.linalg.norm(pert - ref)
            if d < 1e-12:
                d = 1e-12
            total_log += np.log(d / delta)
            # renormaliza a perturbação na direção da divergência
            direction = (pert - ref) / d
            pert = ref + delta * direction
        return total_log / (steps * self.dt)

    @staticmethod
    def analyze_trajectory(traj: np.ndarray) -> Dict:
        """Métricas de complexidade da trajetória."""
        std = np.std(traj, axis=0)
        ranges = np.ptp(traj, axis=0)
        # Correlação entre variáveis (medida de acoplamiento)
        corr = np.corrcoef(traj.T)
        # Entropia aproximada (histograma de estados) — proxy de complexidade
        flat = np.abs(traj[:, 0]) + np.abs(traj[:, 1])
        hist, _ = np.histogram(flat, bins=32, density=True)
        hist = hist[hist > 0]
        entropy = -np.sum(hist * np.log(hist))
        return {
            "std": std.tolist(),
            "range": ranges.tolist(),
            "cross_corr_xy": float(corr[0, 1]),
            "approx_entropy": float(entropy),
            "mean_x": float(np.mean(traj[:, 0])),
            "mean_y": float(np.mean(traj[:, 1])),
            "mean_z": float(np.mean(traj[:, 2])),
        }

    def is_chaotic(self, threshold: float = 0.0) -> bool:
        """True se o expoente de Lyapunov estimado for positivo."""
        return self.lyapunov_estimate() > threshold


if __name__ == "__main__":
    eng = DynamicsEngine()
    res = eng.simulate(steps=2000)
    m = res["metrics"]
    lam = eng.lyapunov_estimate()
    print(f"Chen: mean_y={m['mean_y']:.2f} entropia={m['approx_entropy']:.3f} "
          f"Lyapunov≈{lam:.3f} caótico={lam > 0}")
