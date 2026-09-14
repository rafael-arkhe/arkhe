"""SOPHIA V5.0 — núcleo auto-hospedado.

Módulos:
- sophia_local.patches: patches #19–22 (ψ_C, Q, cérebro global, cavidade QED)
- sophia_local.orchestrator: orquestrador de agentes locais
- sophia_local.temporal_bridge: ancoragem local na TemporalChain (append-only, SHA3)
- sophia_local.linguistic_agent: evolução Fenício→Grego
- sophia_local.security: hardening, integridade, firewall
"""

__version__ = "5.0.0"
__all__ = ["patches", "orchestrator", "temporal_bridge", "linguistic_agent"]