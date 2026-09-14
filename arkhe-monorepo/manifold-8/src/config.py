"""
Configuração central do ecossistema Arkhe 8.1.

Todas as variáveis de ambiente podem ser sobrescritas via docker-compose.
Quando os serviços reais (ROQUO/Glass5D/SQC) não estão disponíveis,
o bridge opera em modo simulação.
"""

import os
from dataclasses import dataclass, field
from typing import Dict


@dataclass
class Settings:
    """Configuração carregada do ambiente com padrões para simulação."""

    # Rotas dos serviços Arkhe
    ROQUO_API_URL: str = "http://localhost:8081"
    GLASS5D_API_URL: str = "http://localhost:8082"
    SQC_API_URL: str = "http://localhost:8083"

    # Autenticação
    ARKHE_API_KEY: str = "arkhe-sim-963"

    # Banco / infraestrutura
    POSTGRES_HOST: str = "localhost"
    POSTGRES_PORT: int = 5432
    POSTGRES_DB: str = "integrated_system"
    POSTGRES_USER: str = "arkhe"
    POSTGRES_PASSWORD: str = "arkhe_password"
    REDIS_HOST: str = "localhost"
    REDIS_PORT: int = 6379
    KAFKA_BOOTSTRAP_SERVERS: str = "localhost:9092"
    KAFKA_TOPIC: str = "events.raw"
    S3_ENDPOINT_URL: str = "http://localhost:9000"
    METRICS_PORT: int = 8000

    # Flags de integração
    GLASS5D_ENABLED: bool = True
    ROQUO_ENABLED: bool = True
    SIMULATION_MODE: bool = field(default_factory=lambda: True)

    @classmethod
    def _from_env(cls, name: str, default: str) -> str:
        return os.environ.get(name, default)

    @classmethod
    def from_env(cls) -> "Settings":
        """Constrói Settings a partir das variáveis de ambiente."""
        s = cls()
        s.ROQUO_API_URL = cls._from_env("ROQUO_API_URL", s.ROQUO_API_URL)
        s.GLASS5D_API_URL = cls._from_env("GLASS5D_API_URL", s.GLASS5D_API_URL)
        s.SQC_API_URL = cls._from_env("SQC_API_URL", s.SQC_API_URL)
        s.ARKHE_API_KEY = cls._from_env("ARKHE_API_KEY", s.ARKHE_API_KEY)
        s.POSTGRES_HOST = cls._from_env("POSTGRES_HOST", s.POSTGRES_HOST)
        s.POSTGRES_PORT = int(cls._from_env("POSTGRES_PORT", str(s.POSTGRES_PORT)))
        s.REDIS_HOST = cls._from_env("REDIS_HOST", s.REDIS_HOST)
        s.KAFKA_BOOTSTRAP_SERVERS = cls._from_env("KAFKA_BOOTSTRAP_SERVERS", s.KAFKA_BOOTSTRAP_SERVERS)
        s.KAFKA_TOPIC = cls._from_env("KAFKA_TOPIC", s.KAFKA_TOPIC)
        s.S3_ENDPOINT_URL = cls._from_env("S3_ENDPOINT_URL", s.S3_ENDPOINT_URL)
        s.METRICS_PORT = int(cls._from_env("METRICS_PORT", str(s.METRICS_PORT)))
        s.GLASS5D_ENABLED = cls._from_env("GLASS5D_ENABLED", "true").lower() == "true"
        s.ROQUO_ENABLED = cls._from_env("ROQUO_ENABLED", "true").lower() == "true"
        s.SIMULATION_MODE = cls._from_env("SIMULATION_MODE", "true").lower() == "true"
        return s

    def to_dict(self) -> Dict:
        """Exports settings as dict (sem segredos)."""
        return {
            "roquo_api": self.ROQUO_API_URL,
            "glass5d_api": self.GLASS5D_API_URL,
            "sqc_api": self.SQC_API_URL,
            "simulation_mode": self.SIMULATION_MODE,
            "glass5d_enabled": self.GLASS5D_ENABLED,
            "roquo_enabled": self.ROQUO_ENABLED,
        }


settings = Settings.from_env()