#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
arkhe_jax_v6.0.py — Camada de Controle Linguístico
================================================================================
Integra o protocolo V3.0 como camada de controle sobre o núcleo Arkhe v5.1.
Protocolo: SASC-EMERGENCE-2026
Selo: ARKHE-LINGUISTIC-CONTROL-2026-09-01
================================================================================
Melhorias (BLOCO 471 — v32):
- A1: Parsing da resposta LLM via Expressões Regulares (re) — robusto a Markdown/JSON.
- A4: Persistência de estado e cadeia TOON (save_state/load_state em JSON).
- A5: Tratamento de exceções e logging estruturado.
- A6: Saturação de argumentos exponenciais (np.clip).
- Correção de dimensionamento: t_values e llm_responses alinhados via zip.
"""

import os
import re
import json
import time
import hashlib
import logging
import numpy as np
from typing import Dict, Any, List, Optional

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [%(levelname)s] %(message)s",
)
logger = logging.getLogger("ArkheJAX")

# ===========================
# 1. PROTOCOLO V3.0
# ===========================

class ProtocolV3:
    """Estrutura de dados para o protocolo V3.0 (RSI via prompt)."""

    # Campos numéricos extraíveis do contrato LLM
    _NUMERIC_FIELDS = ['gamma_B', 'E0', 'omega', 'eta', 'kappa', 'mu', 'xi']

    def __init__(self, config: Dict[str, Any] = None):
        self.config = config or {}
        self.gamma_B = self.config.get('gamma_B', 1.2)
        self.E0 = self.config.get('E0', 0.18)
        self.omega = self.config.get('omega', 2.56e-10)
        self.eta = self.config.get('eta', 0.1)
        self.kappa = self.config.get('kappa', 0.05)
        self.mu = self.config.get('mu', 0.01)
        self.xi = self.config.get('xi', 0.01)
        self.entropy_threshold = self.config.get('entropy_threshold', 1e-3)
        self.phase_tolerance = self.config.get('phase_tolerance', 1e-6)
        self.max_iterations = self.config.get('max_iterations', 100)
        self.toon_chain: List[str] = []

    def to_dict(self) -> Dict[str, Any]:
        return {
            'gamma_B': self.gamma_B, 'E0': self.E0, 'omega': self.omega,
            'eta': self.eta, 'kappa': self.kappa, 'mu': self.mu, 'xi': self.xi,
            'entropy_threshold': self.entropy_threshold,
            'phase_tolerance': self.phase_tolerance,
            'max_iterations': self.max_iterations,
            'toon_chain': self.toon_chain,
        }

    def toon(self, phase: float, entropy: float, alpha: List[float]) -> str:
        # A3: valores convertidos a tipos básicos antes da serialização
        data = {
            'gamma_B': float(phase), 'entropy': float(entropy),
            'alpha': [float(a) for a in alpha],
            'timestamp': time.time(),
            'prev_hash': self.toon_chain[-1] if self.toon_chain else 'GENESIS'
        }
        hash_obj = hashlib.sha256(json.dumps(data, sort_keys=True).encode())
        toon_id = f"TOON:{hash_obj.hexdigest()[:16]}"
        self.toon_chain.append(toon_id)
        return toon_id

# ===========================
# 2. CONTROLADOR LINGUÍSTICO
# ===========================

class LinguisticController:
    """Orquestra o ciclo RSI via linguagem."""
    def __init__(self, protocol: ProtocolV3 = None):
        self.protocol = protocol or ProtocolV3()
        self.history = []
        self.iterations = 0
        self.converged = False
        # A2: janela de variancia para convergência robusta
        self._entropy_window: List[float] = []

    def parse_llm_response(self, response: str) -> Dict[str, Any]:
        """A1: Extrai parâmetros via regex, independente de formato (linha/JSON/Markdown)."""
        params: Dict[str, Any] = {}
        if not response:
            return params

        # Suporta tanto "key=value" quanto "key = value"
        for key in self.protocol._NUMERIC_FIELDS:
            pattern = re.compile(rf"(?:{re.escape(key)})\s*=\s*([-+]?[0-9]*\.?[0-9]+(?:[eE][-+]?[0-9]+)?)")
            match = pattern.search(response)
            if match:
                try:
                    params[key] = float(match.group(1))
                except (ValueError, TypeError):
                    logger.warning(f"Valor inválido para {key}: {match.group(1)!r}")
        return params

    def update_protocol(self, params: Dict[str, Any]) -> None:
        for key, value in params.items():
            if hasattr(self.protocol, key):
                setattr(self.protocol, key, value)

    def _check_convergence(self, entropy: float, phase: float, t: float) -> bool:
        """A2: Convergência baseada em janela de variancia das últimas iterações."""
        # Condição primária (entropia e fase)
        primary = (
            entropy < self.protocol.entropy_threshold
            and abs(phase - self.protocol.gamma_B) < self.protocol.phase_tolerance
            and t > 100.0
        )
        if not primary:
            return False

        # Estabilidade estatística: guardar janela de entropia
        self._entropy_window.append(entropy)
        if len(self._entropy_window) < 5:
            return False

        window = self._entropy_window[-5:]
        variance = float(np.var(window))
        # Convergência confirmada se a variancia for pequena (estabilidade)
        return variance < 1e-4

    def step(self, llm_response: str, co_verify_metrics: Dict[str, Any]) -> Dict[str, Any]:
        try:
            self.iterations += 1
            params = self.parse_llm_response(llm_response)
            self.update_protocol(params)

            C_surface = co_verify_metrics.get('C_surface', 0.0)
            entropy = co_verify_metrics.get('entropy', 1.0)
            phase = co_verify_metrics.get('gamma_B', self.protocol.gamma_B)
            t = co_verify_metrics.get('t', 0.0)

            toon_id = None
            if self._check_convergence(entropy, phase, t):
                toon_id = self.protocol.toon(phase, entropy, [0.1, 0.2, 0.3])
                self.converged = True

            self.history.append({
                'iteration': self.iterations,
                'params': params,
                'metrics': co_verify_metrics,
                'toon': toon_id,
                'converged': self.converged,
            })

            return {
                'iteration': self.iterations,
                'params': params,
                'converged': self.converged,
                'toon': toon_id,
                'entropy': entropy,
                'phase': phase,
                't': t,
            }
        except Exception as e:
            logger.error(f"Erro no passo {self.iterations}: {e}")
            return {
                'iteration': self.iterations,
                'params': {},
                'converged': False,
                'toon': None,
                'entropy': co_verify_metrics.get('entropy', 1.0),
                'phase': co_verify_metrics.get('gamma_B', self.protocol.gamma_B),
                't': co_verify_metrics.get('t', 0.0),
                'error': str(e),
            }

# ===========================
# 3. NÚCLEO ARKHE v5.1
# ===========================

class ValidatedMicrotubuleSurfaceCode:
    def __init__(self, L: int = 13, p_error: float = 0.008, gamma_corr: float = 8.0):
        self.L = L
        self.p_error = p_error
        self.gamma_corr = gamma_corr
        self.C_eq = gamma_corr / (p_error + gamma_corr)

    def evolve(self, rho: float, t: float) -> float:
        rate = self.p_error + self.gamma_corr
        rho_new = self.C_eq + (rho - self.C_eq) * np.exp(np.clip(-rate * t, -50.0, 0.0))
        return np.clip(rho_new, 0.0, 1.0)

class ValidatedExosomeBridge:
    def __init__(self):
        self.exosome_increase_factor = 1.5
        self.ef_intensity = 1.0

    def secretion_rate(self, E: float) -> float:
        E_norm = E / self.ef_intensity
        return 0.5 + 0.5 * (E_norm**2 / (1 + E_norm**2)) * self.exosome_increase_factor

    def coherence_fidelity(self, C_surface: float, E: float) -> float:
        exo_rate = self.secretion_rate(E)
        exo_rate_max = 0.5 + 0.5 * 1.0 * self.exosome_increase_factor
        return C_surface * (exo_rate / exo_rate_max)

# ===========================
# 4. ARKHE-JAX v6.0 INTEGRADO
# ===========================

class ArkheJAXv6:
    def __init__(self, protocol: ProtocolV3 = None, state_file: str = "arkhe_state.json"):
        self.protocol = protocol or ProtocolV3()
        self.controller = LinguisticController(self.protocol)
        self.surface = ValidatedMicrotubuleSurfaceCode(L=13, p_error=0.008, gamma_corr=8.0)
        self.exosome = ValidatedExosomeBridge()
        self.history = []
        self.state_file = state_file
        self.load_state()  # A4: carrega estado persistido

    def run_iteration(self, llm_response: str, t: float) -> Dict[str, Any]:
        rho_initial = 1.0
        rho_evolved = self.surface.evolve(rho_initial, t)
        C_surface = float(rho_evolved)
        E_field = self.protocol.E0
        fidelity = self.exosome.coherence_fidelity(C_surface, E_field)
        entropy = 1.0 - C_surface

        metrics = {
            'C_surface': C_surface,
            'fidelity': fidelity,
            'entropy': entropy,
            'gamma_B': self.protocol.gamma_B,
            't': t,
        }

        try:
            result = self.controller.step(llm_response, metrics)
            self.history.append({
                'iteration': result['iteration'],
                'protocol': self.protocol.to_dict(),
                'metrics': metrics,
                'converged': result['converged'],
                'toon': result['toon'],
            })
            self.save_state()  # A4: salva estado
            return result
        except Exception as e:
            logger.error(f"Erro na iteração {self.controller.iterations}: {e}")
            return {'error': str(e), 'iteration': self.controller.iterations}

    def save_state(self) -> None:
        """A4: persistência do estado e da cadeia TOON."""
        try:
            state = {
                'protocol': self.protocol.to_dict(),
                'iterations': self.controller.iterations,
                'converged': self.controller.converged,
                'history': self.history[-100:],
            }
            with open(self.state_file, 'w', encoding='utf-8') as f:
                json.dump(state, f, indent=2)
            logger.debug(f"Estado salvo em {self.state_file}")
        except Exception as e:
            logger.error(f"Falha ao salvar estado: {e}")

    def load_state(self) -> None:
        """A4: carga do estado persistido."""
        if not os.path.exists(self.state_file):
            return
        try:
            with open(self.state_file, 'r', encoding='utf-8') as f:
                state = json.load(f)
            protocol_dict = state.get('protocol', {})
            for k, v in protocol_dict.items():
                if k == 'toon_chain':
                    self.protocol.toon_chain = list(v)
                elif hasattr(self.protocol, k):
                    setattr(self.protocol, k, v)
            self.controller.iterations = state.get('iterations', 0)
            self.controller.converged = state.get('converged', False)
            self.history = state.get('history', [])
            logger.info(f"Estado anterior carregado de {self.state_file}")
        except Exception as e:
            logger.warning(f"Falha ao carregar estado: {e}. Iniciando do zero.")

    def run_loop(self, t_values: List[float], llm_feedback: List[str]) -> List[Dict[str, Any]]:
        results = []
        # Garantia de alinhamento de tamanho entre t_values e llm_feedback
        for t, feedback in zip(t_values, llm_feedback):
            result = self.run_iteration(feedback, t)
            results.append(result)
            if result.get('converged'):
                logger.info(f"✅ Convergência alcançada na iteração {result['iteration']}")
        return results

# ===========================
# 5. EXECUÇÃO
# ===========================

if __name__ == "__main__":
    protocol = ProtocolV3({
        'gamma_B': 1.2, 'E0': 0.18, 'omega': 2.56e-10,
        'eta': 0.1, 'kappa': 0.05, 'mu': 0.01, 'xi': 0.01,
        'entropy_threshold': 1e-3, 'phase_tolerance': 1e-6,
        'max_iterations': 100,
    })

    arkhe = ArkheJAXv6(protocol)

    llm_responses = [
        "gamma_B = 1.20\nE0 = 0.18\neta = 0.10\nAjuste inicial.",
        "gamma_B = 1.22\nE0 = 0.19\neta = 0.12\nAumentar eta.",
        "gamma_B = 1.21\nE0 = 0.185\neta = 0.11\nFine-tuning.",
        "gamma_B = 1.20\nE0 = 0.18\neta = 0.10\nConvergência.",
        "gamma_B = 1.20\nE0 = 0.18\neta = 0.10\nEstado estável.",
    ]

    t_values = [0.0, 50.0, 100.0, 200.0, 300.0]
    results = arkhe.run_loop(t_values, llm_responses)

    logger.info(f"📜 TOONs: {arkhe.protocol.toon_chain}")
    logger.info(f"📊 Iterações: {arkhe.controller.iterations}")
    logger.info(f"🔧 γ_B={arkhe.protocol.gamma_B:.4f}, E₀={arkhe.protocol.E0:.4f}")
