"""
Logger estruturado do ecossistema Arkhe 8.1.
"""

import json
import logging
import sys
from datetime import datetime, timezone
from typing import Dict, Any, Optional

_LOG_CONFIGURED = False


def _configure_root():
    global _LOG_CONFIGURED
    if _LOG_CONFIGURED:
        return
    handler = logging.StreamHandler(sys.stdout)
    handler.setFormatter(ArkheFormatter())
    root = logging.getLogger()
    root.setLevel(logging.INFO)
    root.handlers = [handler]
    _LOG_CONFIGURED = True


class ArkheFormatter(logging.Formatter):
    """Formatter com timestamp UTC ISO e campos Arkhe."""

    def format(self, record: logging.LogRecord) -> str:
        ts = datetime.now(timezone.utc).isoformat()
        return f"[{ts}] {record.levelname:7s} {record.name}: {record.getMessage()}"


def get_logger(name: str) -> logging.Logger:
    """Retorna um logger nomeado configurado."""
    _configure_root()
    logger = logging.Logger(name)
    logger.handlers = []
    logger.addHandler(logging.StreamHandler(sys.stdout))
    logger.setLevel(logging.INFO)
    return logger


def log_structured(name: str, event: str, **fields: Any) -> None:
    """Emite um log estruturado em JSON."""
    payload: Dict[str, Any] = {
        "timestamp": datetime.now(timezone.utc).isoformat(),
        "logger": name,
        "event": event,
        **fields,
    }
    print(json.dumps(payload, default=str), file=sys.stdout, flush=True)