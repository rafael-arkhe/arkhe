import asyncio
import logging
import time

logger = logging.getLogger(__name__)


class DecisorDaemon:
    def __init__(self, target_phi: float = 0.992, decay_rate: float = 0.001):
        self.running = False
        self.phi = 0.85
        self.target_phi = target_phi
        self.decay_rate = decay_rate
        self._task = None

    async def start(self):
        self.running = True
        self._task = asyncio.create_task(self._loop())
        logger.info("Decisor Daemon started")

    async def serve(self):
        """Roda start() e mantém o mesmo event loop ativo até ser parado."""
        await self.start()
        await asyncio.Event().wait()

    async def stop(self):
        self.running = False
        if self._task:
            self._task.cancel()
            try:
                await self._task
            except asyncio.CancelledError:
                pass
        logger.info("Decisor Daemon stopped")

    async def health_check(self) -> bool:
        return 0.5 < self.phi < 1.0

    async def _loop(self):
        while self.running:
            delta = (self.target_phi - self.phi) * self.decay_rate
            self.phi += delta
            self.phi = max(0.0, min(1.0, self.phi))
            await asyncio.sleep(0.1)
