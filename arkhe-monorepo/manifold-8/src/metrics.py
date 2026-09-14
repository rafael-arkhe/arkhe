"""
Gerenciador de métricas do ecossistema Arkhe 8.1.

Registra operações de banco, eventos processados e erros.
Em modo container as métricas são expostas em formato Prometheus.
"""

import threading
import time
from collections import defaultdict
from datetime import datetime, timezone
from typing import Dict, Any, List


class MetricsManager:
    """Coletor de métricas em memória com API compatível com a spec 8.1."""

    def __init__(self) -> None:
        self._lock = threading.Lock()
        self._db_operations: List[Dict[str, Any]] = []
        self._events_processed: Dict[str, int] = defaultdict(int)
        self._errors: Dict[str, int] = defaultdict(int)
        self._counters: Dict[str, float] = defaultdict(float)
        self._started_at = datetime.now(timezone.utc).isoformat()

    # ------------------------------------------------------------------
    # API interna de registro
    # ------------------------------------------------------------------

    def record_database_operation(self, operation: str, duration: float) -> None:
        with self._lock:
            self._db_operations.append({
                "operation": operation,
                "duration_s": float(duration),
                "timestamp": datetime.now(timezone.utc).isoformat(),
            })
            self._counters[f"db_operation_{operation}"] += 1

    def record_event_processed(self, event_type: str, status: str, duration: float) -> None:
        with self._lock:
            key = f"{event_type}:{status}"
            self._events_processed[key] += 1
            self._counters[f"event_{key}"] += 1

    def record_error(self, error_type: str, category: str) -> None:
        with self._lock:
            self._errors[f"{category}:{error_type}"] += 1
            self._counters[f"errors_total"] += 1

    def increment(self, metric: str, value: float = 1.0) -> None:
        with self._lock:
            self._counters[metric] += value

    # ------------------------------------------------------------------
    # Exportação
    # ------------------------------------------------------------------

    def snapshot(self) -> Dict[str, Any]:
        with self._lock:
            return {
                "started_at": self._started_at,
                "db_operations": list(self._db_operations[-100:]),
                "db_operations_total": len(self._db_operations),
                "events_processed": dict(self._events_processed),
                "errors": dict(self._errors),
                "counters": dict(self._counters),
            }

    def prometheus_text(self) -> str:
        """Exporta métricas no formato text do Prometheus."""
        lines: List[str] = [
            "# HELP arkhe_events_total Eventos processados pelo pipeline.",
            "# TYPE arkhe_events_total counter",
        ]
        for key, value in sorted(self._events_processed.items()):
            lines.append(f'arkhe_events_total{{event="{key}"}} {value}')
        lines.append("# HELP arkhe_errors_total Erros registrados.")
        lines.append("# TYPE arkhe_errors_total counter")
        for key, value in sorted(self._errors.items()):
            lines.append(f'arkhe_errors_total{{error="{key}"}} {value}')
        lines.append("# HELP arkhe_db_operations_total Operações de banco/armazenamento.")
        lines.append("# TYPE arkhe_db_operations_total counter")
        for entry in self._db_operations:
            lines.append(
                f'arkhe_db_operations_total{{operation="{entry["operation"]}"}} 1'
            )
        return "\n".join(lines)


metrics_manager = MetricsManager()