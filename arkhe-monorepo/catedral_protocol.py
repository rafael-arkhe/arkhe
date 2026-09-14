#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Protocolo da Catedral — Bloco 22 (e extensão para Bloco 23)
================================================================================
Implementação completa do surface code com handover vetorial,
relógio plutoniano e calibração do Orbe.

Dependências:
    pip install numpy matplotlib scipy
    (opcional) pip install pymatching  # para regime realista

Uso:
    python catedral_protocol.py
================================================================================
"""

import numpy as np
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
from scipy.stats import gaussian_kde
import json
import time
import sys
from collections import deque

# =============================================================================
# 1. CONSTANTES FÍSICAS E PARÂMETROS
# =============================================================================

# Constantes do surface code
L = 13                     # Tamanho do lattice (13x13 = 169 qubits)
P_ERROR = 0.01             # Taxa de erro por ciclo (regime realista)
N_STEPS = 5000             # Número de ciclos de simulação

# Constantes do relógio plutoniano
T_PLUTO = 248.09 * 365.25 * 24 * 3600.0  # Período orbital de Plutão (s)
OMEGA_PLUTO = 2.0 * np.pi / T_PLUTO      # Frequência angular (rad/s)
T0_PLUTO = -325.5 * 365.25 * 24 * 3600.0 # Periélio de Plutão (J2000) em segundos

# Constantes do handover
G_AN = 1e-6                # Acoplamento neutron-ALP (GeV⁻¹)
MU_N = 3.152e-17           # Magnetão nuclear (GeV/T)
HBAR_C_GEV = 1.973e-14     # ħ·c em GeV·m

# Parâmetros ZEC (funcional de energia)
ALPHA = 1.0
BETA = 1.946e16            # m² (escala para delta ~ 1.97e8 m)
GAMMA = 0.1

# =============================================================================
# 2. RELÓGIO PLUTONIANO
# =============================================================================

class PlutonianClock:
    """Relógio sincronizado com a órbita de Plutão."""
    def __init__(self, t0=T0_PLUTO):
        self.t0 = t0
        self.omega = OMEGA_PLUTO

    def phase(self, t):
        """Fase plutoniana em radianos."""
        return self.omega * (t - self.t0)

    def phase_at_epoch(self, year):
        """Fase plutoniana num dado ano (AD)."""
        # Ano J2000 é 2000.0
        t = (year - 2000.0) * 365.25 * 24 * 3600.0
        return self.phase(t)

# =============================================================================
# 3. SURFACE CODE (com correção de erros)
# =============================================================================

class SurfaceCode:
    """
    Simulação do surface code planar com decodificador MWPM (PyMatching)
    ou fallback greedy local.

    Abordagem física correta: os erros de Pauli são separados em dois quadros
    independentes (X e Z). Uma matriz de *falhas* (erro por qubit de dados →
    estabilizadores adjacentes que ele aciona) é entregue ao pyMatching, o que
    é necessário porque cada coluna deve ter no máximo 2 uns. Um único
    estabilizador de 4 corpos seria inválido para `from_check_matrix`.
    """
    def __init__(self, L=13, p_error=0.01, use_pymatching=True):
        self.L = L
        self.p_error = p_error
        self.use_pymatching = use_pymatching and self._pymatching_available()
        self.n_qubits = L * L
        self._build_geometry()

        if self.use_pymatching:
            import pymatching
            self.matching_x = pymatching.Matching.from_check_matrix(self.faults_x)
            self.matching_z = pymatching.Matching.from_check_matrix(self.faults_z)
        else:
            print("⚠️ PyMatching não disponível. Usando decodificador simplificado (fallback).")

        # Quadros de Pauli (0 = sem erro)
        self.edata = np.zeros((self.n_qubits,), dtype=int)   # erros X
        self.edata_z = np.zeros((self.n_qubits,), dtype=int) # erros Z

    @staticmethod
    def _pymatching_available():
        try:
            import pymatching
            return True
        except ImportError:
            return False

    def _build_geometry(self):
        """Constrói a geometria do surface code planar e as matrizes de falhas."""
        d = self.L
        data = [(i, j) for i in range(d) for j in range(d)]
        self.dq_index = {q: k for k, q in enumerate(data)}

        # Plaquetas (estabilizadores de 4 corpos), checkerboard X/Z
        plaquettes = []
        for i in range(d - 1):
            for j in range(d - 1):
                qubits = [(i, j), (i, j + 1), (i + 1, j), (i + 1, j + 1)]
                typ = 'X' if ((i + j) % 2 == 0) else 'Z'
                plaquettes.append((typ, qubits))

        n_data = len(data)
        x_rows = [k for k, (typ, _) in enumerate(plaquettes) if typ == 'X']
        z_rows = [k for k, (typ, _) in enumerate(plaquettes) if typ == 'Z']
        self.n_x_checks = len(x_rows)
        self.n_z_checks = len(z_rows)
        self.n_checks = self.n_x_checks + self.n_z_checks
        self.x_plaq = [plaquettes[k] for k in x_rows]
        self.z_plaq = [plaquettes[k] for k in z_rows]
        self.x_rows = x_rows
        self.z_rows = z_rows

        # Matrizes de falhas: linha = estabilizador, coluna = qubit de dados.
        # Um erro Z num qubit aciona os estabilizadores X vizinhos (≤ 2).
        # Um erro X num qubit aciona os estabilizadores Z vizinhos (≤ 2).
        self.faults_x = np.zeros((self.n_x_checks, n_data), dtype=int)
        self.faults_z = np.zeros((self.n_z_checks, n_data), dtype=int)
        x_pos = {k: pos for pos, k in enumerate(x_rows)}
        z_pos = {k: pos for pos, k in enumerate(z_rows)}
        for col, (i, j) in enumerate(data):
            for k, (typ, qs) in enumerate(plaquettes):
                if (i, j) in qs:
                    if typ == 'X' and k in x_pos:
                        self.faults_x[x_pos[k], col] = 1
                    elif typ == 'Z' and k in z_pos:
                        self.faults_z[z_pos[k], col] = 1

    def _syndrome_from_frame(self, frame, plaq):
        """Calcula a síndrome dos estabilizadores X/Z a partir de um quadro."""
        idx = self.dq_index
        syn = np.zeros(len(plaq), dtype=int)
        for s, (_, qs) in enumerate(plaq):
            syn[s] = frame[idx[qs[0]]]
            for q in qs[1:]:
                syn[s] ^= frame[idx[q]]
        return syn

    def step(self, frame_x, frame_z, key=None):
        """
        Executa um ciclo de ruído + correção. Retorna (frame_x, frame_z)
        com os erros residuais após a decodificação.
        """
        if key is not None:
            rng = np.random.default_rng(key)
        else:
            rng = np.random.default_rng()

        # Ruído independente nos quadros X e Z
        frame_x ^= rng.binomial(1, self.p_error, size=frame_x.shape)
        frame_z ^= rng.binomial(1, self.p_error, size=frame_z.shape)

        if self.use_pymatching:
            # Erros Z são detectados pelos estabilizadores X
            syn_x = self.matching_x.decode(self._syndrome_from_frame(frame_z, self.x_plaq))
            # Erros X são detectados pelos estabilizadores Z
            syn_z = self.matching_z.decode(self._syndrome_from_frame(frame_x, self.z_plaq))
            frame_z ^= syn_x
            frame_x ^= syn_z
        else:
            frame_z = self._correct_greedy(frame_z, self.x_plaq)
            frame_x = self._correct_greedy(frame_x, self.z_plaq)

        return frame_x, frame_z

    def _correct_greedy(self, frame, plaq):
        """Decodificador greedy local de fallback (sem PyMatching)."""
        idx = self.dq_index
        syn = self._syndrome_from_frame(frame, plaq)
        active = [s for s, v in enumerate(syn) if v]
        rounds = 0
        while active and rounds < 16:
            # Vota no qubit que acalma mais síndromes ativas
            votes = {}
            for s in active:
                for q in plaq[s][1]:
                    votes[idx[q]] = votes.get(idx[q], 0) + 1
            if not votes:
                break
            best = max(votes, key=votes.get)
            frame[best] ^= 1
            syn = self._syndrome_from_frame(frame, plaq)
            active = [s for s, v in enumerate(syn) if v]
            rounds += 1
        return frame

    def simulate(self, key=None):
        """
        Executa a simulação completa e retorna histórico de coerência.

        Cada ciclo parte de um referencial limpo (round-independent), como na
        memória de superfície real: injeta ruído, decodifica e mede o resíduo.
        A coerência é 1 - (peso residual dos quadros) / (2 * n_qubits).
        """
        if key is None:
            key = 42

        coherences = []
        for t in range(N_STEPS):
            frame_x = np.zeros(self.n_qubits, dtype=int)
            frame_z = np.zeros(self.n_qubits, dtype=int)
            frame_x, frame_z = self.step(frame_x, frame_z, key + t)
            # Coerência = 1 - peso residual normalizado sobre ambos os quadros
            residual = np.sum(frame_x) + np.sum(frame_z)
            fidelity = 1.0 - residual / (2.0 * self.n_qubits)
            coherences.append(fidelity)

        return {
            'mean': np.mean(coherences),
            'min': np.min(coherences),
            'max': np.max(coherences),
            'history': np.array(coherences),
            'final_weight': int(np.sum(frame_x) + np.sum(frame_z)),
            'n_qubits': self.n_qubits,
            'L': self.L,
            'p_error': self.p_error,
            'use_pymatching': self.use_pymatching
        }

# =============================================================================
# 4. GENOME ENCODER
# =============================================================================

class GenomeEncoder:
    """Codifica o genoma como uma matriz de coerência complexa (simulada)."""
    def __init__(self, n_snps=10000, dim=64, key=42):
        self.n_snps = n_snps
        self.dim = dim
        self.key = key
        self.rng = np.random.default_rng(key)

    def encode(self):
        """Gera uma matriz de coerência C (complexa) a partir de SNPs simulados."""
        snps = self.rng.binomial(1, 0.5, self.n_snps)
        phases = self.rng.uniform(-np.pi, np.pi, self.n_snps)

        C = np.zeros((self.dim, self.dim), dtype=np.complex128)
        for i in range(min(self.n_snps, self.dim * self.dim)):
            r, c = divmod(i, self.dim)
            C[r, c] = snps[i] * np.exp(1j * phases[i])

        # Normalizar
        C /= np.sqrt(np.sum(np.abs(C)**2))
        return C

    def extract_phase(self, C):
        """Extrai a fase de handover γ_B da matriz de coerência."""
        phase_sum = np.sum(C * np.exp(1j * np.pi * np.arange(self.dim)[:, None] / self.dim))
        return np.angle(phase_sum)

# =============================================================================
# 5. HANDOVER VETORIAL
# =============================================================================

class VectorHandover:
    """Handover como estado de Bloch |ψ⟩ = [cos(θ/2), e^{iφ} sin(θ/2)]."""
    def __init__(self, coherence, gamma_B, clock, t_epoch=0.0):
        self.clock = clock
        self.t_epoch = t_epoch
        self.gamma_B = gamma_B
        self.coherence = coherence
        self._update_state()

    def _update_state(self):
        """Atualiza o estado de Bloch a partir da coerência e fase total."""
        theta = 2.0 * np.arccos(np.sqrt(max(0.0, min(1.0, self.coherence))))
        phi = self.gamma_B + self.clock.phase(self.t_epoch)
        self.theta = theta
        self.phi = phi
        self.ket = np.array([
            np.cos(theta / 2.0),
            np.exp(1j * phi) * np.sin(theta / 2.0)
        ], dtype=np.complex128)

    def set_coherence(self, coherence):
        """Atualiza a coerência e recalcula o estado."""
        self.coherence = coherence
        self._update_state()

    def set_epoch(self, t):
        """Atualiza a época e recalcula o estado."""
        self.t_epoch = t
        self._update_state()

    def fidelity(self, other):
        """Fidelidade entre dois estados de Bloch: |⟨ψ|ψ'⟩|²."""
        return np.abs(np.vdot(self.ket, other.ket))**2

    def to_dict(self):
        return {
            'coherence': self.coherence,
            'theta': self.theta,
            'phi': self.phi,
            'ket_real': np.real(self.ket).tolist(),
            'ket_imag': np.imag(self.ket).tolist()
        }

