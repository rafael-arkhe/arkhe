# =============================================================================
# BLOCO 470 v13 — MONITORAMENTO DE TRUST COM ALERTAS (O5)
# trust_monitor_v13.py
#
# Monitor de trust scores com:
#   - Thresholds configuráveis (warning/critical/recovery)
#   - Deduplicação com cooldown
#   - Canais de alerta: console, email, webhook
#   - Estados acknowledged/resolved
#
# Requer: pip install requests
# =============================================================================
import json
import time
import threading
import smtplib
import requests
from datetime import datetime, timedelta
from dataclasses import dataclass, field
from typing import Dict, List, Optional, Callable, Any
from enum import Enum
import hashlib


class AlertSeverity(Enum):
    INFO = "info"
    WARNING = "warning"
    CRITICAL = "critical"


class AlertChannel(Enum):
    CONSOLE = "console"
    EMAIL = "email"
    WEBHOOK = "webhook"
    SLACK = "slack"
    PAGERDUTY = "pagerduty"


@dataclass
class Alert:
    """Alerta de monitoramento de trust."""
    id: str
    severity: AlertSeverity
    peer: str
    message: str
    trust_score: float
    threshold: float
    timestamp: datetime
    channel: AlertChannel
    acknowledged: bool = False
    resolved: bool = False
    resolved_at: Optional[datetime] = None


@dataclass
class TrustThresholds:
    """Thresholds para alertas de trust."""
    warning_threshold: float = 0.5   # Score < 0.5  -> Warning
    critical_threshold: float = 0.3  # Score < 0.3  -> Critical
    recovery_threshold: float = 0.6  # Score > 0.6  -> Resolved
    cooldown_seconds: int = 3600     # Intervalo mínimo entre alertas por peer


class TrustMonitor:
    """
    Monitor de trust scores com alertas automáticos.
    Baseado no modelo CAEP do MolTrust.
    """

    def __init__(self, federation_manager: Any):
        self.federation = federation_manager
        self.thresholds = TrustThresholds()
        self.alerts: Dict[str, Alert] = {}
        self.alert_history: List[Alert] = []
        self._handlers: Dict[AlertChannel, List[Callable]] = {
            channel: [] for channel in AlertChannel
        }
        self._running = False
        self._monitor_thread = None

    def register_handler(self, channel: AlertChannel, handler: Callable) -> None:
        """Registra um handler para um canal de alerta."""
        self._handlers[channel].append(handler)

    def start(self, interval: int = 30) -> None:
        """Inicia o monitoramento periódico."""
        if self._running:
            return
        self._running = True
        self._monitor_thread = threading.Thread(
            target=self._monitor_loop,
            args=(interval,),
            daemon=True,
        )
        self._monitor_thread.start()
        print(f"[TrustMonitor] Started (interval: {interval}s)")

    def stop(self) -> None:
        """Para o monitoramento."""
        self._running = False
        if self._monitor_thread:
            self._monitor_thread.join(timeout=5)

    def _monitor_loop(self, interval: int) -> None:
        """Loop principal de monitoramento."""
        while self._running:
            try:
                self._check_peers()
            except Exception as e:
                print(f"[TrustMonitor] Error: {e}")
            time.sleep(interval)

    def _check_peers(self) -> None:
        """Verifica todos os peers e gera alertas."""
        for name, peer in list(self.federation._peers.items()):
            self._check_peer(name, peer)

    def _check_peer(self, name: str, peer: Any) -> None:
        """Verifica um peer específico."""
        score = getattr(peer, "trust_score", 0.0)
        existing_alert = self._get_active_alert(name)

        if score < self.thresholds.critical_threshold:
            self._create_alert(
                peer=name,
                severity=AlertSeverity.CRITICAL,
                message=f"Trust score crítico: {score:.2f} (abaixo de {self.thresholds.critical_threshold})",
                trust_score=score,
                threshold=self.thresholds.critical_threshold,
            )
        elif score < self.thresholds.warning_threshold:
            self._create_alert(
                peer=name,
                severity=AlertSeverity.WARNING,
                message=f"Trust score baixo: {score:.2f} (abaixo de {self.thresholds.warning_threshold})",
                trust_score=score,
                threshold=self.thresholds.warning_threshold,
            )
        elif existing_alert and score >= self.thresholds.recovery_threshold:
            # Resolve o alerta
            existing_alert.resolved = True
            existing_alert.resolved_at = datetime.utcnow()
            self._send_alert(
                existing_alert,
                f"✅ Trust score recuperado: {score:.2f} (acima de {self.thresholds.recovery_threshold})",
            )

    def _create_alert(
        self,
        peer: str,
        severity: AlertSeverity,
        message: str,
        trust_score: float,
        threshold: float,
    ) -> Optional[Alert]:
        """Cria um novo alerta com deduplicação + cooldown."""
        existing = self._get_active_alert(peer)
        if existing:
            age = (datetime.utcnow() - existing.timestamp).total_seconds()
            if age < self.thresholds.cooldown_seconds:
                return None
            # Atualiza o alerta existente
            existing.severity = severity
            existing.message = message
            existing.trust_score = trust_score
            existing.threshold = threshold
            existing.timestamp = datetime.utcnow()
            alert = existing
        else:
            alert_id = hashlib.sha256(
                f"{peer}:{datetime.utcnow().isoformat()}".encode()
            ).hexdigest()[:8]
            alert = Alert(
                id=alert_id,
                severity=severity,
                peer=peer,
                message=message,
                trust_score=trust_score,
                threshold=threshold,
                timestamp=datetime.utcnow(),
                channel=AlertChannel.CONSOLE,
            )
            self.alerts[alert_id] = alert
            self.alert_history.append(alert)

        self._send_alert(alert, message)
        return alert

    def _get_active_alert(self, peer: str) -> Optional[Alert]:
        """Obtém um alerta ativo para um peer."""
        for alert in self.alerts.values():
            if alert.peer == peer and not alert.resolved:
                return alert
        return None

    def _send_alert(self, alert: Alert, message: str) -> None:
        """Envia um alerta através dos canais configurados."""
        if alert.severity == AlertSeverity.CRITICAL:
            channels = [AlertChannel.PAGERDUTY, AlertChannel.SLACK, AlertChannel.EMAIL, AlertChannel.CONSOLE]
        elif alert.severity == AlertSeverity.WARNING:
            channels = [AlertChannel.SLACK, AlertChannel.EMAIL, AlertChannel.CONSOLE]
        else:
            channels = [AlertChannel.CONSOLE]

        for channel in channels:
            for handler in self._handlers.get(channel, []):
                try:
                    handler(alert, message)
                except Exception as e:
                    print(f"[TrustMonitor] Handler error for {channel}: {e}")

    def get_alerts(self, resolved: bool = False) -> List[Alert]:
        """Obtém alertas ativos ou resolvidos."""
        if resolved:
            return [a for a in self.alerts.values() if a.resolved]
        return [a for a in self.alerts.values() if not a.resolved]

    def acknowledge_alert(self, alert_id: str) -> bool:
        """Marca um alerta como reconhecido."""
        if alert_id in self.alerts:
            self.alerts[alert_id].acknowledged = True
            return True
        return False

    def get_stats(self) -> Dict:
        """Obtém estatísticas do monitoramento."""
        active = [a for a in self.alerts.values() if not a.resolved]
        critical = [a for a in active if a.severity == AlertSeverity.CRITICAL]
        warning = [a for a in active if a.severity == AlertSeverity.WARNING]

        return {
            "total_alerts": len(self.alert_history),
            "active_alerts": len(active),
            "critical": len(critical),
            "warning": len(warning),
            "thresholds": {
                "warning": self.thresholds.warning_threshold,
                "critical": self.thresholds.critical_threshold,
                "recovery": self.thresholds.recovery_threshold,
            },
        }


