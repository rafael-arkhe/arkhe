"""
Verbal Events Processor — transforma declarações verbais em eventos do pipeline.

Conector entre `core.verbal_chemistry.VerbalStatement` e `src.models.Event`.
"""

from typing import Dict, Any
from datetime import datetime, timezone

from src.models import Event, ProcessedEvent
from core.verbal_chemistry import VerbalStatement, Polarity


class VerbalEventProcessor:
    """Processa declarações verbais e produz eventos para o pipeline."""

    def __init__(self) -> None:
        self.processed_count = 0

    def verbal_statement_to_event(self, statement: VerbalStatement) -> Event:
        """Converte uma VerbalStatement em um evento bruto do pipeline."""
        return Event(
            source="verbal_chemistry",
            event_type="verbal_statement",
            payload={
                "text": statement.text,
                "polarity": statement.polarity.name,
                "emotional_charge": statement.emotional_charge,
                "biochemical_impact": statement.biochemical_impact,
            },
            timestamp=datetime.now(timezone.utc),
        )

    def process_event(self, event: Event) -> ProcessedEvent:
        """Processa um evento verbal em um evento processado (pronto para engrama)."""
        self.processed_count += 1
        payload = event.payload
        return ProcessedEvent(
            event_id=event.event_id,
            source=event.source,
            status="processed",
            polarity=payload.get("polarity"),
            emotional_charge=payload.get("emotional_charge"),
            biochemical_impact=payload.get("biochemical_impact"),
            processed_at=datetime.now(timezone.utc),
        )

    def analyze_text(self, text: str) -> Dict[str, Any]:
        """Atalho: analisa texto e retorna dict pronto para resposta HTTP."""
        statement = VerbalStatement.from_text(text)
        return statement.to_dict()