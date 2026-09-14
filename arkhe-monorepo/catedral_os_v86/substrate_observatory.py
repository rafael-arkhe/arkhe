#!/usr/bin/env python3
"""
Substrato — Observatório Ontológico (Auto-Observação e Anomalias)

Sistema de auto-observação para o Catedral OS AGI. Registra métricas,
detecta anomalias estatísticas, e gera insights periódicos.

NOTA DE HONESTIDADE (auditoria):
 - A detecção de anomalias usa z-score (desvio padrão), que pressupõe
   distribuição aproximadamente normal. Para distribuições pesudas-cauda,
   usar median absolute deviation (MAD) — extensível futuramente.
 - O "gerador de insights" é baseado em templates de string, não um LLM.
   Para insights semânticos, integrar com um modelo de linguagem.
 - O Observatório NÃO toma decisões autônomas — apenas observa e reporta.
   O Catedral OS AGI é responsável por ações baseadas nos insights.

Selo: CATEDRAL-OS-v86.0-SUBSTRATE-OBSERVATORY
"""

import time
import logging
import math
from typing import Any, Dict, List, Optional
from collections import deque
from dataclasses import dataclass, field

logger = logging.getLogger("substrate.observatory")


@dataclass
class Observation:
    """Uma observação individual."""
    timestamp: float
    metric: str
    value: Any
    context: Dict = field(default_factory=dict)


@dataclass
class Insight:
    """Um insight gerado pelo observatório."""
    timestamp: float
    content: str
    confidence: float
    source_metrics: List[str] = field(default_factory=list)


class Observatory:
    """
    Observatório Ontológico: auto-observação, detecção de anomalias,
    e geração de insights.
    """

    def __init__(self, window_size: int = 1000):
        self.window_size = window_size
        self.observations: deque = deque(maxlen=window_size)
        self.metrics: Dict[str, deque] = {}
        self.insights: List[Insight] = []

    def record(self, metric: str, value: Any,
               context: Optional[Dict] = None) -> None:
        """Registra uma observação."""
        if metric not in self.metrics:
            self.metrics[metric] = deque(maxlen=self.window_size)
        self.metrics[metric].append(value)
        self.observations.append(
            Observation(time.time(), metric, value, context or {})
        )

    def get_statistics(self, metric: str) -> Dict:
        """Estatísticas descritivas de uma métrica."""
        vals = list(self.metrics.get(metric, []))
        if not vals:
            return {'count': 0}

        numeric = []
        for v in vals:
            try:
                numeric.append(float(v))
            except (TypeError, ValueError):
                continue

        if not numeric:
            return {'count': len(vals)}

        n = len(numeric)
        mean = sum(numeric) / n
        variance = sum((x - mean) ** 2 for x in numeric) / max(n - 1, 1)
        std = math.sqrt(variance)

        return {
            'count': n,
            'mean': mean,
            'std': std,
            'min': min(numeric),
            'max': max(numeric),
            'range': max(numeric) - min(numeric),
            'recent': numeric[-1],
        }

    def detect_anomalies(self, metric: str,
                          threshold: float = 2.0,
                          min_samples: int = 10) -> List[Dict]:
        """
        Detecta anomalias usando z-score.
        Retorna valores que excedem threshold desvios padrão da média.
        """
        stats = self.get_statistics(metric)
        if stats.get('count', 0) < min_samples or stats.get('std', 0) < 1e-12:
            return []

        vals = list(self.metrics.get(metric, []))
        numeric = []
        for v in vals:
            try:
                numeric.append(float(v))
            except (TypeError, ValueError):
                continue

        mean, std = stats['mean'], stats['std']
        anomalies = []
        for v in numeric[-20:]:
            z = abs(v - mean) / std
            if z > threshold:
                anomalies.append({
                    'value': v,
                    'z_score': z,
                    'mean': mean,
                    'std': std,
                })
        return anomalies

    def detect_trend(self, metric: str, window: int = 50) -> Optional[str]:
        """
        Detecta tendências simples: crescente, decrescente, ou estável.
        Retorna string descritiva ou None se insuficientes dados.
        """
        vals = list(self.metrics.get(metric, []))
        if len(vals) < window:
            return None

        recent = vals[-window:]
        numeric = []
        for v in recent:
            try:
                numeric.append(float(v))
            except (TypeError, ValueError):
                continue

        if len(numeric) < window // 2:
            return None

        # Regressão linear simples
        n = len(numeric)
        x_mean = (n - 1) / 2.0
        y_mean = sum(numeric) / n
        num = sum((i - x_mean) * (y - y_mean) for i, y in enumerate(numeric))
        den = sum((i - x_mean) ** 2 for i in range(n))

        if den < 1e-12:
            return "estável"

        slope = num / den
        if abs(slope) < 1e-6:
            return "estável"
        elif slope > 0:
            return f"crescente (slope={slope:.6f})"
        else:
            return f"decrescente (slope={slope:.6f})"

    def generate_insight(self, content: str, confidence: float = 0.5,
                          source_metrics: Optional[List[str]] = None) -> Insight:
        """Gera um insight com confiança e métricas fonte."""
        insight = Insight(
            timestamp=time.time(),
            content=content,
            confidence=confidence,
            source_metrics=source_metrics or [],
        )
        self.insights.append(insight)
        return insight

    def get_recent_insights(self, n: int = 10) -> List[Dict]:
        """Retorna os últimos n insights."""
        return [{
            'content': i.content,
            'confidence': i.confidence,
            'source_metrics': i.source_metrics,
            'timestamp': i.timestamp,
        } for i in self.insights[-n:]]

    def get_dashboard(self) -> Dict:
        """Retorna um dashboard completo de todas as métricas."""
        return {
            'total_observations': len(self.observations),
            'total_insights': len(self.insights),
            'metrics': {k: self.get_statistics(k) for k in self.metrics},
            'anomalies': {
                k: self.detect_anomalies(k)
                for k in self.metrics
            },
            'trends': {
                k: self.detect_trend(k)
                for k in self.metrics
            },
        }
