"""
Modelos de dados do pipeline de eventos Arkhe 8.1.
"""

from typing import Any, Dict, Optional
from datetime import datetime, timezone
from pydantic import BaseModel, Field


class Event(BaseModel):
    """Evento bruto proveniente do pipeline de dados."""

    event_id: str = Field(default_factory=lambda: f"evt-{int(datetime.now(timezone.utc).timestamp() * 1000)}")
    source: str = "unknown"
    event_type: str = "generic"
    payload: Dict[str, Any] = Field(default_factory=dict)
    timestamp: datetime = Field(default_factory=lambda: datetime.now(timezone.utc))

    def to_dict(self) -> Dict[str, Any]:
        return self.model_dump(mode="json")


class ProcessedEvent(BaseModel):
    """Evento processado, pronto para arquivamento em Glass5D."""

    event_id: str
    source: str
    status: str = "processed"
    polarity: Optional[str] = None
    emotional_charge: Optional[float] = None
    biochemical_impact: Optional[Dict[str, float]] = None
    topological_winding: Optional[float] = None
    validation: Optional[str] = None
    roquo_job_id: Optional[str] = None
    glass5d_record_id: Optional[str] = None
    processed_at: datetime = Field(default_factory=lambda: datetime.now(timezone.utc))

    def to_dict(self) -> Dict[str, Any]:
        return self.model_dump(mode="json")


class EngramRecord(BaseModel):
    """Registro de um engrama arquivado no Glass5D."""

    record_id: str
    dataset_name: str
    created_at: datetime
    bytes_written: int
    metadata: Dict[str, Any] = Field(default_factory=dict)