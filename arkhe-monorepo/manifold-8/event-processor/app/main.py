"""
Event Processor — serviço FastAPI do pipeline de dados Arkhe 8.1.

Endpoints:
  GET  /health               → health check
  POST /verbal/analyze       → análise verbal química
  POST /events               → ingere evento bruto (processa engrama)
  GET  /metrics              → métricas Prometheus
  GET  /status               → status do ArkheBridge
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from typing import Any, Dict, Optional

from fastapi import FastAPI, HTTPException
from pydantic import BaseModel, Field

from src.config import settings
from src.logger import get_logger
from src.metrics import metrics_manager
from src.arkhe_bridge import ArkheBridge, ArkhePriority
from core.verbal_chemistry import VerbalStatement
from verbal_events_processor import VerbalEventProcessor
from src.models import Event

logger = get_logger("event_processor")

app = FastAPI(title="Arkhe Event Processor 8.1", version="8.1")

_bridge = ArkheBridge()
_processor = VerbalEventProcessor()


class AnalyzeRequest(BaseModel):
    text: str = Field(min_length=1)


class EventRequest(BaseModel):
    source: str = "client"
    event_type: str = "generic"
    payload: Dict[str, Any] = Field(default_factory=dict)


@app.get("/health")
def health() -> Dict[str, Any]:
    return {"status": "ok", "service": "event-processor", "version": "8.1"}


@app.post("/verbal/analyze")
def verbal_analyze(req: AnalyzeRequest) -> Dict[str, Any]:
    try:
        statement = VerbalStatement.from_text(req.text)
        result = statement.to_dict()
        result["biochemical_impact"] = statement.biochemical_impact
        metrics_manager.record_event_processed(
            event_type="verbal_analyze", status="success", duration=0.0
        )
        return result
    except Exception as e:
        logger.error(f"❌ Falha ao analisar texto: {e}")
        metrics_manager.record_error("verbal_analyze", "analysis")
        raise HTTPException(status_code=500, detail=str(e))


@app.post("/events")
def ingest_event(req: EventRequest) -> Dict[str, Any]:
    event = Event(
        source=req.source,
        event_type=req.event_type,
        payload=req.payload,
    )
    processed = _processor.process_event(event)
    metrics_manager.record_event_processed(
        event_type="event_ingest", status="success", duration=0.0
    )
    return processed.to_dict()


@app.get("/metrics")
def prometheus_metrics() -> str:
    return metrics_manager.prometheus_text()


@app.get("/status")
def system_status() -> Dict[str, Any]:
    return _bridge.get_status()