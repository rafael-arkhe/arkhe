# =============================================================================
# BLOCO 470 v15 — REDIS CLUSTER (R1)
# redis_cache_production_v15.py
#
# Cliente Redis de produção com:
#   - Sentinel para failover automático
#   - Connection pooling (master/slave discovery)
#   - Serialização JSON otimizada
#   - Cache stampede protection (setnx)
#   - Stats de hit/miss/errors
#
# Requer: pip install redis
# Env:    REDIS_PASSWORD, REDIS_SENTINEL_HOSTS (host:port csv)
# =============================================================================
import redis
from redis.sentinel import Sentinel
import os
import json
from typing import Optional, Dict, Any, List, Tuple
import logging


def _parse_sentinel_hosts(raw: str) -> List[Tuple[str, int]]:
    """Converte 'host:port,host:port' em lista de tuplas."""
    hosts = []
    for entry in raw.split(","):
        entry = entry.strip()
        if not entry:
            continue
        if ":" in entry:
            host, _, port = entry.partition(":")
            hosts.append((host, int(port)))
        else:
            hosts.append((entry, 26379))
    return hosts or [("redis-catedral-sentinel", 26379)]


class RedisCacheProduction:
    """
    Redis cache para produção com failover automático via Sentinel.
    """

    def __init__(self, sentinel_hosts: Optional[List[Tuple[str, int]]] = None):
        self.password = os.environ.get("REDIS_PASSWORD", "")
        self.sentinel_hosts = sentinel_hosts or _parse_sentinel_hosts(
            os.environ.get("REDIS_SENTINEL_HOSTS", "redis-catedral-sentinel:26379")
        )

        self.sentinel = Sentinel(
            self.sentinel_hosts,
            password=self.password or None,
            socket_timeout=5,
            retry_on_timeout=True,
        )

        self._stats: Dict[str, int] = {"hits": 0, "misses": 0, "errors": 0}

    def _get_client(self) -> redis.Redis:
        """Obtém o master via Sentinel."""
        return self.sentinel.master_for(
            "mymaster",
            password=self.password or None,
            socket_timeout=5,
            retry_on_timeout=True,
        )

    def _get_slave(self) -> redis.Redis:
        """Obtém uma réplica via Sentinel (leitura distribuída)."""
        return self.sentinel.slave_for(
            "mymaster",
            password=self.password or None,
            socket_timeout=5,
            retry_on_timeout=True,
        )

    # ------------------------------------------------------------------------
    # OPS
    # ------------------------------------------------------------------------
    def get(self, key: str) -> Optional[Any]:
        """Obtém valor do cache."""
        try:
            client = self._get_client()
            data = client.get(key)
            if data:
                self._stats["hits"] += 1
                return json.loads(data)
            self._stats["misses"] += 1
            return None
        except Exception as e:
            self._stats["errors"] += 1
            logging.error(f"Redis get error: {e}")
            return None

    def set(self, key: str, value: Any, ttl: int = 300) -> bool:
        """Armazena valor com TTL."""
        try:
            client = self._get_client()
            return bool(client.setex(key, ttl, json.dumps(value, default=str)))
        except Exception as e:
            self._stats["errors"] += 1
            logging.error(f"Redis set error: {e}")
            return False

    def setnx(self, key: str, value: Any, ttl: int = 300) -> bool:
        """Set if not exists — para cache stampede protection."""
        try:
            client = self._get_client()
            return bool(client.set(key, json.dumps(value, default=str), ex=ttl, nx=True))
        except Exception as e:
            self._stats["errors"] += 1
            logging.error(f"Redis setnx error: {e}")
            return False

    def delete(self, key: str) -> bool:
        """Remove uma chave do cache."""
        try:
            client = self._get_client()
            return bool(client.delete(key))
        except Exception as e:
            self._stats["errors"] += 1
            logging.error(f"Redis delete error: {e}")
            return False

    def get_read_only(self, key: str) -> Optional[Any]:
        """Leitura via réplica (custo menor, relaxa consistência)."""
        try:
            client = self._get_slave()
            data = client.get(key)
            if data:
                self._stats["hits"] += 1
                return json.loads(data)
            self._stats["misses"] += 1
            return None
        except Exception as e:
            self._stats["errors"] += 1
            logging.error(f"Redis slave get error: {e}")
            return None

    def get_or_compute(self, key: str, compute, ttl: int = 300) -> Optional[Any]:
        """
        Cache-aside com stampede protection.
        Tenta setnx com valor temporário de "in-flight"; se outro processo
        venceu, lê o valor que ele gravou após a computação.
        """
        cached = self.get(key)
        if cached is not None:
            return cached

        # Stampede protection: lock leve por chave
        lock_key = f"lock:{key}"
        if self.setnx(lock_key, {"computing": True}, ttl=30):
            try:
                value = compute()
                if value is not None:
                    self.set(key, value, ttl)
                return value
            finally:
                self.delete(lock_key)
        else:
            # Já está sendo computado; aguarda e lê
            import time

            for _ in range(20):
                time.sleep(0.25)
                cached = self.get(key)
                if cached is not None:
                    return cached
            return None

    def get_stats(self) -> Dict:
        """Estatísticas do cache."""
        total = self._stats["hits"] + self._stats["misses"]
        return {
            "hits": self._stats["hits"],
            "misses": self._stats["misses"],
            "hit_rate": self._stats["hits"] / max(total, 1),
            "errors": self._stats["errors"],
            "sentinel_hosts": [f"{h}:{p}" for h, p in self.sentinel_hosts],
        }


if __name__ == "__main__":
    logging.basicConfig(level=logging.INFO, format="%(asctime)s %(levelname)s %(message)s")
    cache = RedisCacheProduction()
    cache.set("probe", {"ok": True}, ttl=60)
    print("Probe:", cache.get("probe"))
    print("Stats:", cache.get_stats())