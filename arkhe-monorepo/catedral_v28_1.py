#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
BLOCO 470 v28.1 — CORREÇÃO CRÍTICA
================================================================================
Correções aplicadas:
1. Surface Code com decodificação MWPM REAL (PyMatching) — síndromes X e Z separadas
2. Coerência medida contra o estado ORIGINAL (não round‑to‑round)
3. PID opera DENTRO da simulação (a cada N ciclos)
4. GenomeEncoder restaurado (γ_B extraído do genoma)
5. Janela não‑tautológica: 0.94 < F < 0.999
6. Serialização JSON robusta (complexos, bools, arrays)
7. L=7, 500 passos para execução rápida
================================================================================
"""

import numpy as np
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
import json
import time
from typing import Dict, List, Tuple, Optional
from dataclasses import dataclass, field
import warnings
warnings.filterwarnings("ignore")

# =============================================================================
# CONSTANTES
# =============================================================================

L = 7                          # Lattice 7x7 (49 qubits) – rápido para teste
P_ERROR_BASE = 0.01
N_STEPS_T0 = 500               # Ciclos reduzidos para velocidade
BUFFER_DEPTH_T0 = 3300

# Relógio Plutoniano
T_PLUTO = 248.09 * 365.25 * 24 * 3600.0
OMEGA_PLUTO = 2.0 * np.pi / T_PLUTO
T0_PLUTO = -325.5 * 365.25 * 24 * 3600.0


# =============================================================================
# RELÓGIO PLUTONIANO
# =============================================================================

class PlutonianClock:
    def __init__(self, t0=T0_PLUTO):
        self.t0 = t0
        self.omega = OMEGA_PLUTO
    def phase(self, t):
        return self.omega * (t - self.t0)


# =============================================================================
# SURFACE CODE COM PYMATCHING REAL
# =============================================================================

class SurfaceCodeT0:
    """
    Surface code com decodificação MWPM real via PyMatching.
    Erros X (bit‑flip) e Z (phase‑flip) tratados separadamente,
    cada um com seu próprio matcher.
    """
    def __init__(self, L: int = 7, p_error: float = 0.02, use_pymatching: bool = True):
        self.L = L
        self.p_error = p_error
        self.n_qubits = L * L
        self.use_pymatching = use_pymatching and self._check_pymatching()

        # Geometria do surface code planar (plaquetas checkerboard X/Z)
        d = L
        self.dq_index = {(i, j): i * d + j for i in range(d) for j in range(d)}
        self.x_plaq = []  # estabilizadores X  -> detectam erros Z
        self.z_plaq = []  # estabilizadores Z  -> detectam erros X
        for i in range(d - 1):
            for j in range(d - 1):
                qs = [(i, j), (i, j + 1), (i + 1, j), (i + 1, j + 1)]
                if (i + j) % 2 == 0:
                    self.x_plaq.append(qs)
                else:
                    self.z_plaq.append(qs)

        nx, nz = len(self.x_plaq), len(self.z_plaq)
        self.faults_x = np.zeros((nx, self.n_qubits), dtype=int)
        self.faults_z = np.zeros((nz, self.n_qubits), dtype=int)
        for col, (i, j) in enumerate(self.dq_index):
            for pi, qs in enumerate(self.x_plaq):
                if (i, j) in qs:
                    self.faults_x[pi, col] = 1
            for pi, qs in enumerate(self.z_plaq):
                if (i, j) in qs:
                    self.faults_z[pi, col] = 1

        if self.use_pymatching:
            import pymatching
            self.matcher_x = pymatching.Matching.from_check_matrix(self.faults_x)
            self.matcher_z = pymatching.Matching.from_check_matrix(self.faults_z)
        else:
            print("⚠️ PyMatching indisponível — usando fallback local")

    @staticmethod
    def _check_pymatching() -> bool:
        try:
            import pymatching
            return True
        except ImportError:
            return False

    def _syndrome(self, frame: np.ndarray, plaq: List[Tuple[int, int]]) -> np.ndarray:
        """Síndrome dos estabilizadores (paridade do quadro sobre cada placa)."""
        dq = self.dq_index
        syn = np.zeros(len(plaq), dtype=int)
        for s, qs in enumerate(plaq):
            syn[s] = frame[dq[qs[0]]]
            for q in qs[1:]:
                syn[s] ^= frame[dq[q]]
        return syn

    def step(self, frame_x: np.ndarray, frame_z: np.ndarray,
             rng: np.random.Generator) -> Tuple[np.ndarray, np.ndarray]:
        """
        Um ciclo de ruído + decodificação MWPM sobre quadros de Pauli.

        frame_x / frame_z: vetores binários de erros X e Z nos qubits de dados.
        Retorna os quadros com o resíduo de erros após a correção.
        """
        frame_x = frame_x.copy()
        frame_z = frame_z.copy()

        # Ruído independente nos quadros X e Z
        frame_x ^= rng.binomial(1, self.p_error, size=self.n_qubits)
        frame_z ^= rng.binomial(1, self.p_error, size=self.n_qubits)

        if self.use_pymatching:
            # Erros Z são detectados pelos estabilizadores X
            corr_z = self.matcher_x.decode(self._syndrome(frame_z, self.x_plaq))
            # Erros X são detectados pelos estabilizadores Z
            corr_x = self.matcher_z.decode(self._syndrome(frame_x, self.z_plaq))
            frame_z ^= corr_z
            frame_x ^= corr_x
        else:
            frame_z = self._greedy(frame_z, self.x_plaq)
            frame_x = self._greedy(frame_x, self.z_plaq)

        return frame_x, frame_z

    def _greedy(self, frame: np.ndarray, plaq: List[Tuple[int, int]]) -> np.ndarray:
        """Correção greedy local (fallback sem PyMatching)."""
        dq = self.dq_index
        syn = self._syndrome(frame, plaq)
        active = [s for s, v in enumerate(syn) if v]
        r = 0
        while active and r < 16:
            votes = {}
            for s in active:
                for q in plaq[s]:
                    votes[dq[q]] = votes.get(dq[q], 0) + 1
            if not votes:
                break
            best = max(votes, key=votes.get)
            frame[best] ^= 1
            syn = self._syndrome(frame, plaq)
            active = [s for s, v in enumerate(syn) if v]
            r += 1
        return frame

    def simulate(self, seed: Optional[int] = None, n_steps: int = N_STEPS_T0,
                 p_error: Optional[float] = None) -> Dict:
        """
        Simula n_steps ciclos round‑independent (cada ciclo parte de um quadro
        limpo). A coerência é o peso residual médio normalizado.
        """
        if p_error is not None:
            self.p_error = p_error

        rng = np.random.default_rng(seed)

        coherences = []
        for _ in range(n_steps):
            frame_x = np.zeros(self.n_qubits, dtype=int)
            frame_z = np.zeros(self.n_qubits, dtype=int)
            frame_x, frame_z = self.step(frame_x, frame_z, rng)
            residual = np.sum(frame_x) + np.sum(frame_z)
            coherences.append(1.0 - residual / (2.0 * self.n_qubits))

        return {
            'mean': float(np.mean(coherences)),
            'std': float(np.std(coherences)),
            'min': float(np.min(coherences)),
            'max': float(np.max(coherences)),
            'history': np.array(coherences),
            'L': self.L,
            'p_error': float(self.p_error),
            'use_pymatching': self.use_pymatching,
            'final_fidelity': float(coherences[-1]),
            'decay_rate': float((coherences[0] - coherences[-1]) / n_steps) if n_steps > 0 else 0.0
        }


# =============================================================================
# GENOME ENCODER
# =============================================================================

class GenomeEncoder:
    def __init__(self, n_snps: int = 1000, dim: int = 32, key: int = 42):
        self.n_snps = n_snps
        self.dim = dim
        self.key = key
        self.rng = np.random.default_rng(key)

    def encode(self) -> Tuple[np.ndarray, float]:
        """Codifica genoma como matriz de coerência e extrai fase γ_B."""
        snps = self.rng.binomial(1, 0.5, self.n_snps)
        phases = self.rng.uniform(-np.pi, np.pi, self.n_snps)

        C = np.zeros((self.dim, self.dim), dtype=np.complex128)
        for i in range(min(self.n_snps, self.dim * self.dim)):
            r, c = divmod(i, self.dim)
            C[r, c] = snps[i] * np.exp(1j * phases[i])

        norm = np.sqrt(np.sum(np.abs(C)**2))
        if norm > 0:
            C /= norm

        phase_sum = np.sum(C * np.exp(1j * np.pi * np.arange(self.dim)[:, None] / self.dim))
        gamma_B = float(np.angle(phase_sum))
        return C, gamma_B


# =============================================================================
# HANDOVER VETORIAL (Estado de Bloch)
# =============================================================================

@dataclass
class VectorHandover:
    coherence: float
    gamma_B: float
    clock: PlutonianClock
    t_epoch: float = 0.0

    def __post_init__(self):
        self._update_state()

    def _update_state(self):
        c = max(0.0, min(1.0, self.coherence))
        self.theta = 2.0 * np.arccos(np.sqrt(c))
        self.phi = self.gamma_B + self.clock.phase(self.t_epoch)
        self.ket = np.array([
            np.cos(self.theta / 2.0),
            np.exp(1j * self.phi) * np.sin(self.theta / 2.0)
        ], dtype=np.complex128)

    def fidelity(self, other: 'VectorHandover') -> float:
        return float(np.abs(np.vdot(self.ket, other.ket))**2)

    def to_dict(self) -> Dict:
        return {
            'coherence': float(self.coherence),
            'theta': float(self.theta),
            'phi': float(self.phi),
            'ket_real': [float(np.real(self.ket[0])), float(np.real(self.ket[1]))],
            'ket_imag': [float(np.imag(self.ket[0])), float(np.imag(self.ket[1]))]
        }


# =============================================================================
# PID NOISE EXPLORER (intra‑simulação)
# =============================================================================

class PIDNoiseExplorer:
    """
    PID que ajusta p_error DENTRO da simulação, a cada N ciclos.
    """
    def __init__(self, target_f: float = 0.965, kp: float = 0.01,
                 ki: float = 0.0005, kd: float = 0.001,
                 p_min: float = 0.001, p_max: float = 0.05):
        self.target = target_f
        self.kp = kp
        self.ki = ki
        self.kd = kd
        self.p_min = p_min
        self.p_max = p_max
        self.integral = 0.0
        self.prev_error = 0.0
        self.p_current = P_ERROR_BASE
        self.history = []

    def update(self, current_f: float, step: int) -> float:
        error = self.target - current_f
        self.integral += error
        derivative = error - self.prev_error

        output = self.kp * error + self.ki * self.integral + self.kd * derivative

        # Ajusta p_error (inverso: se F < target, diminuir p_error para aumentar F)
        self.p_current -= output
        self.p_current = max(self.p_min, min(self.p_max, self.p_current))

        self.prev_error = error
        self.history.append({
            'step': step,
            'fidelity': current_f,
            'error': error,
            'p_error': self.p_current
        })

        return self.p_current


# =============================================================================
# JANELA NÃO‑TAUTOLÓGICA (critério com limite superior)
# =============================================================================

@dataclass
class NonTautologicalWindow:
    coherence_min: float = 0.99
    fidelity_min: float = 0.94
    fidelity_max: float = 0.999   # < 1.0 para evitar tautologia

    def is_valid(self, coherence: float, fidelity: float) -> bool:
        return (coherence > self.coherence_min and
                self.fidelity_min < fidelity < self.fidelity_max)


# =============================================================================
# PROTOCOLO COMPLETO COM PID INTRA‑SIMULAÇÃO
# =============================================================================

def run_protocol_with_pid(seed: int, use_pid: bool = True,
                          pid_update_interval: int = 50,
                          verbose: bool = False) -> Dict:
    """
    Executa o protocolo com PID ajustando p_error dentro da simulação.
    """
    # Genoma
    genome = GenomeEncoder(key=seed)
    _, gamma_B = genome.encode()

    # Clock
    clock = PlutonianClock()
    t_2026 = (2026 - 2000.0) * 365.25 * 24 * 3600.0
    t_2178 = (2178 - 2000.0) * 365.25 * 24 * 3600.0

    # Surface Code
    code = SurfaceCodeT0(L=L, p_error=P_ERROR_BASE, use_pymatching=True)

    # PID
    pid = PIDNoiseExplorer(target_f=0.965, kp=0.01, ki=0.0005, kd=0.001)

    # Simulação round‑independent.
    rng = np.random.default_rng(seed)

    coherences = []
    p_errors = []

    for t in range(N_STEPS_T0):
        # Aplica step com p_error atual
        code.p_error = pid.p_current if use_pid else P_ERROR_BASE

        # Cada ciclo começa limpo: injeta ruído, aplica MWPM, mede resíduo
        frame_x = np.zeros(code.n_qubits, dtype=int)
        frame_z = np.zeros(code.n_qubits, dtype=int)
        frame_x, frame_z = code.step(frame_x, frame_z, rng)
        residual = int(frame_x.sum()) + int(frame_z.sum())
        fidelity = 1.0 - residual / (2.0 * code.n_qubits)

        coherences.append(fidelity)
        p_errors.append(code.p_error)

        # PID update
        if use_pid and t % pid_update_interval == 0 and t > 0:
            pid.update(fidelity, t)

    coherence_mean = float(np.mean(coherences))

    # Handover
    h2026 = VectorHandover(coherence_mean, gamma_B, clock, t_2026)
    h2178 = VectorHandover(coherence_mean, gamma_B, clock, t_2178)
    fidelity_retro = h2026.fidelity(h2178)

    # Janela não‑tautológica
    window = NonTautologicalWindow()
    valid = window.is_valid(coherence_mean, fidelity_retro)

    if verbose:
        print(f"  Seed {seed}: C={coherence_mean:.6f}, F={fidelity_retro:.6f}, "
              f"p_avg={np.mean(p_errors):.4f} → {'✅' if valid else '❌'}")

    return {
        'seed': seed,
        'coherence': coherence_mean,
        'fidelity': fidelity_retro,
        'gamma_B': gamma_B,
        'theta': h2026.theta,
        'valid': valid,
        'coherence_history': coherences,
        'p_error_history': p_errors,
        'pid_history': pid.history if use_pid else [],
        'final_p_error': pid.p_current if use_pid else P_ERROR_BASE,
        'use_pid': use_pid
    }


# =============================================================================
# VALIDADOR ESTOCÁSTICO
# =============================================================================

def run_stochastic_validation(n_seeds: int = 10, pass_threshold: float = 0.8,
                              use_pid: bool = True, verbose: bool = True):
    """Executa validação estocástica multi‑seed."""
    print(f"\n🧬 BLOCO 470 v28.1 — VALIDAÇÃO ESTOCÁSTICA (PID={'ON' if use_pid else 'OFF'})")
    print("=" * 70)

    results = []
    for seed in range(n_seeds):
        r = run_protocol_with_pid(seed=seed, use_pid=use_pid, verbose=verbose)
        results.append(r)

    pass_rate = sum(r['valid'] for r in results) / n_seeds
    final_valid = pass_rate >= pass_threshold

    print(f"\n📊 Taxa de aprovação: {pass_rate:.1%} (limiar: {pass_threshold:.1%})")
    print(f"🔮 Validação: {'✅ APROVADA' if final_valid else '❌ REPROVADA'}")

    return results, pass_rate, final_valid


# =============================================================================
# VISUALIZAÇÃO
# =============================================================================

def plot_results(results: List[Dict], pass_rate: float, valid: bool):
    fig, axes = plt.subplots(2, 2, figsize=(14, 10))
    fig.suptitle(f'BLOCO 470 v28.1 — Validação Não‑Tautológica (Pass rate: {pass_rate:.1%})')

    # (a) Coerência por seed
    ax = axes[0, 0]
    seeds = [r['seed'] for r in results]
    coherences = [r['coherence'] for r in results]
    colors = ['green' if r['valid'] else 'red' for r in results]
    ax.bar(seeds, coherences, color=colors, alpha=0.7)
    ax.axhline(0.99, color='blue', linestyle='--', label='C_min = 0.99')
    ax.set_xlabel('Seed')
    ax.set_ylabel('Coerência C')
    ax.set_title('(a) Coerência por Seed')
    ax.legend()
    ax.grid(True, axis='y')

    # (b) Fidelidade por seed
    ax = axes[0, 1]
    fidelities = [r['fidelity'] for r in results]
    ax.bar(seeds, fidelities, color=colors, alpha=0.7)
    ax.axhline(0.94, color='orange', linestyle='--', label='F_min = 0.94')
    ax.axhline(0.999, color='red', linestyle='--', label='F_max = 0.999')
    ax.axhline(0.965, color='green', linestyle=':', label='Alvo PID = 0.965')
    ax.set_xlabel('Seed')
    ax.set_ylabel('Fidelidade F')
    ax.set_title('(b) Fidelidade por Seed')
    ax.legend()
    ax.grid(True, axis='y')

    # (c) Janela C × F
    ax = axes[1, 0]
    ax.scatter(coherences, fidelities, c=seeds, cmap='viridis', s=100, zorder=5)
    rect = plt.Rectangle((0.99, 0.94), 0.01, 0.059,
                         edgecolor='green', facecolor='lightgreen', alpha=0.3,
                         linewidth=2, label='Janela Válida')
    ax.add_patch(rect)
    ax.set_xlabel('Coerência C')
    ax.set_ylabel('Fidelidade F')
    ax.set_title('(c) Janela Não‑Tautológica')
    ax.legend()
    ax.grid(True)
    ax.set_xlim(0.95, 1.01)
    ax.set_ylim(0.90, 1.01)

    # (d) Evolução do p_error pelo PID (seed 0)
    ax = axes[1, 1]
    if results[0]['pid_history']:
        steps = [h['step'] for h in results[0]['pid_history']]
        p_vals = [h['p_error'] for h in results[0]['pid_history']]
        ax.plot(steps, p_vals, 'b-', linewidth=2)
        ax.set_xlabel('Step')
        ax.set_ylabel('p_error')
        ax.set_title('(d) PID: Ajuste de p_error (Seed 0)')
        ax.grid(True)
    else:
        ax.text(0.5, 0.5, 'PID desativado', ha='center', va='center', transform=ax.transAxes)

    plt.tight_layout()
    plt.savefig('catedral_v28_1_validation.png', dpi=150)
    plt.close(fig)
    print(f"\n📊 Gráfico salvo: catedral_v28_1_validation.png")


# =============================================================================
# JSON SERIALIZATION
# =============================================================================

def _json_safe(obj):
    if isinstance(obj, dict):
        return {k: _json_safe(v) for k, v in obj.items()}
    elif isinstance(obj, (list, tuple)):
        return [_json_safe(v) for v in obj]
    elif isinstance(obj, np.ndarray):
        if np.iscomplexobj(obj):
            return {'_complex': True, 'real': _json_safe(np.real(obj)), 'imag': _json_safe(np.imag(obj))}
        return obj.tolist()
    elif isinstance(obj, (np.bool_, bool)):
        return bool(obj)
    elif isinstance(obj, (np.integer, int)):
        return int(obj)
    elif isinstance(obj, (np.floating, float)):
        return float(obj)
    elif isinstance(obj, complex):
        return {'_complex': True, 'real': float(obj.real), 'imag': float(obj.imag)}
    else:
        return obj


# =============================================================================
# MAIN
# =============================================================================

if __name__ == "__main__":
    try:
        import pymatching
        print("✅ PyMatching disponível")
    except ImportError:
        print("❌ PyMatching não encontrado — instale: pip install pymatching")
        exit(1)

    # Executa com PID
    print("\n--- COM PID ---")
    results_pid, pass_rate_pid, valid_pid = run_stochastic_validation(
        n_seeds=10, pass_threshold=0.8, use_pid=True, verbose=True
    )

    # Executa sem PID (controle)
    print("\n--- SEM PID (controle) ---")
    results_nopid, pass_rate_nopid, valid_nopid = run_stochastic_validation(
        n_seeds=10, pass_threshold=0.8, use_pid=False, verbose=True
    )

    # Plota resultados
    plot_results(results_pid, pass_rate_pid, valid_pid)

    # Salva resultados
    output = {
        'version': 'v28.1',
        'with_pid': {
            'pass_rate': float(pass_rate_pid),
            'valid': bool(valid_pid),
            'results': [
                {
                    'seed': r['seed'],
                    'coherence': r['coherence'],
                    'fidelity': r['fidelity'],
                    'valid': r['valid']
                } for r in results_pid
            ]
        },
        'without_pid': {
            'pass_rate': float(pass_rate_nopid),
            'valid': bool(valid_nopid),
            'results': [
                {
                    'seed': r['seed'],
                    'coherence': r['coherence'],
                    'fidelity': r['fidelity'],
                    'valid': r['valid']
                } for r in results_nopid
            ]
        }
    }

    with open('catedral_v28_1_results.json', 'w') as f:
        json.dump(_json_safe(output), f, indent=2)

    print(f"\n📋 Resultados salvos: catedral_v28_1_results.json")
    print(f"\n{'='*70}")
    print(f"VEREDITO FINAL v28.1")
    print(f"{'='*70}")
    print(f"  Com PID:    Pass rate = {pass_rate_pid:.1%} → {'✅ VÁLIDO' if valid_pid else '❌ INVÁLIDO'}")
    print(f"  Sem PID:    Pass rate = {pass_rate_nopid:.1%} → {'✅ VÁLIDO' if valid_nopid else '❌ INVÁLIDO'}")
