"""monitoring.py — v77 VETADO — BLOCO 517 (G2)

Monitoramento Prometheus para a Catedral OS.

VETAGEM:
  - prometheus_client disponivel neste venv (verificado). O selftest usa um
    CollectorRegistry proprio (sem levantar HTTP) para ser hermetico.
  - start_monitoring(port) permanece utilitario, mas nao roda no selftest.
"""

from __future__ import annotations

from typing import Optional

from prometheus_client import (
    CollectorRegistry,
    Counter,
    Gauge,
    Histogram,
    generate_latest,
)


class CatedralMetrics:
    def __init__(self, registry: Optional[CollectorRegistry] = None) -> None:
        self.reg = registry or CollectorRegistry()
        self.phi_c = Gauge(
            "catedral_phi_c", "Coerencia atual phi_c", registry=self.reg
        )
        self.handovers = Counter(
            "catedral_handovers_total", "Total de handovers", registry=self.reg
        )
        self.aqec = Counter(
            "catedral_aqec_total", "Total de correcoes AQEC", registry=self.reg
        )
        self.recovery_success = Counter(
            "catedral_recovery_success", "Recuperacoes bem-sucedidas", registry=self.reg
        )
        self.recovery_fail = Counter(
            "catedral_recovery_fail", "Recuperacoes falhas", registry=self.reg
        )
        self.latency = Histogram(
            "catedral_aqec_latency_seconds", "Latencia do AQEC", registry=self.reg
        )

    def record_phi_c(self, value: float) -> None:
        self.phi_c.set(max(0.0, min(1.0, value)))

    def record_handover(self) -> None:
        self.handovers.inc()

    def record_recovery(self, success: bool) -> None:
        (self.recovery_success if success else self.recovery_fail).inc()

    def observe_latency(self, seconds: float) -> None:
        self.latency.observe(seconds)


def start_monitoring(port: int = 8000, metrics: Optional[CatedralMetrics] = None) -> CatedralMetrics:
    """Sobe o endpoint /metrics. NAO invocada em selftests (hermetico)."""
    from prometheus_client import start_http_server

    m = metrics or CatedralMetrics()
    start_http_server(port, registry=m.reg)
    return m


if __name__ == "__main__":
    import sys

    fail = 0
    m = CatedralMetrics()
    m.record_phi_c(0.970)
    m.record_handover()
    m.record_handover()
    m.record_recovery(True)
    m.record_recovery(False)
    m.observe_latency(0.05)

    def sample_gauge(metric: Gauge) -> float:
        return float(metric.collect()[0].samples[0].value)

    def sample_counter(metric: Counter) -> float:
        return float(metric.collect()[0].samples[0].value)

    if abs(sample_gauge(m.phi_c) - 0.970) > 1e-9:
        fail += 1
        print("[MET] FAIL: gauge phi_c")
    if sample_counter(m.handovers) != 2.0:
        fail += 1
        print("[MET] FAIL: contador handovers")
    if sample_counter(m.recovery_success) != 1.0 or sample_counter(m.recovery_fail) != 1.0:
        fail += 1
        print("[MET] FAIL: contadores recovery")
    else:
        print(f"[MET] gauge phi_c={sample_gauge(m.phi_c)}  handovers={sample_counter(m.handovers)}")

    text = generate_latest(m.reg).decode()
    if "catedral_phi_c" not in text:
        fail += 1
        print("[MET] FAIL: exposicao prometheus")
    else:
        print("[MET] exposicao /metrics ok (amostra sem HTTP)")

    print(f"[MET] {'ALL CHECKS PASS' if fail == 0 else 'CHECKS FAILED'}")
    sys.exit(1 if fail else 0)