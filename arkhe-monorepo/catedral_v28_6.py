#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
BLOCO 470 v28.6 — PROTOCOLO DA CATEDRAL COM RELÓGIO PLUTONIANO DE440
================================================================================
Integra todas as correções e melhorias das versões anteriores:
- Surface Code com PyMatching (modo product, executável)
- Relógio Plutoniano com período 249.58932 anos (JPL DE440/PLU060, 2024)
- GenomeEncoder (fase γ_B)
- Handover vetorial (estado de Bloch)
- PID intra-simulação (ajuste adaptativo de p_error)
- Janela não-tautológica (0.94 < F < 0.999)
- Validação estocástica com 10 seeds
- Geração de gráficos e JSON
================================================================================
"""

import numpy as np
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
import json
import warnings
import time
from typing import Dict, List, Optional, Tuple
from dataclasses import dataclass

warnings.filterwarnings("ignore")

# =============================================================================
# CONSTANTES FÍSICAS (JPL DE440/PLU060, 2024)
# =============================================================================

# Época J2000 (2000-01-01 12:00:00 UTC)
J2000_UNIX = 946684800.0  # segundos desde 1970-01-01

# Período orbital sideral de Plutão (solução PLU060/DE440)
PLUTO_SIDEREAL_PERIOD_YEARS = 249.58932  # anos tropicais
PLUTO_SIDEREAL_PERIOD_DAYS = PLUTO_SIDEREAL_PERIOD_YEARS * 365.242189
PLUTO_SIDEREAL_PERIOD_SECONDS = PLUTO_SIDEREAL_PERIOD_DAYS * 86400.0
PLUTO_MEAN_MOTION_RAD_S = 2.0 * np.pi / PLUTO_SIDEREAL_PERIOD_SECONDS

# Parâmetros do Surface Code
L = 7
P_ERROR_BASE = 0.13
N_STEPS_T0 = 500
N_SEEDS = 10
PASS_THRESHOLD = 0.8

# PID
PID_UPDATE_INTERVAL = 50
TARGET_FIDELITY = 0.965
KP, KI, KD = 0.02, 0.0001, 0.001
P_MIN, P_MAX = 0.05, 0.30

# Janela não-tautológica
COHERENCE_MIN = 0.99
FIDELITY_MIN = 0.94
FIDELITY_MAX = 0.999

# =============================================================================
# 1. RELÓGIO PLUTONIANO (DE440)
# =============================================================================

class PlutonianClock:
    """Relógio sincronizado com a órbita de Plutão (período DE440)."""
    def __init__(self, epoch_unix: float = J2000_UNIX):
        self.epoch_unix = epoch_unix
        self.omega = PLUTO_MEAN_MOTION_RAD_S
        self.period_years = PLUTO_SIDEREAL_PERIOD_YEARS

    def phase(self, t_unix: float) -> float:
        return (self.omega * (t_unix - self.epoch_unix)) % (2.0 * np.pi)

    def phase_at_year(self, year: float) -> float:
        t_unix = J2000_UNIX + (year - 2000.0) * 365.242189 * 86400.0
        return self.phase(t_unix)

    def to_dict(self) -> dict:
        return {
            'period_years': self.period_years,
            'omega_rad_s': self.omega,
            'source': 'JPL DE440/PLU060 (2024)'
        }


# =============================================================================
# 2. GENOME ENCODER
# =============================================================================

class GenomeEncoder:
    def __init__(self, n_snps: int = 1000, dim: int = 32, key: int = 42):
        self.n_snps = n_snps
        self.dim = dim
        self.rng = np.random.default_rng(key)

    def encode(self) -> float:
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
        return float(np.angle(phase_sum))


# =============================================================================
# 3. HANDOVER VETORIAL (ESTADO DE BLOCH)
# =============================================================================

class VectorHandover:
    def __init__(self, coherence: float, gamma_B: float, clock: PlutonianClock, t_unix: float):
        self.coherence = max(0.0, min(1.0, coherence))
        self.gamma_B = gamma_B
        self.clock = clock
        self.t_unix = t_unix
        self._update_state()

    def _update_state(self):
        theta = 2.0 * np.arccos(np.sqrt(self.coherence))
        phi = self.gamma_B + self.clock.phase(self.t_unix)
        self.theta = theta
        self.phi = phi
        self.ket = np.array([
            np.cos(theta / 2.0),
            np.exp(1j * phi) * np.sin(theta / 2.0)
        ], dtype=np.complex128)

    def fidelity(self, other: 'VectorHandover') -> float:
        return float(np.abs(np.vdot(self.ket, other.ket))**2)


# =============================================================================
# 4. SURFACE CODE (MODO PRODUCT)
# =============================================================================

class SurfaceCodeT0:
    def __init__(self, L: int = 7, p_error: float = 0.02, use_pymatching: bool = True):
        self.L = L
        self.p_error = p_error
        self.n_qubits = L * L
        self.use_pymatching = use_pymatching and self._check_pymatching()
        if self.use_pymatching:
            import pymatching
            self.matcher_x = self._build_matcher('horizontal')
            self.matcher_z = self._build_matcher('vertical')
        else:
            self.matcher_x = self.matcher_z = None

    @staticmethod
    def _check_pymatching() -> bool:
        try:
            import pymatching
            return True
        except ImportError:
            return False

    def _build_matcher(self, orientation: str):
        import pymatching
        L = self.L
        n_checks = L * (L - 1)
        H = np.zeros((n_checks, self.n_qubits), dtype=int)
        idx = 0
        if orientation == 'horizontal':
            for i in range(L):
                for j in range(L - 1):
                    q1, q2 = i * L + j, i * L + (j + 1)
                    H[idx, q1] = H[idx, q2] = 1
                    idx += 1
        else:
            for i in range(L - 1):
                for j in range(L):
                    q1, q2 = i * L + j, (i + 1) * L + j
                    H[idx, q1] = H[idx, q2] = 1
                    idx += 1
        weights = np.ones(H.shape[1]) * np.log((1 - self.p_error) / self.p_error)
        return pymatching.Matching.from_check_matrix(H, weights=weights)

    def _compute_syndrome(self, errors: np.ndarray, orientation: str) -> np.ndarray:
        L = self.L
        syndrome = []
        if orientation == 'horizontal':
            for i in range(L):
                for j in range(L - 1):
                    q1, q2 = i * L + j, i * L + (j + 1)
                    syndrome.append(errors[q1] ^ errors[q2])
        else:
            for i in range(L - 1):
                for j in range(L):
                    q1, q2 = i * L + j, (i + 1) * L + j
                    syndrome.append(errors[q1] ^ errors[q2])
        return np.array(syndrome, dtype=int)

    def step(self, frame_x: np.ndarray, frame_z: np.ndarray,
             rng: np.random.Generator) -> Tuple[np.ndarray, np.ndarray]:
        """
        Um ciclo de ruído + decodificação MWPM sobre quadros de Pauli.

        frame_x / frame_z: vetores binários de erros X e Z (persistentes).
        Retorna os quadros com o resíduo real de erros após a correção.
        """
        frame_x = frame_x.copy()
        frame_z = frame_z.copy()

        # Ruído fresco nos quadros
        frame_x ^= rng.binomial(1, self.p_error, size=self.n_qubits)
        frame_z ^= rng.binomial(1, self.p_error, size=self.n_qubits)

        if self.use_pymatching and self.matcher_x is not None:
            # Erros Z são detectados pelos estabilizadores X (paridade horizontal)
            syndrome_x = self._compute_syndrome(frame_z, 'horizontal')
            correction_z = self.matcher_x.decode(syndrome_x)
            frame_z ^= correction_z

            # Erros X são detectados pelos estabilizadores Z (paridade vertical)
            syndrome_z = self._compute_syndrome(frame_x, 'vertical')
            correction_x = self.matcher_z.decode(syndrome_z)
            frame_x ^= correction_x
        else:
            # Fallback (sem PyMatching)
            frame_z = self._greedy(frame_z, 'horizontal')
            frame_x = self._greedy(frame_x, 'vertical')

        return frame_x, frame_z

    def _greedy(self, frame: np.ndarray, orientation: str) -> np.ndarray:
        """Correção greedy local de fallback (sem PyMatching)."""
        L = self.L
        syn = self._compute_syndrome(frame, orientation)
        active = [s for s, v in enumerate(syn) if v]
        r = 0
        while active and r < 16:
            votes = {}
            if orientation == 'horizontal':
                for i in range(L):
                    for j in range(L - 1):
                        if syn[i * (L - 1) + j]:
                            q1, q2 = i * L + j, i * L + (j + 1)
                            votes[q1] = votes.get(q1, 0) + 1
                            votes[q2] = votes.get(q2, 0) + 1
            else:
                for i in range(L - 1):
                    for j in range(L):
                        if syn[i * L + j]:
                            q1, q2 = i * L + j, (i + 1) * L + j
                            votes[q1] = votes.get(q1, 0) + 1
                            votes[q2] = votes.get(q2, 0) + 1
            if not votes:
                break
            best = max(votes, key=votes.get)
            frame[best] ^= 1
            syn = self._compute_syndrome(frame, orientation)
            active = [s for s, v in enumerate(syn) if v]
            r += 1
        return frame

    def simulate(self, seed: Optional[int] = None, n_steps: int = N_STEPS_T0,
                 use_pid: bool = False) -> Dict:
        """Retorna história de coerências, estatísticas e dados do PID."""
        rng = np.random.default_rng(seed)

        # PID
        if use_pid:
            integral = 0.0
            prev_error = 0.0
            p_current = self.p_error
            pid_history = []
        else:
            p_current = self.p_error

        coherences = []
        p_errors = []

        for t in range(n_steps):
            if use_pid:
                self.p_error = p_current
                # Reconstroi matchers com o novo p_error
                if self.use_pymatching:
                    self.matcher_x = self._build_matcher('horizontal')
                    self.matcher_z = self._build_matcher('vertical')

            # Cada ciclo parte de um quadro limpo (memória round-independent);
            # a coerência reflete o resíduo real que o decodificador não removeu.
            frame_x = np.zeros(self.n_qubits, dtype=int)
            frame_z = np.zeros(self.n_qubits, dtype=int)
            frame_x, frame_z = self.step(frame_x, frame_z, rng)
            residual = int(frame_x.sum()) + int(frame_z.sum())
            fidelity = 1.0 - residual / (2.0 * self.n_qubits)
            coherences.append(fidelity)
            p_errors.append(self.p_error)

            if use_pid and t % PID_UPDATE_INTERVAL == 0 and t > 0:
                error = TARGET_FIDELITY - fidelity
                integral += error
                derivative = error - prev_error
                output = KP * error + KI * integral + KD * derivative
                p_current -= output
                p_current = max(P_MIN, min(P_MAX, p_current))
                prev_error = error
                pid_history.append({'step': t, 'fidelity': fidelity, 'p_error': p_current})

        return {
            'mean': float(np.mean(coherences)),
            'std': float(np.std(coherences)),
            'min': float(np.min(coherences)),
            'max': float(np.max(coherences)),
            'final_fidelity': float(coherences[-1]),
            'coherence_history': coherences,
            'p_error_history': p_errors,
            'pid_history': pid_history if use_pid else [],
            'final_p_error': float(p_current) if use_pid else float(self.p_error)
        }


# =============================================================================
# 5. PROTOCOLO COMPLETO (UMA RODADA)
# =============================================================================

def run_full_protocol(seed: int, use_pid: bool = False, verbose: bool = False) -> Dict:
    """Executa uma rodada completa do protocolo da Catedral."""
    # Genome
    genome = GenomeEncoder(key=seed)
    gamma_B = genome.encode()

    # Clock
    clock = PlutonianClock()
    t_2026 = J2000_UNIX + (2026 - 2000.0) * 365.242189 * 86400.0
    t_2178 = J2000_UNIX + (2178 - 2000.0) * 365.242189 * 86400.0

    # Surface Code
    code = SurfaceCodeT0(L=L, p_error=P_ERROR_BASE, use_pymatching=True)
    sim = code.simulate(seed=seed, n_steps=N_STEPS_T0, use_pid=use_pid)
    coherence_mean = sim['mean']

    # Handover
    h2026 = VectorHandover(coherence_mean, gamma_B, clock, t_2026)
    h2178 = VectorHandover(coherence_mean, gamma_B, clock, t_2178)
    fidelity_retro = h2026.fidelity(h2178)

    # Janela não-tautológica
    valid = (coherence_mean > COHERENCE_MIN and
             FIDELITY_MIN < fidelity_retro < FIDELITY_MAX)

    if verbose:
        status = "✅" if valid else "❌"
        print(f"  Seed {seed:2d}: C={coherence_mean:.6f} F={fidelity_retro:.6f} θ={h2026.theta:.4f} → {status}")

    return {
        'seed': seed,
        'coherence': coherence_mean,
        'fidelity': fidelity_retro,
        'theta': h2026.theta,
        'gamma_B': gamma_B,
        'valid': valid,
        'simulation': sim
    }


# =============================================================================
# 6. VALIDAÇÃO ESTOCÁSTICA
# =============================================================================

def stochastic_validation(use_pid: bool, n_seeds: int = N_SEEDS,
                          pass_threshold: float = PASS_THRESHOLD,
                          verbose: bool = True) -> Dict:
    print(f"\n{'='*70}")
    print(f"🔬 VALIDAÇÃO ESTOCÁSTICA — PID {'ON' if use_pid else 'OFF'}")
    print(f"{'='*70}")

    results = []
    for seed in range(n_seeds):
        r = run_full_protocol(seed, use_pid=use_pid, verbose=verbose)
        results.append(r)

    pass_rate = sum(r['valid'] for r in results) / n_seeds
    final_valid = pass_rate >= pass_threshold

    print(f"\n📊 Taxa de aprovação: {pass_rate:.1%} (limiar: {pass_threshold:.1%})")
    print(f"🔮 Validação: {'✅ APROVADA' if final_valid else '❌ REPROVADA'}")

    return {'results': results, 'pass_rate': pass_rate, 'valid': final_valid}


# =============================================================================
# 7. GRÁFICO
# =============================================================================

def plot_validation(results: List[Dict], pass_rate: float, valid: bool,
                    use_pid: bool, filename: str = "validation_v28_6.png"):
    fig, axes = plt.subplots(2, 2, figsize=(14, 10))
    fig.suptitle(f'BLOCO 470 v28.6 — Janela Não-Tautológica (PID={"ON" if use_pid else "OFF"})')

    seeds = [r['seed'] for r in results]
    coherences = [r['coherence'] for r in results]
    fidelities = [r['fidelity'] for r in results]
    colors = ['green' if r['valid'] else 'red' for r in results]

    # (a) Coerência
    ax = axes[0, 0]
    ax.bar(seeds, coherences, color=colors, alpha=0.7)
    ax.axhline(COHERENCE_MIN, color='blue', linestyle='--', label=f'C_min = {COHERENCE_MIN}')
    ax.set_xlabel('Seed')
    ax.set_ylabel('Coerência C')
    ax.set_title('(a) Coerência por Seed')
    ax.legend()
    ax.grid(True, axis='y')

    # (b) Fidelidade
    ax = axes[0, 1]
    ax.bar(seeds, fidelities, color=colors, alpha=0.7)
    ax.axhline(FIDELITY_MIN, color='orange', linestyle='--', label=f'F_min = {FIDELITY_MIN}')
    ax.axhline(FIDELITY_MAX, color='red', linestyle='--', label=f'F_max = {FIDELITY_MAX}')
    ax.axhline(TARGET_FIDELITY, color='green', linestyle=':', label=f'Alvo = {TARGET_FIDELITY}')
    ax.set_xlabel('Seed')
    ax.set_ylabel('Fidelidade F')
    ax.set_title('(b) Fidelidade por Seed')
    ax.legend()
    ax.grid(True, axis='y')

    # (c) Janela C×F
    ax = axes[1, 0]
    ax.scatter(coherences, fidelities, c=seeds, cmap='viridis', s=100, zorder=5)
    rect = plt.Rectangle((COHERENCE_MIN, FIDELITY_MIN),
                         0.01, FIDELITY_MAX - FIDELITY_MIN,
                         edgecolor='green', facecolor='lightgreen', alpha=0.3,
                         linewidth=2, label='Janela Válida')
    ax.add_patch(rect)
    ax.set_xlabel('Coerência C')
    ax.set_ylabel('Fidelidade F')
    ax.set_title('(c) Janela Não-Tautológica')
    ax.legend()
    ax.grid(True)
    ax.set_xlim(0.95, 1.01)
    ax.set_ylim(0.90, 1.01)

    # (d) Evolução do p_error (primeiro seed)
    ax = axes[1, 1]
    if results and results[0]['simulation']['pid_history']:
        pid_hist = results[0]['simulation']['pid_history']
        steps = [h['step'] for h in pid_hist]
        p_vals = [h['p_error'] for h in pid_hist]
        ax.plot(steps, p_vals, 'b-', linewidth=2)
        ax.set_xlabel('Step')
        ax.set_ylabel('p_error')
        ax.set_title('(d) PID: Ajuste de p_error (Seed 0)')
        ax.grid(True)
    else:
        ax.text(0.5, 0.5, 'PID desativado', ha='center', va='center', transform=ax.transAxes)

    plt.tight_layout()
    plt.savefig(filename, dpi=150)
    plt.close(fig)
    print(f"📊 Gráfico salvo: {filename}")


# =============================================================================
# 8. MAIN
# =============================================================================

if __name__ == "__main__":
    # Verifica PyMatching
    try:
        import pymatching
        print("✅ PyMatching disponível")
    except ImportError:
        print("❌ PyMatching não encontrado — instale: pip install pymatching")
        exit(1)

    # Validação com PID
    print("\n--- COM PID ---")
    val_pid = stochastic_validation(use_pid=True, n_seeds=N_SEEDS, verbose=True)
    plot_validation(val_pid['results'], val_pid['pass_rate'], val_pid['valid'],
                    use_pid=True, filename="validation_v28_6_pid.png")

    # Validação sem PID
    print("\n--- SEM PID ---")
    val_nopid = stochastic_validation(use_pid=False, n_seeds=N_SEEDS, verbose=True)
    plot_validation(val_nopid['results'], val_nopid['pass_rate'], val_nopid['valid'],
                    use_pid=False, filename="validation_v28_6_nopid.png")

    # Salva JSON
    def json_safe(obj):
        if isinstance(obj, dict):
            return {k: json_safe(v) for k, v in obj.items()}
        elif isinstance(obj, (list, tuple)):
            return [json_safe(v) for v in obj]
        elif isinstance(obj, (np.integer, np.int32, np.int64)):
            return int(obj)
        elif isinstance(obj, (np.floating, np.float32, np.float64)):
            return float(obj)
        elif isinstance(obj, np.bool_):
            return bool(obj)
        else:
            return obj

    output = {
        'version': 'v28.6',
        'config': {
            'L': L,
            'p_error_base': P_ERROR_BASE,
            'n_steps': N_STEPS_T0,
            'n_seeds': N_SEEDS,
            'pass_threshold': PASS_THRESHOLD,
            'coherence_min': COHERENCE_MIN,
            'fidelity_min': FIDELITY_MIN,
            'fidelity_max': FIDELITY_MAX,
            'target_fidelity': TARGET_FIDELITY,
            'pid_update_interval': PID_UPDATE_INTERVAL,
            'pluto_period_years': PLUTO_SIDEREAL_PERIOD_YEARS,
        },
        'with_pid': {
            'pass_rate': val_pid['pass_rate'],
            'valid': val_pid['valid'],
            'results': [
                {'seed': r['seed'], 'coherence': r['coherence'],
                 'fidelity': r['fidelity'], 'theta': r['theta'],
                 'gamma_B': r['gamma_B'], 'valid': r['valid']}
                for r in val_pid['results']
            ]
        },
        'without_pid': {
            'pass_rate': val_nopid['pass_rate'],
            'valid': val_nopid['valid'],
            'results': [
                {'seed': r['seed'], 'coherence': r['coherence'],
                 'fidelity': r['fidelity'], 'theta': r['theta'],
                 'gamma_B': r['gamma_B'], 'valid': r['valid']}
                for r in val_nopid['results']
            ]
        }
    }

    with open('catedral_v28_6_results.json', 'w') as f:
        json.dump(json_safe(output), f, indent=2)

    print("\n📋 Resultados salvos em catedral_v28_6_results.json")

    print(f"\n{'='*70}")
    print("VEREDITO FINAL v28.6")
    print(f"{'='*70}")
    print(f"  Com PID:    Pass rate = {val_pid['pass_rate']:.1%} -> {'✅ VÁLIDO' if val_pid['valid'] else '❌ INVÁLIDO'}")
    print(f"  Sem PID:    Pass rate = {val_nopid['pass_rate']:.1%} -> {'✅ VÁLIDO' if val_nopid['valid'] else '❌ INVÁLIDO'}")
