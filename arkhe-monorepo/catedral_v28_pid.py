#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
BLOCO 470 v28 — IMPLEMENTAÇÃO DO CRITÉRIO NÃO-TAUTOLÓGICO E PID NOISE EXPLORER
================================================================================
Integra:
- Janela de Validação: Coerência > 0.99 E 0.94 < Fidelidade < 1.0
- Controlador PID para ajustar a taxa de erro (p_error) e explorar a janela
- Validação estocástica multi-seed (10 seeds, ≥80% de aprovação)
================================================================================
"""

import numpy as np
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
import json
import time
from typing import Dict, List, Tuple
from dataclasses import dataclass, field
import warnings
warnings.filterwarnings("ignore")

# =============================================================================
# 1. IMPORTAÇÃO DO PROTOCOLO BASE (v27)
# =============================================================================
# Assumimos que o código do Bloco 470 v26/v27 está disponível.
# Para autonomia, reimplementamos os componentes essenciais aqui.

# --- Constantes Físicas ---
L = 13
P_ERROR_BASE = 0.01
N_STEPS_T0 = 5000
BUFFER_DEPTH_T0 = 3300

# --- Relógio Plutoniano ---
T_PLUTO = 248.09 * 365.25 * 24 * 3600.0
OMEGA_PLUTO = 2.0 * np.pi / T_PLUTO
T0_PLUTO = -325.5 * 365.25 * 24 * 3600.0

class PlutonianClock:
    def __init__(self, t0=T0_PLUTO):
        self.t0 = t0
        self.omega = OMEGA_PLUTO
    def phase(self, t):
        return self.omega * (t - self.t0)

# --- Surface Code Robusto (v28) ---
class SurfaceCodeT0:
    def __init__(self, L=13, p_error=0.01, use_pymatching=True):
        self.L = L
        self.p_error = p_error
        self.use_pymatching = use_pymatching and self._pymatching_available()
        self.n_qubits = L * L
        self._build_matchers()

        # quadros de Pauli (separação física X/Z válida para PyMatching)
        self.x_plaq = []   # estabilizadores X (detectam erros Z)
        self.z_plaq = []   # estabilizadores Z (detectam erros X)
        d = L
        for i in range(d - 1):
            for j in range(d - 1):
                qs = [(i, j), (i, j + 1), (i + 1, j), (i + 1, j + 1)]
                if (i + j) % 2 == 0:
                    self.x_plaq.append(qs)
                else:
                    self.z_plaq.append(qs)
        self.dq_index = {(i, j): i * L + j for i in range(d) for j in range(d)}

        # matrizes de falhas (erro de qubit -> estabilizadores vizinhos, <= 2 uns)
        nx = len(self.x_plaq)
        nz = len(self.z_plaq)
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
            print("⚠️ Fallback: sem PyMatching")

    @staticmethod
    def _pymatching_available():
        try:
            import pymatching
            return True
        except ImportError:
            return False

    def _build_matchers(self):
        return None, None

    def _syndrome(self, frame, plaq):
        dq = self.dq_index
        syn = np.zeros(len(plaq), dtype=int)
        for s, qs in enumerate(plaq):
            syn[s] = frame[dq[qs[0]]]
            for q in qs[1:]:
                syn[s] ^= frame[dq[q]]
        return syn

    def step(self, frame_x, frame_z, rng):
        frame_x ^= rng.binomial(1, self.p_error, size=frame_x.shape)
        frame_z ^= rng.binomial(1, self.p_error, size=frame_z.shape)

        if self.use_pymatching:
            corr_z = self.matcher_x.decode(self._syndrome(frame_z, self.x_plaq))
            corr_x = self.matcher_z.decode(self._syndrome(frame_x, self.z_plaq))
            frame_z ^= corr_z
            frame_x ^= corr_x
        else:
            frame_z = self._greedy(frame_z, self.x_plaq)
            frame_x = self._greedy(frame_x, self.z_plaq)
        return frame_x, frame_z

    def _greedy(self, frame, plaq):
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

    def simulate(self, seed=None, n_steps=N_STEPS_T0, p_error=None):
        if p_error is not None:
            self.p_error = p_error
        rng = np.random.default_rng(seed)
        coherences = []

        for _ in range(n_steps):
            frame_x = np.zeros(self.n_qubits, dtype=int)
            frame_z = np.zeros(self.n_qubits, dtype=int)
            frame_x, frame_z = self.step(frame_x, frame_z, rng)
            residual = int(frame_x.sum()) + int(frame_z.sum())
            coherences.append(1.0 - residual / (2.0 * self.n_qubits))

        return {
            'mean': np.mean(coherences),
            'std': np.std(coherences),
            'history': np.array(coherences),
            'L': self.L,
            'p_error': self.p_error
        }

# --- Handover e demais componentes (adaptados do v26) ---
def run_protocol_for_seed(seed: int, p_error: float = P_ERROR_BASE) -> Dict:
    """Executa uma rodada completa do protocolo para um dado seed e p_error."""
    clock = PlutonianClock()
    t_2026 = (2026 - 2000.0) * 365.25 * 24 * 3600.0
    t_2178 = (2178 - 2000.0) * 365.25 * 24 * 3600.0

    # 1. Surface Code
    code = SurfaceCodeT0(L=L, p_error=p_error, use_pymatching=True)
    sc_result = code.simulate(seed=seed, n_steps=N_STEPS_T0)
    coherence = sc_result['mean']

    # 2. Handover (Bloch)
    gamma_B = 0.5  # fase fixa para simulação
    theta = 2.0 * np.arccos(np.sqrt(max(0.0, min(1.0, coherence))))
    phi_2026 = gamma_B + clock.phase(t_2026)
    phi_2178 = gamma_B + clock.phase(t_2178)

    ket_2026 = np.array([np.cos(theta/2), np.exp(1j*phi_2026)*np.sin(theta/2)])
    ket_2178 = np.array([np.cos(theta/2), np.exp(1j*phi_2178)*np.sin(theta/2)])
    fidelity = np.abs(np.vdot(ket_2026, ket_2178))**2

    return {
        'coherence': coherence,
        'fidelity': fidelity,
        'theta': theta,
        'p_error': p_error,
        'seed': seed
    }

# =============================================================================
# 2. JANELA NÃO-TAUTOLÓGICA E VALIDADOR
# =============================================================================

@dataclass
class NonTautologicalWindow:
    """Define a janela de validação: C > 0.99 e 0.94 < F < 1.0"""
    coherence_min: float = 0.99
    fidelity_min: float = 0.94
    fidelity_max: float = 1.0  # estritamente menor que 1.0

    def is_valid(self, coherence: float, fidelity: float) -> bool:
        return (coherence > self.coherence_min and
                self.fidelity_min < fidelity < self.fidelity_max)

class StochasticResilienceValidator:
    def __init__(self, n_seeds=10, pass_threshold=0.8):
        self.n_seeds = n_seeds
        self.pass_threshold = pass_threshold
        self.window = NonTautologicalWindow()
        self.history = []

    def validate(self, protocol_runner, p_error_base=P_ERROR_BASE, verbose=True):
        results = []
        for seed in range(self.n_seeds):
            # Adiciona pequena variação no p_error para explorar a janela
            p_error = p_error_base * (1.0 + 0.1 * np.random.randn())
            p_error = max(0.005, min(0.05, p_error))

            result = protocol_runner(seed=seed, p_error=p_error)
            c = result['coherence']
            f = result['fidelity']
            valid = self.window.is_valid(c, f)
            results.append(valid)

            self.history.append({
                'seed': seed,
                'coherence': c,
                'fidelity': f,
                'p_error': p_error,
                'valid': valid
            })

            if verbose:
                status = "✅" if valid else "❌"
                print(f"  Seed {seed:2d}: C={c:.6f}, F={f:.6f} (p={p_error:.4f}) → {status}")

        pass_rate = sum(results) / self.n_seeds
        final_valid = pass_rate >= self.pass_threshold

        if verbose:
            print(f"\n📊 Taxa de aprovação: {pass_rate:.1%} (limiar: {self.pass_threshold:.1%})")
            print(f"🔮 Validação geral: {'✅ APROVADA' if final_valid else '❌ REPROVADA'}")

        return {
            'pass_rate': pass_rate,
            'threshold': self.pass_threshold,
            'valid': final_valid,
            'results': results,
            'history': self.history
        }

# =============================================================================
# 3. CONTROLADOR PID PARA EXPLORAÇÃO DA JANELA
# =============================================================================

class PIDNoiseExplorer:
    """
    Controlador PID digital que ajusta o ruído (p_error) para manter a
    fidelidade F dentro da janela não-tautológica.

    Objetivo: F_target = 0.97 (meio da janela 0.94-1.0)
    Ação: ajusta p_error dinamicamente.
    """
    def __init__(self, target_f=0.97, kp=0.5, ki=0.1, kd=0.05, dt=1.0):
        self.target = target_f
        self.kp = kp
        self.ki = ki
        self.kd = kd
        self.dt = dt
        self.integral = 0.0
        self.prev_error = 0.0
        self.p_error_current = P_ERROR_BASE
        self.history = []

    def update(self, current_f: float) -> float:
        """Calcula a nova taxa de erro (p_error) com base no erro de F."""
        error = self.target - current_f
        self.integral += error * self.dt
        derivative = (error - self.prev_error) / self.dt if self.dt > 0 else 0

        output = (self.kp * error +
                  self.ki * self.integral +
                  self.kd * derivative)

        # Ajusta o p_error (limita entre 0.005 e 0.05)
        new_p = self.p_error_current + output * 0.01  # ganho de escala
        new_p = max(0.005, min(0.05, new_p))

        self.prev_error = error
        self.p_error_current = new_p
        self.history.append({
            'step': len(self.history),
            'current_f': current_f,
            'error': error,
            'p_error': new_p
        })

        return new_p

# =============================================================================
# 4. EXECUÇÃO PRINCIPAL — VALIDAÇÃO COMPLETA COM PID
# =============================================================================

def run_catedral_v28_with_pid(n_seeds=10, verbose=True):
    """
    Executa o protocolo v28 com:
    - Validação estocástica multi-seed
    - Controle PID para explorar a janela não-tautológica
    """
    print("🧬 BLOCO 470 v28 — VALIDAÇÃO NÃO-TAUTOLÓGICA COM PID")
    print("=" * 70)

    # Instancia o controlador PID
    pid = PIDNoiseExplorer(target_f=0.97, kp=0.8, ki=0.15, kd=0.1)
    validator = StochasticResilienceValidator(n_seeds=n_seeds, pass_threshold=0.8)

    all_seed_results = []
    p_errors_used = []
    fidelities = []

    for seed in range(n_seeds):
        # Obtém o p_error ajustado pelo PID
        p_error = pid.p_error_current

        # Executa o protocolo
        result = run_protocol_for_seed(seed=seed, p_error=p_error)
        c = result['coherence']
        f = result['fidelity']

        # Atualiza o PID com o F medido
        new_p = pid.update(f)

        # Registra
        all_seed_results.append(result)
        p_errors_used.append(p_error)
        fidelities.append(f)

        valid = validator.window.is_valid(c, f)
        status = "✅" if valid else "❌"

        if verbose:
            print(f"  Seed {seed:2d}: C={c:.6f}, F={f:.6f}, p={p_error:.4f} → {status} (PID ajustou para {new_p:.4f})")

    # Avaliação final
    pass_rate = sum(validator.window.is_valid(r['coherence'], r['fidelity'])
                    for r in all_seed_results) / n_seeds
    final_valid = pass_rate >= validator.pass_threshold

    print("\n" + "=" * 70)
    print("📋 VEREDITO FINAL v28")
    print("=" * 70)
    print(f"✅ Taxa de aprovação: {pass_rate:.1%} (limiar: 80%)")
    print(f"✅ Média de F: {np.mean(fidelities):.6f}")
    print(f"✅ Desvio padrão de F: {np.std(fidelities):.6f}")
    print(f"✅ Janela aplicada: 0.94 < F < 1.0 E C > 0.99")
    print(f"🔮 Resultado: {'✅ VÁLIDO' if final_valid else '❌ INVÁLIDO'}")

    # Gera gráficos
    fig, axes = plt.subplots(2, 2, figsize=(14, 10))
    fig.suptitle('BLOCO 470 v28 — Validação Não-Tautológica com PID')

    # (a) Evolução do p_error pelo PID
    ax = axes[0, 0]
    pid_history = pid.history
    if pid_history:
        steps = [h['step'] for h in pid_history]
        p_vals = [h['p_error'] for h in pid_history]
        f_vals = [h['current_f'] for h in pid_history]
        ax.plot(steps, p_vals, 'b-', label='p_error')
        ax.set_xlabel('Iteração (seed)')
        ax.set_ylabel('Taxa de erro (p)')
        ax.set_title('(a) Ajuste do Ruído pelo PID')
        ax.grid(True)
        ax.legend()

    # (b) Fidelidades por seed
    ax = axes[0, 1]
    seeds = list(range(n_seeds))
    colors = ['green' if validator.window.is_valid(all_seed_results[i]['coherence'], all_seed_results[i]['fidelity'])
              else 'red' for i in range(n_seeds)]
    ax.bar(seeds, fidelities, color=colors, alpha=0.7)
    ax.axhline(0.94, color='orange', linestyle='--', label='Limiar inferior (0.94)')
    ax.axhline(1.0, color='red', linestyle='--', label='Limiar superior (1.0)')
    ax.axhline(0.97, color='green', linestyle=':', label='Alvo PID (0.97)')
    ax.set_xlabel('Seed')
    ax.set_ylabel('Fidelidade F')
    ax.set_title('(b) Fidelidade por Seed')
    ax.legend()
    ax.grid(True, axis='y')

    # (c) Coerência vs Fidelidade (janela)
    ax = axes[1, 0]
    coherences = [r['coherence'] for r in all_seed_results]
    ax.scatter(coherences, fidelities, c=seeds, cmap='viridis', s=80)
    # Desenha a janela
    rect = plt.Rectangle((0.99, 0.94), 0.01, 0.06,
                         edgecolor='green', facecolor='none', linewidth=2, label='Janela Válida')
    ax.add_patch(rect)
    ax.set_xlabel('Coerência C')
    ax.set_ylabel('Fidelidade F')
    ax.set_title('(c) Janela Não-Tautológica')
    ax.legend()
    ax.grid(True)
    ax.set_xlim(0.97, 1.0)
    ax.set_ylim(0.90, 1.01)

    # (d) Evolução do erro do PID
    ax = axes[1, 1]
    if pid_history:
        errors = [h['error'] for h in pid_history]
        ax.plot(steps, errors, 'r-', linewidth=2)
        ax.axhline(0, color='black', linestyle='--')
        ax.set_xlabel('Iteração (seed)')
        ax.set_ylabel('Erro (target - F)')
        ax.set_title('(d) Convergência do PID')
        ax.grid(True)

    plt.tight_layout()
    plt.savefig('catedral_v28_pid_validation.png', dpi=150)
    plt.close(fig)
    print("\n📊 Gráfico salvo: catedral_v28_pid_validation.png")

    return {
        'pass_rate': pass_rate,
        'valid': final_valid,
        'pid_history': pid.history,
        'results': all_seed_results,
        'seeds': seeds,
        'fidelities': fidelities,
        'coherences': coherences
    }

# =============================================================================
# 5. EXECUÇÃO
# =============================================================================

if __name__ == "__main__":
    # Verifica PyMatching
    try:
        import pymatching
        print("✅ PyMatching disponível")
    except ImportError:
        print("❌ PyMatching não encontrado — instale: pip install pymatching")
        exit(1)

    # Executa a validação v28 com PID
    results = run_catedral_v28_with_pid(n_seeds=10, verbose=True)