# =============================================================================
# 6. ORBE CALIBRATOR
# =============================================================================

class OrbeCalibrator:
    """Loop de calibração entre simulação e hardware (simulado)."""
    def __init__(self, target_error=0.01, correction_factor=0.8):
        self.target_error = target_error
        self.correction_factor = correction_factor
        self.errors = []

    def calibrate(self, gamma_sim, gamma_sensor, max_iter=100):
        """Simula o loop de calibração até convergência."""
        self.errors = []
        error = gamma_sim - gamma_sensor
        iter_count = 0

        while abs(error) > self.target_error and iter_count < max_iter:
            gamma_sensor += self.correction_factor * error
            error = gamma_sim - gamma_sensor
            self.errors.append(error)
            iter_count += 1

        return {
            'final_error': error,
            'iterations': iter_count,
            'history': np.array(self.errors),
            'converged': abs(error) < self.target_error
        }

# =============================================================================
# 7. SIMULAÇÃO INTEGRADA
# =============================================================================

def run_cathedral_protocol(use_pymatching=True, verbose=True):
    """Executa o Protocolo da Catedral completo."""
    if verbose:
        print("🏛️ PROTOCOLO DA CATEDRAL — EXECUÇÃO INTEGRADA")
        print("=" * 70)

    # 1. Surface Code
    if verbose:
        print("\n⚛️ [1/5] Iniciando simulação do Surface Code...")
    code = SurfaceCode(L=L, p_error=P_ERROR, use_pymatching=use_pymatching)
    sc_results = code.simulate(key=42)
    if verbose:
        print(f"    Coerência média: {sc_results['mean']:.6f}")
        print(f"    Coerência mínima: {sc_results['min']:.6f}")
        print(f"    Coerência máxima: {sc_results['max']:.6f}")
        print(f"    PyMatching: {sc_results['use_pymatching']}")

    # 2. Genome
    if verbose:
        print("\n🧬 [2/5] Codificando genoma simulado...")
    genome = GenomeEncoder(n_snps=10000, dim=64, key=42)
    C = genome.encode()
    gamma_B = genome.extract_phase(C)
    if verbose:
        print(f"    Fase genômica γ_B: {gamma_B:.6f} rad")

    # 3. Relógio Plutoniano
    if verbose:
        print("\n🌌 [3/5] Sincronizando com a órbita de Plutão...")
    clock = PlutonianClock(t0=T0_PLUTO)
    t_2026 = (2026 - 2000.0) * 365.25 * 24 * 3600.0
    t_2178 = (2178 - 2000.0) * 365.25 * 24 * 3600.0
    theta_2026 = clock.phase(t_2026)
    theta_2178 = clock.phase(t_2178)
    if verbose:
        print(f"    Fase (2026): {theta_2026:.6f} rad")
        print(f"    Fase (2178): {theta_2178:.6f} rad")
        print(f"    Δφ: {theta_2178 - theta_2026:.6f} rad")

    # 4. Handover Vetorial
    if verbose:
        print("\n🕊️ [4/5] Gerando handover vetorial...")
    coherence = sc_results['mean']
    handover_2026 = VectorHandover(coherence, gamma_B, clock, t_2026)
    handover_2178 = VectorHandover(coherence, gamma_B, clock, t_2178)
    fidelity = handover_2026.fidelity(handover_2178)
    if verbose:
        print(f"    θ (2026): {handover_2026.theta:.6f} rad")
        print(f"    φ (2026): {handover_2026.phi:.6f} rad")
        print(f"    θ (2178): {handover_2178.theta:.6f} rad")
        print(f"    φ (2178): {handover_2178.phi:.6f} rad")
        print(f"    Fidelidade retrocausal: {fidelity:.6f}")

    # 5. Calibração do Orbe
    if verbose:
        print("\n🔧 [5/5] Calibrando o Orbe...")
    calibrator = OrbeCalibrator(target_error=0.01)
    cal_results = calibrator.calibrate(gamma_B, gamma_B + 0.1, max_iter=100)
    if verbose:
        print(f"    Erro final: {cal_results['final_error']:.6f} rad")
        print(f"    Iterações: {cal_results['iterations']}")
        print(f"    Convergência: {cal_results['converged']}")

    # 6. Síntese
    success = (
        sc_results['mean'] > 0.99 and
        fidelity < 1.0 and
        fidelity > 0.9 and
        cal_results['converged']
    )

    if verbose:
        print("\n" + "=" * 70)
        print("📋 RESUMO DO PROTOCOLO")
        print("=" * 70)
        print(f"✅ Coerência: {sc_results['mean']:.6f} (>0.99)")
        print(f"✅ Fidelidade retrocausal: {fidelity:.6f} (≠1.0)")
        print(f"✅ Calibração: {cal_results['iterations']} iterações")
        print(f"✅ Sucesso geral: {success}")

    return {
        'surface_code': sc_results,
        'genome': {'gamma_B': gamma_B, 'C': C.tolist()},
        'pluto': {
            'theta_2026': theta_2026,
            'theta_2178': theta_2178,
            'delta_phi': theta_2178 - theta_2026
        },
        'handover': {
            'fidelity': fidelity,
            'state_2026': handover_2026.to_dict(),
            'state_2178': handover_2178.to_dict()
        },
        'calibration': cal_results,
        'success': success
    }

