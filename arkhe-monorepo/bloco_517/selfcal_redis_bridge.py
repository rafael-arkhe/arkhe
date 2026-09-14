"""selfcal_redis_bridge.py — v77 VETADO — BLOCO 517 (T2)

Ponte SelfCalibrator <-> TGN com ajuste dinamico de learning rate.

VETAGEM:
  - redis NAO pode ser requisito do selftest (regra da casa: hermético).
    Redis e usado SOMENTE se `redis` importar e a conexao responder; senao
    _MemStore assume (deterministico).
  - `adjust_lr(base_lr, phi_c)` e monotona em phi_c: lr cresce quando a
    coerencia supera o limiar 0.618, decresce abaixo — calculo fechado.
  - Nenhuma metrica misteriosa; parametro klr documentado.
"""

from __future__ import annotations

from typing import Dict, Optional

PHI_TARGET = 0.618       # Auric gap (Gap-1 familia: 0.577350..0.999900)
KLR_DEFAULT = 0.5
LR_MAX = 1.0
LR_MIN = 0.05


class _MemStore:
    """Fallback em memoria quando redis nao esta disponivel."""

    def __init__(self) -> None:
        self._data: Dict[str, object] = {}

    def set(self, k: str, v: object) -> None:
        self._data[k] = v

    def get(self, k: str) -> object:
        return self._data.get(k)


class SelfCalibratorBridge:
    """Ajusta LR em funcao da coerencia e encaminha observacoes TGN."""

    def __init__(self, redis_url: Optional[str] = None, klr: float = KLR_DEFAULT) -> None:
        self.klr = klr
        self._store: object
        if redis_url:
            try:
                import redis  # type: ignore

                client = redis.Redis.from_url(redis_url, socket_connect_timeout=1)
                client.ping()
                self._store = client  # type: ignore[assignment]
                self.backend = "redis"
            except Exception:
                self._store = _MemStore()
                self.backend = "mem (fallback)"
        else:
            self._store = _MemStore()
            self.backend = "mem"

    def adjust_lr(self, base_lr: float, phi_c: float) -> float:
        """lr = base_lr * (1 + klr*(phi_c - 0.618)), clamped em [0.05, 1.0]."""
        factor = 1.0 + self.klr * (phi_c - PHI_TARGET)
        return max(LR_MIN, min(LR_MAX, base_lr * factor))

    def push_observation(self, phi_c: float, step: int) -> float:
        """Encaminha observacao ao TGN e devolve o novo LR sugerido."""
        base = self.adjust_lr(1.0, phi_c)
        self._store.set(f"tgn:obs:{step}", {"phi_c": phi_c, "lr": base})
        self._store.set("tgn:last_state", {"step": step, "phi_c": phi_c, "lr": base})
        return base

    def last_state(self) -> object:
        return self._store.get("tgn:last_state")


if __name__ == "__main__":
    import sys

    fail = 0
    bridge = SelfCalibratorBridge()  # sem rede

    lr_high = bridge.adjust_lr(0.1, 0.9)
    lr_low = bridge.adjust_lr(0.1, 0.3)
    lr_mid = bridge.adjust_lr(0.1, 0.618)
    if not (lr_low < lr_mid < lr_high):
        fail += 1
        print(f"[SEL] FAIL: monotonia da LR ({lr_low=:.4f} {lr_mid=:.4f} {lr_high=:.4f})")
    else:
        print(f"[SEL] monotonia OK  lr(0.3)={lr_low:.4f} < lr(0.618)={lr_mid:.4f} < lr(0.9)={lr_high:.4f}")
    if not (LR_MIN <= lr_low <= LR_MAX and lr_high <= LR_MAX):
        fail += 1
        print("[SEL] FAIL: clamp LR")
    else:
        print("[SEL] clamp LR OK")

    b2 = SelfCalibratorBridge()
    b2.push_observation(0.85, step=1000)
    st = b2.last_state() or {}
    if st.get("step") != 1000 or abs(float(st.get("phi_c", 0)) - 0.85) > 1e-9:
        fail += 1
        print(f"[SEL] FAIL: estado (real: {st})")
    else:
        print(f"[SEL] ponte estado OK (backend={b2.backend}, step={st['step']}, lr={float(st['lr']):.4f})")

    print(f"[SEL] {'ALL CHECKS PASS' if fail == 0 else 'CHECKS FAILED'}")
    sys.exit(1 if fail else 0)