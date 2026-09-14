"""
Verbal Chemistry — biologia quântica das palavras.

Traduz autofalas (declarações verbais) em impacto bioquímico estimado,
com base em heurísticas lexicais e estados de Schmidt normalizados.

Substrato: 227-F (alineação ética), conceitos de coerência verbal.
"""

import re
from dataclasses import dataclass, field
from enum import Enum
from typing import Dict, List, Optional, Tuple


class Polarity(Enum):
    """Polaridade da declaração verbal (espectro de coerência)."""

    COHERENT = "coherent"       # fala alinhada com a identidade do arquiteto
    CONSTRUCTIVE = "constructive"
    NEUTRAL = "neutral"
    DISRUPTIVE = "disruptive"
    TOXIC = "toxic"             # fala corrosiva à própria arquitetura


# Léxico interno: palavras com carga emocional (PT/EN).
_POSITIVE_WORDS = {
    "healing", "strong", "capable", "grateful", "love", "peace", "alive",
    "free", "bright", "calm", "confident", "safe", "whole", "joy", "light",
    "sou", "forte", "capaz", "grato", "amo", "paz", "vivo", "livre",
    "brilhante", "calmo", "confiante", "seguro", "inteiro", "alegria", "luz",
}
_NEGATIVE_WORDS = {
    "fear", "angry", "afraid", "hopeless", "stuck", "worthless", "broken",
    "anxious", "pain", "sick", "weak", "hate", "dark", "medo", "raiva",
    "preso", "sem valor", "quebrado", "ansioso", "dor", "doente", "fraco",
    "odio", "escuro",
}


@dataclass
class VerbalStatement:
    """
    Declaração verbal analisada.

    A `emotional_charge` é normalizada em [-1, 1] e reflete o vetor
    de estado de Schmidt projetado sobre o eixo polaridade.
    """

    text: str
    polarity: Polarity = Polarity.NEUTRAL
    emotional_charge: float = 0.0
    biochemical_impact: Dict[str, float] = field(default_factory=dict)
    tokens: List[str] = field(default_factory=list)

    # ------------------------------------------------------------------
    # Construção
    # ------------------------------------------------------------------

    @classmethod
    def from_text(cls, text: str) -> "VerbalStatement":
        tokens = _tokenize(text)
        pos = sum(1 for t in tokens if t in _POSITIVE_WORDS)
        neg = sum(1 for t in tokens if t in _NEGATIVE_WORDS)
        total = pos + neg
        charge = (pos - neg) / (total if total else 1.0)
        charge = max(-1.0, min(1.0, charge))

        polarity = _classify(charge)
        impact = _biochemical_impact(charge)

        return cls(
            text=text,
            polarity=polarity,
            emotional_charge=charge,
            biochemical_impact=impact,
            tokens=tokens,
        )

    # ------------------------------------------------------------------
    # Utilidades
    # ------------------------------------------------------------------

    def to_dict(self) -> dict:
        return {
            "text": self.text,
            "polarity": self.polarity.name,
            "emotional_charge": self.emotional_charge,
            "biochemical_impact": self.biochemical_impact,
            "tokens": self.tokens,
        }


def _tokenize(text: str) -> List[str]:
    """Tokeniza minúsculas, preservando frases de múltiplas palavras do léxico."""
    lowered = text.lower()
    tokens = re.findall(r"[a-zà-ÿç]{2,}", lowered, flags=re.IGNORECASE)
    return [t.lower() for t in tokens]


def _classify(charge: float) -> Polarity:
    if charge >= 0.6:
        return Polarity.COHERENT
    if charge >= 0.25:
        return Polarity.CONSTRUCTIVE
    if charge > -0.25:
        return Polarity.NEUTRAL
    if charge > -0.6:
        return Polarity.DISRUPTIVE
    return Polarity.TOXIC


def _biochemical_impact(charge: float) -> Dict[str, float]:
    """
    Impacto neuroquímico estimado (variação relativa, sem pretensão clínica).

    Fala coerente: ↑ oxitocina, ↑ serotonina, ↑ dopamina, ↓ cortisol.
    Fala disruptiva: direção inversa.
    """
    oxytocin = 0.15 * charge
    serotonin = 0.12 * charge
    dopamine = 0.10 * charge
    cortisol = -0.18 * charge
    adrenaline = -0.08 * charge
    return {
        "oxytocin": round(oxytocin, 4),
        "serotonin": round(serotonin, 4),
        "dopamine": round(dopamine, 4),
        "cortisol": round(cortisol, 4),
        "adrenaline": round(adrenaline, 4),
    }