# ============================================================================
# HANDLERS PARA CANAIS DE ALERTA
# ============================================================================

class AlertHandlers:
    """Handlers para diferentes canais de alerta."""

    @staticmethod
    def console_handler(alert: Alert, message: str) -> None:
        """Handler para console."""
        emoji = {
            AlertSeverity.CRITICAL: "🚨",
            AlertSeverity.WARNING: "⚠️",
            AlertSeverity.INFO: "ℹ️",
        }
        print(f"{emoji.get(alert.severity, '📢')} [{alert.severity.value.upper()}] {alert.peer}: {message}")

    @staticmethod
    def email_handler(smtp_config: Dict) -> Callable:
        """Handler para email."""
        def handler(alert: Alert, message: str) -> None:
            try:
                with smtplib.SMTP(smtp_config["host"], smtp_config.get("port", 587)) as server:
                    server.starttls()
                    server.login(smtp_config["user"], smtp_config["password"])

                    subject = f"[Catedral OS] Trust Alert: {alert.peer} - {alert.severity.value}"
                    body = f"""
Peer: {alert.peer}
Severity: {alert.severity.value}
Trust Score: {alert.trust_score:.2f}
Threshold: {alert.threshold:.2f}
Message: {message}
Timestamp: {alert.timestamp.isoformat()}
"""
                    msg = f"Subject: {subject}\n\n{body}"
                    server.sendmail(smtp_config["from"], smtp_config["to"], msg)
            except Exception as e:
                print(f"[Email] Failed to send: {e}")

        return handler

    @staticmethod
    def webhook_handler(url: str) -> Callable:
        """Handler para webhook (Slack-compatible)."""
        def handler(alert: Alert, message: str) -> None:
            try:
                data = {
                    "alert_id": alert.id,
                    "severity": alert.severity.value,
                    "peer": alert.peer,
                    "trust_score": alert.trust_score,
                    "threshold": alert.threshold,
                    "message": message,
                    "timestamp": alert.timestamp.isoformat(),
                }
                requests.post(url, json=data, timeout=10)
            except Exception as e:
                print(f"[Webhook] Failed to send: {e}")

        return handler


# ============================================================================
# EXEMPLO DE USO
# ============================================================================
class _StubPeer:
    """Peers mínimo para demo sem depender do federation_v12."""
    def __init__(self, name: str, trust_score: float = 0.5, active: bool = True):
        self.name = name
        self.trust_score = trust_score
        self.active = active


class _StubFederation:
    def __init__(self):
        self._peers: Dict[str, _StubPeer] = {
            "Catedral-PR": _StubPeer("Catedral-PR", 0.92),
            "Catedral-SP": _StubPeer("Catedral-SP", 0.78),
            "Catedral-RJ": _StubPeer("Catedral-RJ", 0.63),
            "Catedral-DF": _StubPeer("Catedral-DF", 0.25),
        }


if __name__ == "__main__":
    fed = _StubFederation()
    monitor = TrustMonitor(fed)
    monitor.register_handler(AlertChannel.CONSOLE, AlertHandlers.console_handler)
    monitor._check_peers()
    print("Stats:", monitor.get_stats())