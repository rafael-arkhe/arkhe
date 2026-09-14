import asyncio
import logging
import time

logger = logging.getLogger(__name__)


class CoherenceDaemon:
    def __init__(self):
        self.running = False
        self.phi = 0.85
        self.handover_count = 0
        self._task = None

    async def start(self):
        self.running = True
        self._task = asyncio.create_task(self._loop())
        logger.info("Coherence Daemon started")

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
        logger.info("Coherence Daemon stopped")

    async def health_check(self) -> bool:
        return self.running and 0.5 < self.phi < 1.0

    async def _loop(self):
        while self.running:
            await asyncio.sleep(0.5)
