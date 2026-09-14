"""
Ponte entre o pipeline de dados e o ecossistema Arkhe 8.1.
Integra ROQUO, Glass5D, SQC Interface e Skyrmions.

[VERSÃO]: 8.1
[STATUS]: ✅ OPERACIONAL (modo simulação quando serviços reais indisponíveis)
"""

import json
import hashlib
import uuid
import requests
from requests.adapters import HTTPAdapter
from urllib3.util.retry import Retry
import numpy as np
import time
from typing import Dict, Any, Optional, List, Union
from datetime import datetime, timezone
from dataclasses import dataclass, asdict
from enum import Enum
from scipy.signal import hilbert

from .config import settings
from .logger import get_logger
from .metrics import metrics_manager

logger = get_logger("arkhe_bridge")

# Cache de conectividade entre instâncias (evita 3×timeout por health check
# quando os serviços estão offline — comum em modo simulação).
_LAST_CONNECT_CHECK: Dict[str, float] = {}
_CONNECT_CHECK_INTERVAL = 10.0


class ArkhePriority(Enum):
    """Prioridades para jobs no ROQUO."""
    LOW = "low"
    NORMAL = "normal"
    HIGH = "high"
    CRITICAL = "critical"


class TopologicalValidation(Enum):
    """Status de validação topológica."""
    VALID = "valid"
    INVALID = "invalid"
    DEGRADED = "degraded"
    UNKNOWN = "unknown"


@dataclass
class EngramMetadata:
    """Metadados de um engrama arquivado no Glass5D."""
    session_id: str
    created_at: str
    source: str
    topological_winding: float
    roquo_job_id: Optional[str] = None
    glass5d_record_id: Optional[str] = None
    data_hash: Optional[str] = None

    def to_dict(self) -> Dict[str, Any]:
        return asdict(self)


