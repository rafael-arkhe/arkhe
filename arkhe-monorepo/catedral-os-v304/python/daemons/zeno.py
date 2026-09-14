import asyncio
import logging
import time

logger = logging.getLogger(__name__)


class ZenoDaemon:
    def __init__(self):
        self.running = False
        self.veto_count = 0
        self.coherence_threshold = 0.577350
        self._task = None

    async def start(self):
        self.running = True
        self._task = asyncio.create_task(self._loop())
        logger.info("Zeno Daemon started")

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
        logger.info("Zeno Daemon stopped")

    async def health_check(self) -> bool:
        return self.running

    async def _loop(self):
        while self.running:
            await asyncio.sleep(1.0)