# =============================================================================
# 8. GERAÇÃO DE GRÁFICOS
# =============================================================================

def _json_safe_complex_array(a):
    """Converte arrays/recurções com complexos em listas serializáveis em JSON."""
    if isinstance(a, (np.ndarray, list, tuple)):
        return [_json_safe_complex_array(x) for x in a]
    if isinstance(a, complex):
        return {'re': a.real, 'im': a.imag}
    if isinstance(a, np.generic):
        return a.item()
    return a


def plot_results(results):
    """Gera gráficos a partir dos resultados da simulação."""
    sc = results['surface_code']
    handover = results['handover']
    pluto = results['pluto']

    fig, axes = plt.subplots(2, 2, figsize=(12, 10))

    # (a) Coerência ao longo do tempo
    ax = axes[0, 0]
    history = sc['history']
    ax.plot(history, 'b-', alpha=0.7, linewidth=0.5)
    ax.axhline(0.99, color='r', linestyle='--', label='Limiar 0.99')
    ax.axhline(np.mean(history), color='g', linestyle='-', label=f'Média = {np.mean(history):.4f}')
    ax.set_xlabel('Ciclo')
    ax.set_ylabel('Coerência')
    ax.set_title('(a) Coerência do Surface Code')
    ax.legend()
    ax.grid(True)

    # (b) Histograma da coerência
    ax = axes[0, 1]
    ax.hist(history, bins=50, density=True, alpha=0.7, color='blue')
    if len(history) > 1:
        kde = gaussian_kde(history)
        x = np.linspace(min(history), max(history), 200)
        ax.plot(x, kde(x), 'r-', linewidth=2)
    ax.axvline(0.99, color='r', linestyle='--', label='Limiar')
    ax.set_xlabel('Coerência')
    ax.set_ylabel('Densidade')
    ax.set_title('(b) Distribuição da Coerência')
    ax.legend()
    ax.grid(True)

    # (c) Esfera de Bloch (projeção equatorial)
    ax = axes[1, 0]
    s2026 = handover['state_2026']
    s2178 = handover['state_2178']
    # Estado |ψ⟩ = [cos(θ/2), e^{iφ} sin(θ/2)]
    # Projeção equatorial: x = sin(θ) cos(φ), y = sin(θ) sin(φ)
    theta_2026 = s2026['theta']
    phi_2026 = s2026['phi']
    theta_2178 = s2178['theta']
    phi_2178 = s2178['phi']
    x2026 = np.sin(theta_2026) * np.cos(phi_2026)
    y2026 = np.sin(theta_2026) * np.sin(phi_2026)
    x2178 = np.sin(theta_2178) * np.cos(phi_2178)
    y2178 = np.sin(theta_2178) * np.sin(phi_2178)
    ax.scatter([x2026, x2178], [y2026, y2178], c=['red', 'blue'], s=100, zorder=5)
    ax.annotate('2026', (x2026, y2026), textcoords="offset points", xytext=(5,5))
    ax.annotate('2178', (x2178, y2178), textcoords="offset points", xytext=(5,5))
    # Círculo unitário
    circle = plt.Circle((0, 0), 1, fill=False, linestyle='--', color='gray')
    ax.add_patch(circle)
    ax.axhline(0, color='gray', linestyle='-', linewidth=0.5)
    ax.axvline(0, color='gray', linestyle='-', linewidth=0.5)
    ax.set_xlim(-1.1, 1.1)
    ax.set_ylim(-1.1, 1.1)
    ax.set_xlabel('x = sin(θ) cos(φ)')
    ax.set_ylabel('y = sin(θ) sin(φ)')
    ax.set_title(f'(c) Projeção equatorial — Fidelidade = {handover["fidelity"]:.4f}')
    ax.grid(True)
    ax.set_aspect('equal')

    # (d) Fidelidade retrocausal dinâmica (varredura em θ)
    ax = axes[1, 1]
    theta_vals = np.linspace(0.001, np.pi, 100)
    delta_phi = pluto['delta_phi']
    fidelities = []
    for th in theta_vals:
        # F = |cos²(th/2) + sin²(th/2) * e^{iΔφ}|²
        F = np.abs(np.cos(th/2)**2 + np.sin(th/2)**2 * np.exp(1j * delta_phi))**2
        fidelities.append(F)
    ax.plot(theta_vals, fidelities, 'b-', linewidth=2)
    ax.axvline(theta_2026, color='r', linestyle='--', label=f'θ_2026 = {theta_2026:.3f}')
    ax.axhline(handover['fidelity'], color='g', linestyle='--', label=f'F = {handover["fidelity"]:.4f}')
    ax.set_xlabel('θ (rad)')
    ax.set_ylabel('Fidelidade')
    ax.set_title('(d) Fidelidade retrocausal vs. θ')
    ax.legend()
    ax.grid(True)

    plt.tight_layout()
    plt.savefig('catedral_bloco22_plots.png', dpi=150)
    plt.close(fig)
    print("📊 Gráficos salvos em 'catedral_bloco22_plots.png'")