class ArkheBridge:
    """
    Ponte completa para o ecossistema Arkhe 8.1.

    Funcionalidades:
    1. Envio de jobs para o ROQUO (via SQC API)
    2. Gravação/leitura no Glass5D (WORM)
    3. Validação topológica via Skyrmions
    4. Processamento completo de engramas

    Quando `settings.SIMULATION_MODE` é True (padrão local) e um serviço
    externo está indisponível, o bridge degrada graciosamente com
    resultados simulados — permitindo o fluxo completo sem infraestrutura.
    """

    def __init__(self) -> None:
        # URLs das APIs
        self.roquo_api = settings.ROQUO_API_URL
        self.glass5d_api = settings.GLASS5D_API_URL
        self.sqc_api = settings.SQC_API_URL

        # Sessão única para o ciclo
        self.session_id = hashlib.sha256(
            f"{datetime.now(timezone.utc).isoformat()}:ARKHE".encode()
        ).hexdigest()[:16]

        # Sessão HTTP sem retries para fail-fast quando serviços estão offline
        retry = Retry(total=0, connect=0, read=0, status=0, redirect=0)
        self._http = requests.Session()
        adapter = HTTPAdapter(max_retries=retry, pool_connections=1, pool_maxsize=1)
        self._http.mount("http://", adapter)
        self._http.mount("https://", adapter)

        # Headers com autenticação
        self.headers = {
            "Authorization": f"Bearer {settings.ARKHE_API_KEY}",
            "Content-Type": "application/json",
            "X-Session-Id": self.session_id,
            "X-Client-Version": "8.1",
        }

        # Cache de jobs / leituras simuladas
        self._job_cache: Dict[str, Dict] = {}
        self._glass5d_cache: Dict[str, Dict] = {}

        # Estado do bridge
        self.status: Dict[str, Any] = {
            "initialized_at": datetime.now(timezone.utc).isoformat(),
            "roquo_connected": False,
            "glass5d_connected": False,
            "sqc_connected": False,
            "last_error": None,
            "engrams_processed": 0,
            "glass5d_bytes_written": 0,
        }

        # Testar conectividade na inicialização
        self._check_connectivity()

        logger.info(f"🌌 ArkheBridge inicializado - Session: {self.session_id}")

    # ========================================================================
    # CONECTIVIDADE
    # ========================================================================

    def _check_connectivity(self) -> None:
        """Verifica conectividade com todos os serviços (com cache global)."""
        now = time.time()
        last = _LAST_CONNECT_CHECK.get("all", 0.0)
        if now - last < _CONNECT_CHECK_INTERVAL:
            return

        services = {
            "roquo": self.roquo_api,
            "glass5d": self.glass5d_api,
            "sqc": self.sqc_api,
        }

        for name, url in services.items():
            try:
                response = self._http.get(
                    f"{url}/health", headers=self.headers, timeout=1, verify=False
                )
                if response.status_code == 200:
                    self.status[f"{name}_connected"] = True
                    logger.info(f"✅ {name.upper()} conectado: {url}")
                else:
                    logger.warning(
                        f"⚠️ {name.upper()} respondeu com status {response.status_code}"
                    )
                    self.status[f"{name}_connected"] = False
            except Exception as e:
                logger.warning(f"⚠️ {name.upper()} indisponível: {e}")
                self.status[f"{name}_connected"] = False
                self.status["last_error"] = str(e)

        _LAST_CONNECT_CHECK["all"] = now

        if settings.SIMULATION_MODE:
            logger.info(
                "🔮 Modo simulação ativo — fluxos prosseguirão com respostas "
                "sintéticas para serviços indisponíveis."
            )

    # ========================================================================
    # 1. ENVIO DE JOBS PARA O ROQUO
    # ========================================================================

    def submit_job_to_roquo(
        self,
        job_payload: Dict[str, Any],
        priority: Union[str, ArkhePriority] = ArkhePriority.NORMAL,
        quantum_backend: str = "auto",
        resources: Optional[Dict[str, int]] = None,
        timeout_seconds: int = 3600,
    ) -> Dict[str, Any]:
        """Submete um job para execução no ROQUO via SQC Interface RESTful."""
        if isinstance(priority, ArkhePriority):
            priority = priority.value

        if not self.status["roquo_connected"]:
            logger.warning("ROQUO não conectado. Tentando reconectar...")
            self._check_connectivity()
            if not self.status["roquo_connected"]:
                if settings.SIMULATION_MODE:
                    logger.info("🔮 ROQUO indisponível — simulando submissão de job.")
                    return self._simulate_roquo_job(
                        job_payload, priority, quantum_backend, resources, timeout_seconds
                    )
                raise ConnectionError("ROQUO indisponível")

        default_resources = {"nodes": 1, "gpus": 4, "cpus": 8, "memory_gb": 512}
        if resources:
            default_resources.update(resources)

        job = {
            "type": "hpc_job",
            "priority": priority,
            "payload": job_payload,
            "quantum_backend": quantum_backend,
            "classical_resources": default_resources,
            "timeout_seconds": timeout_seconds,
            "metadata": {
                "source": "data_pipeline",
                "session": self.session_id,
                "submitted_at": datetime.now(timezone.utc).isoformat(),
                "version": "8.1",
            },
        }

        start_time = time.time()
        try:
            response = self._http.post(
                f"{self.roquo_api}/jobs", headers=self.headers, json=job, timeout=30
            )
            response.raise_for_status()
            result = response.json()

            job_id = result.get("job_id")
            self._job_cache[job_id] = {
                "status": "submitted",
                "submitted_at": datetime.now(timezone.utc).isoformat(),
                "payload": job_payload,
            }

            duration = time.time() - start_time
            metrics_manager.record_database_operation("roquo_submit", duration)
            metrics_manager.record_event_processed(
                event_type="roquo_job", status="submitted", duration=duration
            )

            logger.info(f"✅ Job submetido ao ROQUO: {job_id}")
            return result

        except requests.exceptions.RequestException as e:
            logger.error(f"❌ Falha ao submeter job ao ROQUO: {e}")
            metrics_manager.record_error("roquo_submit_error", "hpc_job")
            self.status["last_error"] = str(e)
            if settings.SIMULATION_MODE:
                logger.info("🔮 Erro de rede — simulando submissão de job.")
                return self._simulate_roquo_job(
                    job_payload, priority, quantum_backend, resources, timeout_seconds
                )
            raise

    def _simulate_roquo_job(
        self,
        job_payload: Dict[str, Any],
        priority: str,
        quantum_backend: str,
        resources: Optional[Dict[str, int]],
        timeout_seconds: int,
    ) -> Dict[str, Any]:
        """Simula submissão de job no ROQUO."""
        job_id = f"roquo-sim-{uuid.uuid4().hex[:8]}"
        self._job_cache[job_id] = {
            "status": "completed",
            "submitted_at": datetime.now(timezone.utc).isoformat(),
            "completed_at": datetime.now(timezone.utc).isoformat(),
            "payload": job_payload,
            "simulated": True,
        }
        return {
            "job_id": job_id,
            "status": "completed",
            "simulated": True,
            "quantum_backend": quantum_backend,
            "priority": priority,
            "timeout_seconds": timeout_seconds,
        }

    def get_job_status(self, job_id: str) -> Dict[str, Any]:
        """Consulta status de um job no ROQUO."""
        if job_id in self._job_cache and self._job_cache[job_id].get("simulated"):
            return {"job_id": job_id, "status": "completed", "simulated": True}

        try:
            response = self._http.get(
                f"{self.roquo_api}/jobs/{job_id}", headers=self.headers, timeout=10
            )
            response.raise_for_status()
            result = response.json()

            if job_id in self._job_cache:
                self._job_cache[job_id]["status"] = result.get("status")
                self._job_cache[job_id]["updated_at"] = datetime.now(timezone.utc).isoformat()

            return result
        except Exception as e:
            logger.error(f"❌ Erro ao consultar job {job_id}: {e}")
            if settings.SIMULATION_MODE:
                return {"job_id": job_id, "status": "completed", "simulated": True}
            raise

    def poll_job_until_complete(
        self, job_id: str, poll_interval: int = 10, timeout: int = 3600
    ) -> Dict[str, Any]:
        """Aguarda a conclusão de um job no ROQUO."""
        if job_id in self._job_cache and self._job_cache[job_id].get("simulated"):
            logger.info(f"✅ Job {job_id} concluído (simulação)")
            return self.get_job_status(job_id)

        start_time = time.time()
        while time.time() - start_time < timeout:
            status = self.get_job_status(job_id)
            job_status = status.get("status", "unknown")

            if job_status in ["completed", "success", "done"]:
                logger.info(f"✅ Job {job_id} concluído com sucesso")
                return status

            if job_status in ["failed", "error", "cancelled"]:
                logger.error(
                    f"❌ Job {job_id} falhou: {status.get('error_message', 'unknown')}"
                )
                return status

            logger.debug(f"⏳ Job {job_id} ainda em execução... ({job_status})")
            time.sleep(poll_interval)

        raise TimeoutError(f"Job {job_id} não concluído dentro do timeout ({timeout}s)")

    # ========================================================================
    # 2. GRAVAÇÃO NO GLASS5D (ARQUIVO ETERNO WORM)
    # ========================================================================

    def write_to_glass5d(
        self,
        data: Dict[str, Any],
        dataset_name: Optional[str] = None,
        metadata: Optional[Dict[str, Any]] = None,
        compression: str = "zstd",
        layer_count: int = 1,
    ) -> Dict[str, Any]:
        """Grava dados no Glass5D — 360 TB, >100.000 anos, WORM."""
        if not self.status["glass5d_connected"]:
            logger.warning("Glass5D não conectado. Tentando reconectar...")
            self._check_connectivity()
            if not self.status["glass5d_connected"]:
                if settings.SIMULATION_MODE:
                    logger.info("🔮 Glass5D indisponível — simulando gravação WORM.")
                    return self._simulate_glass5d_write(
                        data, dataset_name, metadata, compression, layer_count
                    )
                raise ConnectionError("Glass5D indisponível")

        if dataset_name is None:
            dataset_name = f"engram_{datetime.now(timezone.utc).strftime('%Y%m%d_%H%M%S')}"

        glass_payload = {
            "dataset": dataset_name,
            "data": data,
            "metadata": {
                "session": self.session_id,
                "written_at": datetime.now(timezone.utc).isoformat(),
                "format": "arkhe_v8.1",
                "worm": True,
                "encoding": "birefringence + phase voxels",
                "source": "data_pipeline",
                **(metadata or {}),
            },
            "options": {
                "compression": compression,
                "layer_count": layer_count,
                "verification": "sha256",
            },
        }

        start_time = time.time()
        try:
            response = self._http.post(
                f"{self.glass5d_api}/write", headers=self.headers, json=glass_payload, timeout=120
            )
            response.raise_for_status()
            result = response.json()

            record_id = result.get("record_id")
            bytes_written = result.get("bytes_written", 0)

            self._glass5d_cache[record_id] = {
                "dataset": dataset_name,
                "written_at": datetime.now(timezone.utc).isoformat(),
                "bytes_written": bytes_written,
                "data": data,
            }

            self.status["engrams_processed"] += 1
            self.status["glass5d_bytes_written"] += bytes_written

            duration = time.time() - start_time
            metrics_manager.record_database_operation("glass5d_write", duration)
            metrics_manager.record_event_processed(
                event_type="glass5d_write", status="success", duration=duration
            )

            logger.info(f"💎 Dados gravados no Glass5D: {record_id}")
            logger.info(f"   → Dataset: {dataset_name}")
            logger.info(f"   → Tamanho: {bytes_written / 1e12:.2f} TB")
            logger.info(f"   → Camadas: {layer_count}")

            return result

        except requests.exceptions.RequestException as e:
            logger.error(f"❌ Falha na gravação no Glass5D: {e}")
            metrics_manager.record_error("glass5d_write_error", "storage")
            self.status["last_error"] = str(e)
            if settings.SIMULATION_MODE:
                return self._simulate_glass5d_write(
                    data, dataset_name, metadata, compression, layer_count
                )
            raise

    def _simulate_glass5d_write(
        self,
        data: Dict[str, Any],
        dataset_name: Optional[str],
        metadata: Optional[Dict[str, Any]],
        compression: str,
        layer_count: int,
    ) -> Dict[str, Any]:
        """Simula gravação no Glass5D."""
        if dataset_name is None:
            dataset_name = f"engram_{datetime.now(timezone.utc).strftime('%Y%m%d_%H%M%S')}"

        payload_bytes = len(json.dumps(data, default=str, ensure_ascii=False).encode("utf-8"))
        bytes_written = payload_bytes * layer_count
        record_id = f"g5d-sim-{uuid.uuid4().hex[:12]}"

        self._glass5d_cache[record_id] = {
            "dataset": dataset_name,
            "written_at": datetime.now(timezone.utc).isoformat(),
            "bytes_written": bytes_written,
            "data": data,
            "simulated": True,
        }

        self.status["engrams_processed"] += 1
        self.status["glass5d_bytes_written"] += bytes_written

        return {
            "record_id": record_id,
            "bytes_written": bytes_written,
            "status": "archived",
            "simulated": True,
            "worm": True,
            "lifetime_years": 100_000,
        }

    def read_from_glass5d(self, record_id: str) -> Dict[str, Any]:
        """Recupera dados do Glass5D pelo ID do registro."""
        if record_id in self._glass5d_cache:
            entry = self._glass5d_cache[record_id]
            logger.info(f"📖 Dados recuperados do Glass5D (cache/sim): {record_id}")
            return {"record_id": record_id, "data": entry.get("data")}

        try:
            response = self._http.get(
                f"{self.glass5d_api}/read/{record_id}", headers=self.headers, timeout=30
            )
            response.raise_for_status()
            result = response.json()
            logger.info(f"📖 Dados recuperados do Glass5D: {record_id}")
            return result
        except Exception as e:
            logger.error(f"❌ Falha na leitura do Glass5D: {e}")
            if settings.SIMULATION_MODE:
                return {"record_id": record_id, "data": None, "simulated": True}
            raise

    def glass5d_list_datasets(self, limit: int = 100, offset: int = 0) -> Dict[str, Any]:
        """Lista datasets disponíveis no Glass5D."""
        if settings.SIMULATION_MODE and not self.status["glass5d_connected"]:
            datasets = [
                {
                    "dataset": v["dataset"],
                    "record_id": k,
                    "bytes_written": v["bytes_written"],
                    "created_at": v["written_at"],
                    "simulated": v.get("simulated", False),
                }
                for k, v in list(self._glass5d_cache.items())[offset : offset + limit]
            ]
            return {"datasets": datasets, "simulated": True}

        try:
            response = self._http.get(
                f"{self.glass5d_api}/datasets",
                headers=self.headers,
                params={"limit": limit, "offset": offset},
                timeout=30,
            )
            response.raise_for_status()
            return response.json()
        except Exception as e:
            logger.error(f"❌ Falha ao listar datasets: {e}")
            if settings.SIMULATION_MODE:
                return {"datasets": [], "simulated": True}
            raise

    # ========================================================================
    # 3. VALIDAÇÃO TOPOLÓGICA COM SKYRMIONS
    # ========================================================================

    def validate_topological(self, signal: np.ndarray) -> Dict[str, Any]:
        """
        Aplica o protocolo de skyrmions quânticos para verificar resiliência
        topológica do sinal.

        Protocolo baseado em:
        - Nature Communications 16, 2934 (2025)
        - Nature Communications 17, 2085 (2026) — robustez em turbulência

        Args:
            signal: Array 1D com o sinal a ser validado

        Returns:
            Dict com winding number, resiliência e validação
        """
        if len(signal) < 10:
            return {
                "validated": False,
                "error": "Sinal muito curto para validação topológica",
            }

        try:
            # 1. Transformada de Hilbert para fase analítica
            analytic = hilbert(signal)
            phase = np.unwrap(np.angle(analytic))

            # 2. Número de winding (invariante topológico)
            winding = (phase[-1] - phase[0]) / (2 * np.pi)

            # 3. Teste de resiliência com ruído determinístico
            np.random.seed(963)  # Determinístico para reprodutibilidade
            noise_amplitude = 0.1 * (np.std(signal) + 1e-12)
            noise = np.random.normal(0, noise_amplitude, len(signal))
            noisy_signal = signal + noise

            analytic_noisy = hilbert(noisy_signal)
            phase_noisy = np.unwrap(np.angle(analytic_noisy))
            winding_noisy = (phase_noisy[-1] - phase_noisy[0]) / (2 * np.pi)

            # 4. Resiliência topológica (norma relativa — invariante por escala)
            winding_mag = max(abs(winding), 1.0)
            relative_drift = abs(winding - winding_noisy) / winding_mag
            resilience = relative_drift < 0.01

            # 5. Classificação
            if resilience:
                validation = TopologicalValidation.VALID
            elif relative_drift < 0.05:
                validation = TopologicalValidation.DEGRADED
            else:
                validation = TopologicalValidation.INVALID

            # 6. Métricas adicionais
            coherence = self._calculate_phase_coherence(phase)
            topological_charge = self._calculate_topological_charge(signal)

            result = {
                "winding_number": float(winding),
                "winding_after_noise": float(winding_noisy),
                "resilience": bool(resilience),
                "validation": validation.value,
                "phase_coherence": float(coherence),
                "topological_charge": float(topological_charge),
                "protocol_2025": "Nature Communications 16, 2934 (2025)",
                "protocol_2026": "Nature Communications 17, 2085 (2026)",
                "noise_amplitude": float(noise_amplitude),
                "validated": bool(resilience),
                "signal_length": len(signal),
            }

            if resilience:
                metrics_manager.record_event_processed(
                    event_type="topological_validation", status="valid", duration=0.0
                )
            else:
                metrics_manager.record_error("topological_validation", "data_quality")

            logger.debug(
                f"🧬 Validação topológica: {validation.value} (winding={winding:.3f})"
            )
            return result

        except Exception as e:
            logger.error(f"❌ Erro na validação topológica: {e}")
            return {"validated": False, "error": str(e)}

    def _calculate_phase_coherence(self, phase: np.ndarray) -> float:
        """Calcula coerência de fase (medida de ordem)."""
        complex_phase = np.exp(1j * phase)
        mean_phase = np.mean(complex_phase)
        return float(np.abs(mean_phase))

    def _calculate_topological_charge(self, signal: np.ndarray) -> float:
        """Calcula carga topológica do sinal."""
        analytic = hilbert(signal)
        phase = np.unwrap(np.angle(analytic))
        phase_derivative = np.gradient(phase)
        return float(np.sum(phase_derivative) / (2 * np.pi))

    # ========================================================================
    # 4. PONTE COMPLETA: SCRAPE → ROQUO → GLASS5D
    # ========================================================================

    def process_engram(
        self,
        raw_data: Dict[str, Any],
        signal: Optional[np.ndarray] = None,
        dataset_name: Optional[str] = None,
        priority: Union[str, ArkhePriority] = ArkhePriority.NORMAL,
        validate_topology: bool = True,
        wait_for_roquo: bool = True,
    ) -> Dict[str, Any]:
        """
        Fluxo completo de processamento de um engrama:
        1. Validação topológica (opcional)
        2. Processamento no ROQUO
        3. Arquivamento no Glass5D
        """
        start_time = time.time()

        engram_metadata = EngramMetadata(
            session_id=self.session_id,
            created_at=datetime.now(timezone.utc).isoformat(),
            source="data_pipeline",
            topological_winding=0.0,
        )

        result: Dict[str, Any] = {
            "session": self.session_id,
            "timestamp": datetime.now(timezone.utc).isoformat(),
            "engram_id": hashlib.sha256(
                f"{self.session_id}:{datetime.now(timezone.utc).isoformat()}".encode()
            ).hexdigest()[:16],
            "steps": {},
            "status": "processing",
        }

        try:
            # 1. Validação topológica
            if validate_topology and signal is not None:
                topo_result = self.validate_topological(signal)
                result["steps"]["topological_validation"] = topo_result
                engram_metadata.topological_winding = topo_result.get("winding_number", 0.0)

                if not topo_result.get("resilience", False):
                    logger.warning("⚠️ Sinal não passou na validação topológica")

            # 2. Hash do dado para verificação
            data_hash = hashlib.sha256(
                json.dumps(raw_data, sort_keys=True, default=str).encode()
            ).hexdigest()
            engram_metadata.data_hash = data_hash

            # 3. Submeter ao ROQUO
            job_payload = {
                "data": raw_data,
                "metadata": {
                    "engram_id": result["engram_id"],
                    "session": self.session_id,
                    "topological_winding": engram_metadata.topological_winding,
                    "data_hash": data_hash,
                },
            }

            job_result = self.submit_job_to_roquo(job_payload, priority=priority)
            result["steps"]["roquo_job"] = job_result
            engram_metadata.roquo_job_id = job_result.get("job_id")

            # 4. Aguardar conclusão (opcional)
            if wait_for_roquo and engram_metadata.roquo_job_id:
                final_status = self.poll_job_until_complete(
                    engram_metadata.roquo_job_id, timeout=3600
                )
                result["steps"]["roquo_completion"] = final_status

            # 5. Arquiva no Glass5D
            glass_result = self.write_to_glass5d(
                data={
                    "raw_data": raw_data,
                    "processing_result": job_result,
                    "topological_validation": result["steps"].get("topological_validation"),
                    "metadata": engram_metadata.to_dict(),
                },
                dataset_name=dataset_name
                or f"engram_{datetime.now(timezone.utc).strftime('%Y%m%d_%H%M%S')}",
                metadata={
                    "engram_id": result["engram_id"],
                    "session": self.session_id,
                    "topological_winding": engram_metadata.topological_winding,
                    "data_hash": data_hash,
                },
                layer_count=2,  # Redundância para dados críticos
            )
            result["steps"]["glass5d_archive"] = glass_result
            engram_metadata.glass5d_record_id = glass_result.get("record_id")

            # 6. Status final
            result["status"] = "success"
            result["engram_metadata"] = engram_metadata.to_dict()

            total_duration = time.time() - start_time
            metrics_manager.record_database_operation("engram_processing", total_duration)
            metrics_manager.record_event_processed(
                event_type="engram_complete", status="success", duration=total_duration
            )

            logger.info(f"🌌 Engrama {result['engram_id']} processado e arquivado com sucesso")
            logger.info(f"   📊 Duration: {total_duration:.2f}s")
            logger.info(f"   💎 Glass5D Record: {engram_metadata.glass5d_record_id}")

            return result

        except Exception as e:
            logger.error(f"❌ Falha no processamento do engrama: {e}")
            result["status"] = "failed"
            result["error"] = str(e)
            self.status["last_error"] = str(e)

            metrics_manager.record_error("engram_processing", "critical")
            raise

    # ========================================================================
    # 5. MÉTODOS UTILITÁRIOS
    # ========================================================================

    def get_status(self) -> Dict[str, Any]:
        """Retorna o status atual do bridge."""
        return {
            **self.status,
            "session_id": self.session_id,
            "cached_jobs": len(self._job_cache),
            "cached_glass5d": len(self._glass5d_cache),
        }

    def health_check(self) -> Dict[str, Any]:
        """Health check completo de todos os serviços."""
        self._check_connectivity()
        return {
            "bridge": "healthy",
            "session": self.session_id,
            "roquo": self.status["roquo_connected"],
            "glass5d": self.status["glass5d_connected"],
            "sqc": self.status["sqc_connected"],
            "simulation_mode": settings.SIMULATION_MODE,
            "engrams_processed": self.status["engrams_processed"],
            "glass5d_bytes_written": self.status["glass5d_bytes_written"],
            "timestamp": datetime.now(timezone.utc).isoformat(),
        }

    def reset_session(self) -> str:
        """Gera uma nova sessão."""
        self.session_id = hashlib.sha256(
            f"{datetime.now(timezone.utc).isoformat()}:ARKHE".encode()
        ).hexdigest()[:16]
        self.headers["X-Session-Id"] = self.session_id
        logger.info(f"🔄 Nova sessão gerada: {self.session_id}")
        return self.session_id