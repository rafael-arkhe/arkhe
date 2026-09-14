# quantum/__init__.py
"""Modulo quantico da Catedral OS v294.0 — NHSE Engine e Decisor CQFI."""

from quantum.nhse_engine import NHSEngine
from quantum.nhse_decider import NHSE_DQN, NHSEDecider

__all__ = ["NHSEngine", "NHSE_DQN", "NHSEDecider"]