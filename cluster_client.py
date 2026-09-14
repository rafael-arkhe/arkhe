#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
cluster_client.py — Cliente da Cluster Protocol API (BLOCO 483 — v44)
================================================================================
API descentralizada de inferência de IA (OpenAI-compatible) com pagamento via
x402 (USDC na Base) e roteamento por provedor:
    Venice  (E2EE, privacidade)
    Phala   (TEE, hardware-level privacy)
    0G      (descentralizado)
    Groq    (ultra-low latency)

Integrações na Catedral OS:
    - Arkhe-JAX (LinguisticController)  -> feedback real de governança
    - Observador Primordial             -> explicações de anomalias
    - Zeno                              -> sugestões de correção
    - Litografia Quântica               -> interpretação de comandos físicos
    - Fingerprinting                    -> relatórios de auditoria em linguagem natural

Protocolo: SASC-EXTERNAL-INFERENCE-2026
Selo: CLUSTER-CATEDRAL-2026-09-01
invariantes: Ethics-1 (227-F), Ethics-2 (Data Minimization), Provenance-1 (TLSNotary)

Endpoint de referência:
    https://api.clusterprotocol.ai/v1/chat/completions
"""

import os
import re
import json
import time
import base64
import logging
from typing import Any, Dict, Generator, List, Optional

import requests

logger = logging.getLogger("ClusterClient")

# Provider padronizados pela Cluster Protocol. "zerog" é o provedor 0G.
PROVIDERS = ("venice", "phala", "zerog", "groq", "together",
             "fireworks", "deepinfra", "sambanova", "redpill")

DEFAULT_MODEL = "llama-3.3-70b-instruct"
DEFAULT_BASE_URL = "https://api.clusterprotocol.ai"
DEFAULT_PRICE_USDC = 0.003  # por chamada /v1/chat/completions


class ClusterConfig:
    """Configuração do cliente Cluster Protocol."""

    def __init__(
        self,
        base_url: str = DEFAULT_BASE_URL,
        api_key: Optional[str] = None,
        default_model: str = DEFAULT_MODEL,
        default_provider: str = "venice",
        max_retries: int = 3,
        retry_delay: float = 1.0,
        timeout: int = 30,
        use_x402: bool = False,
        private_key: Optional[str] = None,
        x402_payment_header: Optional[str] = None,
    ) -> None:
        self.base_url = base_url.rstrip("/")
        self.api_key = api_key or os.environ.get("CLUSTER_API_KEY")
        self.default_model = default_model
        self.default_provider = default_provider
        self.max_retries = max_retries
        self.retry_delay = retry_delay
        self.timeout = timeout
        self.use_x402 = use_x402
        # Chave privada usada SOMENTE para assinatura off-chain de testes
        # (x402 em produção exige carteira Base + EIP-3009 — ver _sign_payment)
        self.private_key = private_key
        # Cabeçalho X-PAYMENT pré-assinado (teste/homologação, sem carteira)
        self.x402_payment_header = x402_payment_header

    @classmethod
    def from_env(cls) -> "ClusterConfig":
        return cls(
            api_key=os.environ.get("CLUSTER_API_KEY"),
            default_model=os.environ.get("CLUSTER_DEFAULT_MODEL", DEFAULT_MODEL),
            default_provider=os.environ.get("CLUSTER_DEFAULT_PROVIDER", "venice"),
            base_url=os.environ.get("CLUSTER_BASE_URL", DEFAULT_BASE_URL),
            timeout=int(os.environ.get("CLUSTER_TIMEOUT", "30")),
        )


class ClusterError(Exception):
    """Erro de comunicação com a Cluster Protocol."""

    def __init__(self, status_code: int, detail: str) -> None:
        self.status_code = status_code
        super().__init__(f"Cluster HTTP {status_code}: {detail}")


class ClusterClient:
    """
    Cliente para a API Cluster Protocol com dois modos de autenticação:
    API Key (Bearer) ou x402 (pagamento por requisição com USDC na Base).
    """

    def __init__(self, config: Optional[ClusterConfig] = None) -> None:
        self.config = config or ClusterConfig.from_env()
        self.session = requests.Session()
        self._cost_accumulator = {"requests": 0, "tokens": 0}

        if self.config.api_key:
            self.session.headers.update({"Authorization": f"Bearer {self.config.api_key}"})
        elif not self.config.use_x402:
            logger.warning(
                "Nenhuma autenticação configurada: defina CLUSTER_API_KEY "
                "ou habilite use_x402=True."
            )

    # ------------------------------------------------------------------
    # HELPERS
    # ------------------------------------------------------------------

    @staticmethod
    def _extract_content(response: Dict[str, Any]) -> str:
        """Extrai o texto da resposta OpenAI-compatible sem lançar exceção."""
        if not isinstance(response, dict):
            return ""
        choices = response.get("choices") or []
        if not choices:
            return ""
        message = choices[0].get("message") or {}
        return message.get("content") or ""

    @staticmethod
    def _extract_usage(response: Dict[str, Any]) -> Dict[str, int]:
        usage = (response or {}).get("usage") or {}
        return {
            "prompt_tokens": int(usage.get("prompt_tokens", 0)),
            "completion_tokens": int(usage.get("completion_tokens", 0)),
            "total_tokens": int(usage.get("total_tokens", 0)),
        }

    def _record_cost(self, response: Dict[str, Any]) -> None:
        usage = self._extract_usage(response)
        self._cost_accumulator["requests"] += 1
        self._cost_accumulator["tokens"] += usage["total_tokens"]

    @property
    def estimated_cost_usdc(self) -> float:
        """Custo acumulado (USDC) das chamadas de chat desta sessão."""
        return self._cost_accumulator["requests"] * DEFAULT_PRICE_USDC

    def _sleep_backoff(self, attempt: int, response: requests.Response = None) -> None:
        wait = self.config.retry_delay * (2 ** attempt)
        if response is not None:
            reset = response.headers.get("X-RateLimit-Reset") or response.headers.get("Retry-After")
            if reset:
                try:
                    wait = max(wait, float(reset))
                except (TypeError, ValueError):
                    pass
        time.sleep(min(wait, 60.0))

    # ------------------------------------------------------------------
    # CHAT COMPLETION
    # ------------------------------------------------------------------

    def chat_completion(
        self,
        messages: List[Dict[str, str]],
        model: Optional[str] = None,
        provider: Optional[str] = None,
        temperature: float = 0.7,
        max_tokens: int = 256,
        stream: bool = False,
    ) -> Dict[str, Any]:
        """
        Executa inferência de chat (formato OpenAI).

        Args:
            messages: lista de mensagens [{"role", "content"}].
            model: modelo (default: config.default_model).
            provider: proveniente (venice, phala, zerog, groq, ...).
            temperature: 0-2. max_tokens: limite de geração.
            stream: True retorna apenas um dicionário resumo; para consumo
                    incremental use `stream_chunks`.

        Returns:
            dict no formato OpenAI ({choices, usage, ...}) ou
            dict de erro estruturado.
        """
        payload = self._build_payload(messages, model, provider, temperature, max_tokens, stream)
        url = f"{self.config.base_url}/v1/chat/completions"

        try:
            if self.config.use_x402 and not self.config.api_key:
                response = self._chat_with_x402(payload, url)
            else:
                response = self._chat_with_api_key(payload, url, stream)
        except ClusterError as exc:
            return {"error": f"HTTP_{exc.status_code}", "details": str(exc)}

        self._record_cost(response)
        return response

    def _build_payload(
        self,
        messages: List[Dict[str, str]],
        model: Optional[str],
        provider: Optional[str],
        temperature: float,
        max_tokens: int,
        stream: bool,
    ) -> Dict[str, Any]:
        if provider and provider not in PROVIDERS:
            logger.warning(f"Provedor '{provider}' não está na lista canônica {PROVIDERS}.")
        payload: Dict[str, Any] = {
            "model": model or self.config.default_model,
            "messages": messages,
            "temperature": temperature,
            "max_tokens": max_tokens,
            "stream": stream,
        }
        if provider:
            payload["provider"] = provider
        return payload

    def _chat_with_api_key(self, payload: Dict[str, Any], url: str, stream: bool) -> Dict[str, Any]:
        """Chamada com API Key (Bearer); retries com backoff exponencial."""
        last_error: Optional[str] = None
        for attempt in range(self.config.max_retries):
            try:
                response = self.session.post(
                    url,
                    json=payload,
                    timeout=self.config.timeout,
                )
            except requests.exceptions.RequestException as exc:
                last_error = f"network: {exc}"
                logger.warning(f"Tentativa {attempt + 1} falhou: {exc}")
                self._sleep_backoff(attempt)
                continue

            if response.status_code == 200:
                return self._parse_json(response)
            if response.status_code == 402:
                raise ClusterError(402, f"Balance insuficiente: {response.text[:400]}")
            if response.status_code == 429:
                logger.warning(f"Rate limit (429); aguardando backoff. Tentativa {attempt + 1}")
                self._sleep_backoff(attempt, response)
                continue
            if 500 <= response.status_code <= 599:
                logger.warning(f"Erro 5xx ({response.status_code}); tentativa {attempt + 1}")
                self._sleep_backoff(attempt, response)
                continue

            raise ClusterError(response.status_code, response.text[:400])

        raise ClusterError(599, f"MAX_RETRIES_EXCEEDED ({last_error or 'sem resposta'})")

    def _chat_with_x402(self, payload: Dict[str, Any], url: str) -> Dict[str, Any]:
        """
        Fluxo x402: sem auth -> HTTP 402 com 'payment-required' (base64 JSON)
        -> assina pagamento USDC na Base -> resubmete com X-PAYMENT.
        A assinatura real (EIP-3009 transferWithAuthorization) exige carteira
        Base; esta implementação assina off-chain apenas em modo de teste.
        """
        headers = {"Content-Type": "application/json"}
        response = self.session.post(url, json=payload, headers=headers, timeout=self.config.timeout)

        if response.status_code == 402:
            payment_info = None
            raw = response.headers.get("payment-required")
            if raw:
                try:
                    payment_info = json.loads(base64.b64decode(raw))
                except (ValueError, json.JSONDecodeError):
                    payment_info = {"raw": raw}

            signed = self._sign_payment(payment_info or {})
            if signed is not None:
                headers["X-PAYMENT"] = signed
                retry = self.session.post(url, json=payload, headers=headers, timeout=self.config.timeout)
                if retry.status_code == 200:
                    return self._parse_json(retry)
                return {"error": f"HTTP_{retry.status_code}", "details": retry.text[:400]}

            logger.warning("Pagamento requerido e não pôde ser assinado (sem carteira).")
            return {"error": "PAYMENT_REQUIRED", "payment_info": payment_info, "status_code": 402}

        if response.status_code == 200:
            return self._parse_json(response)
        return {"error": f"HTTP_{response.status_code}", "details": response.text[:400]}

    def _sign_payment(self, payment_info: Dict[str, Any]) -> Optional[str]:
        """
        Retorna o cabeçalho X-PAYMENT assinado ou None se impossível.
        Produção: exigir @x402/fetch (carteira Base). Teste: usar
        config.x402_payment_header pré-assinado.
        """
        if self.config.x402_payment_header:
            return self.config.x402_payment_header
        if self.config.private_key and "amount" in payment_info:
            # Assinatura EIP-3009 (transferWithAuthorization) — implementar com
            # eth-account/eth-keys. Placeholder honesto: não assina em claro.
            logger.warning("private_key configurado, mas assinatura EIP-3009 não implementada.")
            return None
        return None

    @staticmethod
    def _parse_json(response: requests.Response) -> Dict[str, Any]:
        try:
            return response.json()
        except ValueError:
            logger.warning("Resposta 200 não-JSON; devolvendo texto cru.")
            return {"raw_text": response.text}

    # ------------------------------------------------------------------
    # STREAMING (SSE)
    # ------------------------------------------------------------------

    def stream_chunks(self, messages: List[Dict[str, str]], **kwargs: Any) -> Generator[str, None, None]:
        """
        Streaming SSE: produz os deltas de conteúdo conforme chegam.
        Uso típico: pesquisa incremental no Arkhe-JAX.
        """
        payload = self._build_payload(
            messages,
            kwargs.get("model"),
            kwargs.get("provider"),
            kwargs.get("temperature", 0.7),
            kwargs.get("max_tokens", 256),
            stream=True,
        )
        url = f"{self.config.base_url}/v1/chat/completions"
        with self.session.post(
            url,
            json=payload,
            timeout=self.config.timeout,
            stream=True,
            headers=self.session.headers,
        ) as response:
            if response.status_code != 200:
                raise ClusterError(response.status_code, response.text[:400])
            for line in response.iter_lines(decode_unicode=True):
                if not line or not line.startswith("data: "):
                    continue
                data = line[len("data: "):].strip()
                if data == "[DONE]":
                    return
                try:
                    chunk = json.loads(data)
                except json.JSONDecodeError:
                    continue
                delta = ((chunk.get("choices") or [{}])[0].get("delta") or {})
                content = delta.get("content")
                if content:
                    yield content

    # ------------------------------------------------------------------
    # USO NA CATEDRAL: OBSERVADOR, ZENO, LITOGRAFIA, FINGERPRINTING
    # ------------------------------------------------------------------

    def generate_explanation(self, anomaly: str, context: str, max_tokens: int = 512) -> str:
        """Explicação em linguagem natural para o Observador Primordial."""
        messages = [
            {"role": "system", "content": (
                "Você é o Observador Primordial da Catedral OS. Forneça explicações "
                "claras e concisas sobre anomalias de coerência (Φ_C), sem inventar dados. "
                "Se o contexto for insuficiente, diga 'dados insuficientes'."
            )},
            {"role": "user", "content": f"Anomalia: {anomaly}\nContexto: {context}"},
        ]
        response = self.chat_completion(
            messages, max_tokens=max_tokens, temperature=0.3, provider="venice"
        )
        return self._extract_content(response) or f"Erro: {response.get('error', 'resposta vazia')}"

    def suggest_correction(self, anomaly: str, phi_before: float, phi_after: float) -> str:
        """Sugestão de correção para o veto do Zeno (protocolo 233)."""
        messages = [
            {"role": "system", "content": (
                "Você é o Zeno da Catedral OS. Dada uma anomalia e a variação de Φ_C "
                "antes/depois de um handover, sugira UMA correção concreta e acionável, "
                "mencionando qual invariante (Gap/Exit/Quantum) se aplica."
            )},
            {"role": "user", "content": (
                f"Anomalia: {anomaly}\nΦ_C antes: {phi_before:.4f}\nΦ_C depois: {phi_after:.4f}"
            )},
        ]
        response = self.chat_completion(messages, max_tokens=384, temperature=0.4, provider="groq")
        return self._extract_content(response) or f"Erro: {response.get('error', 'resposta vazia')}"

    def interpret_command(self, user_command: str) -> str:
        """Interpreta comandos naturais para a Litografia Quântica (parâmetros físicos)."""
        messages = [
            {"role": "system", "content": (
                "Você é a Litografia Quântica da Catedral OS. Traduza comandos do usuário "
                "(sliders/intenções) em parâmetros físicos (amplitudes de pulso, fases, "
                "exposição), em linguagem técnica concisa."
            )},
            {"role": "user", "content": user_command},
        ]
        response = self.chat_completion(messages, max_tokens=256, temperature=0.5, provider="venice")
        return self._extract_content(response) or ""

    # ------------------------------------------------------------------
    # ARKHE-JAX: FEEDBACK DE GOVERNANÇA
    # ------------------------------------------------------------------

    def governance_feedback(self, phi_history: List[float]) -> str:
        """
        Pede à LLM novos parâmetros de governança e devolve TEXTO cru no formato
        'chlave = valor' — compatível com LinguisticController.parse_llm_response
        (regex), tornando o parse robusto a Markdown/JSON/idiossincrasias.
        """
        history_str = ", ".join(f"{x:.4f}" for x in phi_history[-20:])
        messages = [
            {"role": "system", "content": (
                "Você é o Arquiteto de Governança da Catedral OS. Analise o histórico de "
                "coerência (Φ) e sugira valores para gamma_B, E0, omega, eta, kappa, mu e xi "
                "para melhorar a estabilidade. Responda APENAS com linhas no formato "
                "'gamma_B = 1.20' (uma por parâmetro, sem explicação)."
            )},
            {"role": "user", "content": f"Histórico de Φ: {history_str}"},
        ]
        response = self.chat_completion(messages, max_tokens=256, temperature=0.5)
        return self._extract_content(response)


class ArkheJAXClusterBridge:
    """
    Ponte entre a Cluster Protocol e o Arkhe-JAX v6 (arkhe_jax_v60.py).

    Substitui as respostas simuladas por inferência real:
        feedback = client.governance_feedback(phi_history)
        arkhe.run_iteration(feedback, t)
    O parse acontece dentro de LinguisticController.parse_llm_response (regex),
    logo as respostas da LLM são tolerantes a variação de formato.
    """

    def __init__(self, client: ClusterClient, arkhe) -> None:
        self.client = client
        self.arkhe = arkhe
        self.fallback_to_defaults = True

    def run_iteration(self, t: float, phi_history: Optional[List[float]] = None) -> Dict[str, Any]:
        """Um passo do RSI com feedback real da Cluster Protocol."""
        if phi_history is None:
            phi_history = [m.get("metrics", {}).get("C_surface", 1.0)
                           for m in self.arkhe.history[-20:]] or [1.0]

        feedback = self.client.governance_feedback(phi_history)
        if not feedback:
            logger.warning("Feedback vazio da Cluster; caindo para texto simulado.")
            feedback = (
                f"gamma_B = {self.arkhe.protocol.gamma_B}\n"
                f"E0 = {self.arkhe.protocol.E0}\n"
                "Sem ajuste (rede indisponível)."
            )
        return self.arkhe.run_iteration(feedback, t)

    def run_loop(self, t_values: List[float], phi_history: Optional[List[float]] = None) -> List[Dict[str, Any]]:
        results = []
        history = list(phi_history) if phi_history else []
        for t in t_values:
            result = self.run_iteration(t, history)
            results.append(result)
            if "error" not in result:
                history.append(result.get("params", {}).get("gamma_B",
                                                             self.arkhe.protocol.gamma_B))
        return results


# ==========================================================================
# EXEMPLO DE USO (não requer rede quando CLUSTER_API_KEY não está definida)
# ==========================================================================

if __name__ == "__main__":
    import sys

    logging.basicConfig(level=logging.INFO, format="%(asctime)s [%(levelname)s] %(message)s")

    config = ClusterConfig.from_env()  # usa CLUSTER_API_KEY / CLUSTER_DEFAULT_PROVIDER
    client = ClusterClient(config)

    phi = [0.92, 0.94, 0.91, 0.95, 0.93]
    logger.info(f"Provedor default: {config.default_provider}; custo/chamada: ${DEFAULT_PRICE_USDC}")

    # Modo verificação hermética: sem CLUSTER_API_KEY, apenas auto-teste local.
    live = ("--live" in sys.argv) or bool(config.api_key)
    if not live:
        logger.info("Modo off-line (sem rede): passe --live ou defina CLUSTER_API_KEY para inferência real.")
        sys.exit(0)

    feedback = client.governance_feedback(phi)
    if feedback:
        parsed = {}
        for key in ("gamma_B", "E0", "eta", "omega"):
            m = re.search(rf"(?:{re.escape(key)})\s*=\s*([-+]?[0-9]*\.?[0-9]+(?:[eE][-+]?[0-9]+)?)", feedback)
            if m:
                parsed[key] = float(m.group(1))
        logger.info("Feedback Cluster recebido.")
        logger.info(f"Parâmetros parseados: {parsed or 'nenhum (erro de formato)'}")
    else:
        logger.info("Sem feedback (CLUSTER_API_KEY não definida ou rede indisponível) — modo off-line.")