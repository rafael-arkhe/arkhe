# =============================================================================
# BLOCO 470 v13 — CACHE REDIS PARA RESOLUÇÃO DID (O3)
# did_cache_v13.py
#
# Cache Redis para resolução de DIDs com TTL e invalidação por validUntil.
#
# Requer: pip install redis
# Env:    REDIS_URL (default redis://localhost:6379/0)
# =============================================================================
import json
import redis
from typing import Dict, Optional
from datetime import datetime, timedelta
import hashlib


class DIDCache:
    """
    Cache Redis para resolução de DIDs com TTL e validUntil.
    Prefixo de chave: resolver:obj:{did}
    """

    def __init__(self, redis_url: str = "redis://localhost:6379/0", ttl: int = 300):
        self.redis = redis.from_url(redis_url)
        self.ttl = ttl  # TTL padrão em segundos
        self.key_prefix = "resolver:obj:"

    def get(self, did: str) -> Optional[Dict]:
        """Obtém um DID do cache, validando validUntil e TTL."""
        key = f"{self.key_prefix}{did}"
        data = self.redis.get(key)

        if data:
            doc = json.loads(data)

            # Invalidação por validUntil (expiração do próprio documento)
            if "validUntil" in doc:
                try:
                    valid_until = datetime.fromisoformat(doc["validUntil"])
                    if datetime.utcnow() > valid_until:
                        self.redis.delete(key)
                        return None
                except ValueError:
                    pass

            # Invalidação por TTL do Redis
            ttl = self.redis.ttl(key)
            if ttl <= 0:
                self.redis.delete(key)
                return None

            return doc

        return None

    def set(self, did: str, document: Dict, ttl: int = None) -> None:
        """Armazena um DID no cache."""
        key = f"{self.key_prefix}{did}"

        # Timestamp de cache
        document["_cachedAt"] = datetime.utcnow().isoformat()

        # Se o documento tem validUntil, o TTL deriva dele (máx. 24h)
        if "validUntil" in document:
            try:
                valid_until = datetime.fromisoformat(document["validUntil"])
                cache_ttl = max(0, int((valid_until - datetime.utcnow()).total_seconds()))
                if cache_ttl > 0:
                    self.redis.setex(key, min(cache_ttl, 86400), json.dumps(document))
                    return
            except ValueError:
                pass

        # TTL padrão
        self.redis.setex(key, ttl or self.ttl, json.dumps(document))

    def invalidate(self, did: str) -> None:
        """Invalida um DID no cache."""
        key = f"{self.key_prefix}{did}"
        self.redis.delete(key)

    def invalidate_pattern(self, pattern: str) -> int:
        """Invalida DIDs por padrão global (ex.: did:web:*)."""
        pattern = f"{self.key_prefix}{pattern}*"
        keys = self.redis.keys(pattern)
        if keys:
            return int(self.redis.delete(*keys))
        return 0

    def get_stats(self) -> Dict:
        """Obtém estatísticas do cache."""
        keys = self.redis.keys(f"{self.key_prefix}*")
        return {
            "total_cached": len(keys),
            "prefix": self.key_prefix,
            "ttl": self.ttl,
        }

    def touch(self, did: str, ttl: int = None) -> bool:
        """Renova o TTL de um DID sem reescrever o documento."""
        key = f"{self.key_prefix}{did}"
        return bool(self.redis.expire(key, ttl or self.ttl))


class DIDResolverWithCache:
    """
    Resolvedor de DIDs com cache Redis e registros de resolvedores.
    """

    def __init__(self, cache: DIDCache = None):
        self.cache = cache or DIDCache()
        self._resolvers = {}

    def register_resolver(self, method: str, resolver_func) -> None:
        """Registra um resolvedor para um método DID."""
        self._resolvers[method] = resolver_func

    def resolve(self, did: str, force: bool = False) -> Optional[Dict]:
        """
        Resolve um DID com cache.
        force=True ignora o cache e re-resolve na fonte.
        """
        # 1. Verifica cache (exceto se force)
        if not force:
            cached = self.cache.get(did)
            if cached:
                return cached

        # 2. Extrai o método (did:method:...)
        if not did.startswith("did:"):
            raise ValueError(f"Invalid DID: {did}")
        method = did.split(":")[1] if len(did.split(":")) > 1 else None

        # 3. Resolve usando o resolvedor apropriado
        if method in self._resolvers:
            document = self._resolvers[method](did)
        else:
            document = self._resolve_generic(did)

        # 4. Cacheia o resultado
        if document:
            self.cache.set(did, document)

        return document

    def _resolve_generic(self, did: str) -> Optional[Dict]:
        """Resolução genérica para DIDs."""
        if did.startswith("did:key:"):
            return self._resolve_did_key(did)
        if did.startswith("did:web:"):
            return self._resolve_did_web(did)
        return None

    def _resolve_did_key(self, did: str) -> Dict:
        """Resolve um did:key (construção local do documento)."""
        public_key = did.replace("did:key:", "")
        return {
            "@context": "https://www.w3.org/ns/did/v1",
            "id": did,
            "verification_method": [{
                "id": f"{did}#key-1",
                "type": "Ed25519VerificationKey2020",
                "controller": did,
                "publicKeyMultibase": public_key,
            }],
            "authentication": [f"{did}#key-1"],
            "assertionMethod": [f"{did}#key-1"],
        }

    def _resolve_did_web(self, did: str) -> Optional[Dict]:
        """Resolve um did:web delegando ao DIDWebManager."""
        from did_web_server_v13 import DIDWebManager
        return DIDWebManager.resolve_did_web(did)


if __name__ == "__main__":
    resolver = DIDResolverWithCache()
    doc = resolver.resolve("did:key:z6Mkexample")
    print("Resolvido:", doc)
    print("Stats:", resolver.cache.get_stats())