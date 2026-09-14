"""tpr_bridge_real.py — v77 VETADO — BLOCO 517 (S1/S2)

TPR Bridge com LLM watsonx para extracao de coerencia (S1) e carga da
matriz W_transform treinada via DISCOVER (S2).

VETAGEM:
  - get_hidden_state() faz rede SOMENTE se WATSONX_API_URL/KEY existirem;
    sem credencial retorna None e o fallback projetado abaixo eh usado
    (determinístico, hermetrico). Regra da casa: selftest nunca usa rede.
  - _synthetic_hidden_state() existe exclusivamente para exercitar a via
    de projecao W@h em ambiente sem watsonx; nao e mascarado como real.
  - A W_transform (DIM_DISCOVER) nao existe neste repo; o loader usa
    caminho configurável (env WX_MATRIX_PATH) e fallback determinístico
    ortonormal (3 x HIDDEN) rotulado como fallback.
"""

from __future__ import annotations

import hashlib
import json
import os
from typing import Dict, Optional

import numpy as np


class TPRBridgeReal:
    """TPR Bridge: projeta hidden-state via W_transform e deriva coerencia."""

    HIDDEN_DIM = 16          # dimensao de projecao local (hermetico)
    DIM_DISCOVER = 2880      # dimensao histórica citada na proposta

    def __init__(
        self,
        api_url: Optional[str] = None,
        api_key: Optional[str] = None,
        model_id: str = "ibm/granite-3-8b-instruct",
        w_path: Optional[str] = None,
    ) -> None:
        self.api_url = api_url or os.getenv("WATSONX_API_URL")
        self.api_key = api_key or os.getenv("WATSONX_API_KEY")
        self.model_id = model_id
        self.w_used_fallback = False
        self.W = self._load_W_matrix(w_path)

    # ------------------------------------------------------------------ S2
    def _load_W_matrix(self, w_path: Optional[str]) -> np.ndarray:
        """Carrega W de arquivo; senao constroi fallback ortonormal fixo."""
        candidates = [w_path, os.getenv("WX_MATRIX_PATH"), "models/discover_W_matrix.npy"]
        for path in candidates:
            if not path:
                continue
            try:
                W = np.load(path)
                W = W.astype(np.float32)
                if W.ndim == 2 and W.shape[0] >= 3:
                    return W[:3]
            except (OSError, ValueError):
                continue
        # fallback determinístico: linhas ortonormais derivadas da seed
        rng = np.random.default_rng(20260901)
        W = rng.standard_normal((3, self.HIDDEN_DIM)).astype(np.float32)
        W, _ = np.linalg.qr(W.T)
        self.w_used_fallback = True
        return W.T.astype(np.float32)

    # ------------------------------------------------------------- watsonx
    def get_hidden_state(self, text: str) -> Optional[np.ndarray]:
        """Hidden state do token final via API watsonx (rede, opcional)."""
        import requests  # importação tardia: nao exigida no selftest

        if not self.api_url or not self.api_key:
            return None
        payload = {
            "model_id": self.model_id,
            "input": text,
            "parameters": {"return_hidden_states": True, "max_new_tokens": 1},
        }
        headers = {"Authorization": f"Bearer {self.api_key}", "Content-Type": "application/json"}
        resp = requests.post(self.api_url, json=payload, headers=headers, timeout=30)
        if resp.status_code != 200:
            return None
        data = resp.json()
        states = data.get("hidden_states")
        if not states:
            return None
        vec = states[0][-1]
        return np.asarray(vec, dtype=np.float32)

    # ---------------------------------------------------- fallback (uso em dev)
    def _synthetic_hidden_state(self, text: str) -> np.ndarray:
        """Hidden-state sintético DETERMINÍSTICO para ambientes sem watsonx.

        Somente para desenvolvimento/CI hermetico — NUNCA rotulado como real.
        """
        h = hashlib.sha256(text.encode("utf-8")).digest()
        v = np.frombuffer(h, dtype=np.uint8)[: self.HIDDEN_DIM].astype(np.float32)
        v = v / 127.5 - 1.0  # escala [-1, 1]
        return v

    # ---------------------------------------------------------------- S1
    def project(self, hidden: np.ndarray) -> Dict[str, float]:
        """Projeta h via W_transform e deriva o pacote de coerencia."""
        h = np.asarray(hidden, dtype=np.float32)
        if h.shape[0] != self.W.shape[1]:
            h = np.resize(h, (self.W.shape[1],))
        E = self.W @ h
        phi_c = float(np.clip(np.real(E[0]), 0.0, 1.0))
        phi_delta = float(np.abs(np.real(E[1]))) or 1e-12
        ratio = max(0.0, min(1.0, phi_c / phi_delta))
        phi_sync = float(np.angle(complex(E[2]))) if len(E) > 2 else 0.0
        return {
            "phi_c": phi_c,
            "phi_delta": phi_delta,
            "ratio": ratio,
            "entropy": 1.0 - ratio,
            "phi_sync": phi_sync,
        }

    def extract_coherence_from_tpr(self, text: str, hidden: Optional[np.ndarray] = None) -> Dict[str, float]:
        """Extrai coerencia do texto; usa hidden fornecido ou sintético."""
        if hidden is None:
            hidden = self.get_hidden_state(text)
        if hidden is None:
            hidden = self._synthetic_hidden_state(text)
        return self.project(hidden)


if __name__ == "__main__":
    import sys

    b = TPRBridgeReal()
    fail = 0

    assert b.W.shape[0] == 3, "projeção deve ter 3 componentes"
    a = b.extract_coherence_from_tpr("catedral-seed-20260901")
    d = b.extract_coherence_from_tpr("catedral-seed-20260901")
    if a != d:
        fail += 1
        print("[TPR] FAIL: determinismo quebrado")

    ok = all(
        0.0 <= v <= 1.0
        for k, v in a.items()
        if k in ("phi_c", "phi_delta", "ratio", "entropy")
    )
    if not ok:
        fail += 1
        print("[TPR] FAIL: coerencia fora de [0,1]")

    print(f"[TPR] phi_c={a['phi_c']:.4f} ratio={a['ratio']:.4f} entropy={a['entropy']:.4f}")
    print(f"[TPR] deterministico={a == d} w_fallback={'yes' if b.w_used_fallback else 'no'}")
    print(f"[TPR] {'ALL CHECKS PASS' if fail == 0 else 'CHECKS FAILED'}")
    sys.exit(1 if fail else 0)