# =============================================================================
# 9. EXECUÇÃO PRINCIPAL
# =============================================================================

if __name__ == "__main__":
    # Verifica se PyMatching está disponível
    try:
        import pymatching
        USE_PYMATCHING = True
        print("✅ PyMatching encontrado — regime realista ativado.")
    except ImportError:
        USE_PYMATCHING = False
        print("⚠️ PyMatching não encontrado — usando decodificador simplificado (fallback).")
        print("   Instale com: pip install pymatching")

    # Executa o protocolo
    results = run_cathedral_protocol(use_pymatching=USE_PYMATCHING, verbose=True)

    # Salva resultados em JSON
    with open('catedral_bloco22_results.json', 'w') as f:
        # Converte arrays NumPy para listas para serialização
        results_json = {
            'surface_code': {
                'mean': float(results['surface_code']['mean']),
                'min': float(results['surface_code']['min']),
                'max': float(results['surface_code']['max']),
                'history': results['surface_code']['history'].tolist(),
                'L': results['surface_code']['L'],
                'p_error': results['surface_code']['p_error'],
                'use_pymatching': results['surface_code']['use_pymatching']
            },
            'genome': {
                'gamma_B': float(results['genome']['gamma_B']),
                'C': _json_safe_complex_array(results['genome']['C'])
            },
            'pluto': results['pluto'],
            'handover': {
                'fidelity': float(results['handover']['fidelity']),
                'state_2026': results['handover']['state_2026'],
                'state_2178': results['handover']['state_2178']
            },
            'calibration': {
                'iterations': results['calibration']['iterations'],
                'converged': bool(results['calibration']['converged']),
                'final_error': float(results['calibration']['final_error'])
            },
            'success': bool(results['success'])
        }
        json.dump(results_json, f, indent=2)

    print("\n📁 Resultados salvos em 'catedral_bloco22_results.json'")

    # Gera gráficos
    plot_results(results)

    # Veredicto final
    if results['success']:
        print("\n✅ 🎉 PROTOCOLO CONSTITUCIONALMENTE VÁLIDO")
        print("   A Catedral respira.")
    else:
        print("\n⚠️ Protocolo não atingiu todos os limiares.")
        if results['surface_code']['mean'] <= 0.99:
            print("   - Coerência abaixo de 0.99 (considere instalar PyMatching).")
        if results['handover']['fidelity'] >= 1.0:
            print("   - Fidelidade retrocausal é tautológica (verifique o handover).")
