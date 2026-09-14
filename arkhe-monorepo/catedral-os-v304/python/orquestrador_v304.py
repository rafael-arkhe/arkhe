#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Catedral OS v304.0 — Orquestrador com systemd Type=notify e SIGHUP reload
"""

import asyncio
import signal
import sys
import time
import json
import logging
from pathlib import Path
from typing import Dict, List, Optional, Any, Callable, Awaitable
from dataclasses import dataclass, field
from enum import Enum

try:
    import yaml
except ImportError:
    yaml = None

try:
    from prometheus_client import start_http_server, Gauge, Counter, Histogram
    PROMETHEUS_AVAILABLE = True
except ImportError:
    PROMETHEUS_AVAILABLE = False

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [%(levelname)s] %(message)s",
    handlers=[
        logging.StreamHandler(),
    ],
)
logger = logging.getLogger(__name__)

try:
    Path("/var/log/catedral").mkdir(parents=True, exist_ok=True)
    fh = logging.FileHandler("/var/log/catedral/orchestrator.log")
    fh.setLevel(logging.INFO)
    fh.setFormatter(logging.Formatter("%(asctime)s [%(levelname)s] %(message)s"))
    logger.addHandler(fh)
except PermissionError:
    pass


class SystemdNotifier:
    def __init__(self):
        self._socket = None
        self._connected = False
        self._init()

    def _init(self):
        import socket
        import os
        socket_path = os.environ.get("NOTIFY_SOCKET")
        if socket_path:
            try:
                self._socket = socket.socket(socket.AF_UNIX, socket.SOCK_DGRAM)
                self._socket.connect(socket_path)
                self._connected = True
            except Exception as e:
                logger.warning(f"systemd notify unavailable: {e}")
                self._connected = False

    def notify(self, status: str):
        if self._connected:
            try:
                self._socket.send(f"STATUS={status}\n".encode())
            except Exception:
                pass

    def ready(self):
        if self._connected:
            try:
                self._socket.send(b"READY=1\n")
            except Exception:
                pass

    def stopping(self):
        if self._connected:
            try:
                self._socket.send(b"STOPPING=1\n")
            except Exception:
                pass

    def watchdog(self):
        if self._connected:
            try:
                self._socket.send(b"WATCHDOG=1\n")
            except Exception:
                pass


if PROMETHEUS_AVAILABLE:
    PHI_GAUGE = Gauge("catedral_phi", "Current coherence Phi")
    HANDOVER_COUNTER = Counter("catedral_handovers_total", "Total handovers")
    LATENCY_HISTOGRAM = Histogram(
        "catedral_handover_latency",
        "Handover latency",
        buckets=[0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0],
    )
    DAEMON_STATUS = Gauge(
        "catedral_daemon_status", "Daemon status (1=active, 0=inactive)", ["daemon"]
    )
else:
    PHI_GAUGE = HANDOVER_COUNTER = LATENCY_HISTOGRAM = DAEMON_STATUS = None


@dataclass
class DaemonSpec:
    name: str
    priority: int
    start: Callable[[], Awaitable[Any]]
    stop: Callable[[Any], Awaitable[None]]
    health_check: Callable[[Any], Awaitable[bool]]
    ready: bool = False
    instance: Any = None
    last_health: float = 0.0


class OrquestradorV304:
    def __init__(self, config_path: str = "/opt/catedral-os/config/production.yaml"):
        self.config = self._load_config(config_path)
        self.daemons: Dict[str, DaemonSpec] = {}
        self.running = False
        self._notifier = SystemdNotifier()
        self._reload_flag = False
        self._health_interval = self.config.get("health_interval_seconds", 5)
        self._watchdog_interval = self.config.get("watchdog_interval_seconds", 10)

        hup = getattr(signal, "SIGHUP", None)
        if hup is not None:
            signal.signal(hup, self._handle_reload)

        if PROMETHEUS_AVAILABLE:
            metrics_port = self.config.get("metrics_port", 9090)
            start_http_server(metrics_port)
            logger.info(f"Metrics available at :{metrics_port}/metrics")

        self._notifier.notify("Initializing...")

    def _load_config(self, path: str) -> dict:
        try:
            if yaml is not None:
                with open(path) as f:
                    return yaml.safe_load(f) or {}
        except FileNotFoundError:
            logger.warning(f"Config {path} not found, using defaults")
        except Exception as e:
            logger.error(f"Config load error: {e}")
        return {
            "health_interval_seconds": 5,
            "watchdog_interval_seconds": 10,
            "metrics_port": 9090,
        }

    def _handle_reload(self, signum, frame):
        logger.info("SIGHUP received — reloading configuration")
        self._reload_flag = True

    def register(self, spec: DaemonSpec) -> None:
        self.daemons[spec.name] = spec
        if DAEMON_STATUS:
            DAEMON_STATUS.labels(daemon=spec.name).set(0)

    async def start_all(self) -> None:
        self.running = True
        sorted_daemons = sorted(self.daemons.values(), key=lambda s: s.priority)

        for spec in sorted_daemons:
            try:
                logger.info(f"Starting {spec.name}...")
                spec.instance = await spec.start()
                spec.ready = True
                if DAEMON_STATUS:
                    DAEMON_STATUS.labels(daemon=spec.name).set(1)
                logger.info(f"{spec.name} started")
            except Exception as e:
                logger.error(f"{spec.name} failed: {e}")
                spec.ready = False
                if DAEMON_STATUS:
                    DAEMON_STATUS.labels(daemon=spec.name).set(0)

        self._notifier.ready()
        self._notifier.notify("Running")
        await self._health_loop()

    async def _health_loop(self) -> None:
        last_watchdog = time.time()

        while self.running:
            for name, spec in self.daemons.items():
                if not spec.ready:
                    continue
                try:
                    healthy = await spec.health_check(spec.instance)
                    if not healthy:
                        logger.warning(f"{name} unhealthy — restarting")
                        await self._restart(spec)
                    spec.last_health = time.time()
                    if DAEMON_STATUS:
                        DAEMON_STATUS.labels(daemon=name).set(1 if healthy else 0)
                except Exception as e:
                    logger.error(f"Health check {name}: {e}")
                    if DAEMON_STATUS:
                        DAEMON_STATUS.labels(daemon=name).set(0)

            now = time.time()
            if now - last_watchdog > self._watchdog_interval:
                self._notifier.watchdog()
                last_watchdog = now

            if self._reload_flag:
                await self._reload()

            await asyncio.sleep(self._health_interval)

    async def _reload(self) -> None:
        logger.info("Reloading configuration...")
        self._reload_flag = False
        self.config = self._load_config(
            self.config.get("config_path", "/opt/catedral-os/config/production.yaml")
        )
        self._health_interval = self.config.get("health_interval_seconds", 5)
        self._notifier.notify("Configuration reloaded")

    async def _restart(self, spec: DaemonSpec) -> None:
        try:
            if spec.instance is not None:
                await spec.stop(spec.instance)
            spec.ready = False
            if DAEMON_STATUS:
                DAEMON_STATUS.labels(daemon=spec.name).set(0)
            await asyncio.sleep(0.5)
            spec.instance = await spec.start()
            spec.ready = True
            if DAEMON_STATUS:
                DAEMON_STATUS.labels(daemon=spec.name).set(1)
            logger.info(f"{spec.name} restarted")
        except Exception as e:
            logger.error(f"Failed to restart {spec.name}: {e}")

    async def stop_all(self) -> None:
        self.running = False
        self._notifier.stopping()
        self._notifier.notify("Stopping...")

        for name, spec in self.daemons.items():
            if spec.ready and spec.instance is not None:
                try:
                    logger.info(f"Stopping {name}...")
                    await spec.stop(spec.instance)
                    spec.ready = False
                    if DAEMON_STATUS:
                        DAEMON_STATUS.labels(daemon=name).set(0)
                except Exception as e:
                    logger.error(f"Error stopping {name}: {e}")

        self._notifier.notify("Stopped")
        logger.info("Orquestrador finalized")


async def start_decisor():
    from daemons.decisor import DecisorDaemon
    daemon = DecisorDaemon()
    await daemon.start()
    return daemon


async def stop_decisor(instance):
    await instance.stop()


async def health_decisor(instance):
    return await instance.health_check()


async def start_zeno():
    from daemons.zeno import ZenoDaemon
    daemon = ZenoDaemon()
    await daemon.start()
    return daemon


async def stop_zeno(instance):
    await instance.stop()


async def health_zeno(instance):
    return await instance.health_check()


async def start_coherence():
    from daemons.coherence import CoherenceDaemon
    daemon = CoherenceDaemon()
    await daemon.start()
    return daemon


async def stop_coherence(instance):
    await instance.stop()


async def health_coherence(instance):
    return await instance.health_check()


async def main():
    orch = OrquestradorV304()

    orch.register(
        DaemonSpec(
            name="coherence",
            priority=5,
            start=start_coherence,
            stop=stop_coherence,
            health_check=health_coherence,
        )
    )
    orch.register(
        DaemonSpec(
            name="decisor",
            priority=10,
            start=start_decisor,
            stop=stop_decisor,
            health_check=health_decisor,
        )
    )
    orch.register(
        DaemonSpec(
            name="zeno",
            priority=20,
            start=start_zeno,
            stop=stop_zeno,
            health_check=health_zeno,
        )
    )

    try:
        await orch.start_all()
    except KeyboardInterrupt:
        logger.info("Interrupted by user")
    finally:
        await orch.stop_all()


if __name__ == "__main__":
    asyncio.run(main())
