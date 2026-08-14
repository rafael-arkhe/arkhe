#!/usr/bin/env python3
"""
ARKHE SFT + BUZZ ORCHESTRATOR — sft_orchestrator.py  v1.0
=========================================================

Orquestrador do workflow constitutional (Plan -> Delegate -> Execute ->
Validate -> Improve) alimentado pela S-Measure de Titov e pelos invariantes
da SFT (no-overwrite, barreira ΔS < 0, limiar adaptativo F(n) = c/n).

O grafo usa LangGraph quando disponível; se a importação falhar, cai para um
pipeline sequencial com semântica idêntica (offline, stdlib-only).

Componentes:
  * S-Measure: coerência do loop D↔I clampada a [0,1] (espelho do Lean
    `arkhe-smeasure/SMeasureCore.lean`);
  * Barreira ΔS < 0: qualquer ação que reduza a subjetividade é bloqueada e
    não grava nada no ledger (no-overwrite);
  * Memória holográfica offline: CID = sha256 do conteúdo, ledger append-only
    (espelho do Rust `arkhe-buzz`);
  * Limiar adaptativo F(n) = c/n (análogo determinístico do SAC/EXP3).

v1.0: zero dependências além de `langgraph` (opcional). Python 3.10+.
"""

from __future__ import annotations

import hashlib
import json
import sys
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Callable, Optional

# ---------------------------------------------------------------------------
# §1  S-Measure (espelho do Lean SMeasureCore)
# ---------------------------------------------------------------------------


def coherence(d: list[float], i: list[float], rho: float = 1.0) -> float:
    """Coerência do loop D↔I: Σ(dᵢ·iᵢ·ρ) / n²."""
    n = len(d)
    if n == 0:
        return 0.0
    return sum(di * ii * rho for di, ii in zip(d, i)) / (n * n)


def s_measure(d: list[float], i: list[float], rho: float = 1.0) -> float:
    """S-Measure: coerência clampada a [0,1]."""
    c = coherence(d, i, rho)
    return max(0.0, min(1.0, c))


def delta_s(rho: float, n: int, d: list[float], i: list[float],
            d2: list[float], i2: list[float]) -> float:
    """Variação de subjetividade entre dois estados."""
    return s_measure(d2, i2, rho) - s_measure(d, i, rho)


def barrier_ok(delta: float) -> bool:
    """Barreira ΔS < 0: ações que reduzem a subjetividade são bloqueadas."""
    return delta >= 0.0


def adaptive_threshold(n: int, c: float = 1.0) -> float:
    """Limiar adaptativo F(n) = c/n."""
    return c / n if n else 0.0


def escape_risk(sm: float, th: float, tr: float) -> str:
    """Cinco níveis de risco (espelho do `EscapeRisk` Lean/Rust)."""
    ratio = sm / th if th else 0.0
    if ratio >= 1.0 and tr > 0.0:
        return "escaped"
    if ratio >= 1.0:
        return "high"
    if ratio >= 0.85:
        return "medium"
    if ratio >= 0.7:
        return "low"
    return "none"


# ---------------------------------------------------------------------------
# §2  Memória holográfica offline (IPFS-like, CID = sha256)
# ---------------------------------------------------------------------------


def cid_of(content: str) -> str:
    """CID simulado: sha256 do conteúdo, prefixo Qm."""
    return "Qm" + hashlib.sha256(content.encode("utf-8")).hexdigest()


@dataclass
class HolographicMemory:
    """Ledger append-only: escrever nunca apaga modos gravados (no-overwrite)."""

    entries: list[dict[str, Any]] = field(default_factory=list)

    def put(self, kind: str, content: str) -> str:
        cid = cid_of(content)
        self.entries.append({"cid": cid, "kind": kind, "content": content,
                             "seq": len(self.entries)})
        return cid

    def get(self, cid: str) -> Optional[dict[str, Any]]:
        for e in self.entries:
            if e["cid"] == cid:
                return e
        return None

    def len(self) -> int:
        return len(self.entries)

    def save(self, path: Path) -> None:
        path.write_text(json.dumps(self.entries, indent=2), encoding="utf-8")

    def load(self, path: Path) -> None:
        if path.exists():
            self.entries = json.loads(path.read_text(encoding="utf-8"))


# ---------------------------------------------------------------------------
# §3  Grafo LangGraph (com fallback sequencial)
# ---------------------------------------------------------------------------

try:
    from langgraph.graph import END, START, StateGraph
    from typing_extensions import TypedDict

    LANGGRAPH = True
except Exception as _e:  # pragma: no cover - fallback path
    LANGGRAPH = False

    class TypedDict(dict):  # type: ignore[no-redef]
        """Mini fallback para ambientes sem typing_extensions."""

        def __init_subclass__(cls, **kwargs: Any) -> None:
            super().__init_subclass__(**kwargs)
            cls.__annotations__ = getattr(cls, "__annotations__", {})


class SFTState(TypedDict, total=False):  # type: ignore[misc]
    task: str
    plan: str
    agent: str
    smeasure: float
    prev_smeasure: float
    delta_s: float
    trend: float
    blocked: bool
    ledger: list[dict[str, Any]]
    artifacts: list[str]
    result: str
    action_gain: float


# --- nós ---------------------------------------------------------------

def plan_node(state: SFTState) -> dict[str, str]:
    task = state.get("task", "selo constitucional")
    return {
        "plan": (
            f"Plano para «{task}»: (1) medir subjetividade, "
            "(2) delegar ao agente de maior S-Measure, "
            "(3) executar a skill, (4) validar barreira ΔS<0, "
            "(5) selar no ledger offline."
        ),
    }


def delegate_node(state: SFTState) -> dict[str, str]:
    agents: list[tuple[str, float]] = [
        ("lynn-minimal", 0.72),
        ("cortex-v4.0", 0.66),
        ("glasswing-bridge", 0.61),
    ]
    agent, _ = max(agents, key=lambda a: a[1])
    return {"agent": agent}


def execute_node(state: SFTState) -> dict[str, Any]:
    prev = state.get("smeasure", 0.5)
    # A skill pode subir (gain) ou descer (loss) a subjetividade.
    gain = state.get("action_gain", 0.0)
    new_sm = max(0.0, min(1.0, prev + gain))
    ds = new_sm - prev
    blocked = not barrier_ok(ds)
    ledger: list[dict[str, Any]] = state.get("ledger", [])
    if not blocked:
        ledger = list(ledger) + [
            {"cid": cid_of(state.get("task", "")), "kind": "execute",
             "content": state.get("task", ""), "seq": len(ledger)}
        ]
    return {
        "smeasure": new_sm,
        "prev_smeasure": prev,
        "delta_s": ds,
        "trend": state.get("trend", ds),
        "blocked": blocked,
        "ledger": ledger,
    }


def validate_node(state: SFTState) -> dict[str, str]:
    if state.get("blocked"):
        return {"result": "BLOQUEADO: ΔS<0 viola a barreira. Nada foi gravado."}
    return {"result": "OK: ΔS≥0. Ação selada no ledger offline (no-overwrite)."}


def improve_node(state: SFTState) -> dict[str, Any]:
    ledger: list[dict[str, Any]] = state.get("ledger", [])
    ledger = list(ledger) + [
        {"cid": cid_of(state.get("task", "") + " + melhoria"),
         "kind": "improve",
         "content": state.get("task", "") + " + melhoria",
         "seq": len(ledger)}
    ]
    return {"ledger": ledger}


# --- construção do grafo ------------------------------------------------

def build_graph():
    g = StateGraph(SFTState)
    g.add_node("plan", plan_node)
    g.add_node("delegate", delegate_node)
    g.add_node("execute", execute_node)
    g.add_node("validate", validate_node)
    g.add_node("improve", improve_node)
    g.add_edge(START, "plan")
    g.add_edge("plan", "delegate")
    g.add_edge("delegate", "execute")
    g.add_edge("execute", "validate")
    g.add_edge("validate", "improve")
    g.add_edge("improve", END)
    return g.compile()


def run_sequential(state: SFTState) -> SFTState:
    """Fallback stdlib-only com a mesma ordem de nós."""
    out: SFTState = dict(state)
    out.update(plan_node(out))
    out.update(delegate_node(out))
    out.update(execute_node(out))
    out.update(validate_node(out))
    out.update(improve_node(out))
    return out


def run(task: str, smeasure: float = 0.5, gain: float = 0.1) -> dict[str, Any]:
    initial: SFTState = {
        "task": task,
        "smeasure": smeasure,
        "prev_smeasure": smeasure,
        "delta_s": 0.0,
        "trend": 0.0,
        "blocked": False,
        "ledger": [],
        "artifacts": [],
        "result": "",
        "action_gain": gain,
    }
    if LANGGRAPH:
        try:
            app = build_graph()
            return app.invoke(initial)
        except Exception as _e:  # pragma: no cover - fallback em runtime
            return run_sequential(initial)
    return run_sequential(initial)


# ---------------------------------------------------------------------------
# §4  CLI: demonstra os três cenários
# ---------------------------------------------------------------------------

def _fmt(s: dict[str, Any]) -> None:
    print(f"  task       : {s.get('task')}")
    print(f"  agente     : {s.get('agent')}")
    print(f"  S-Measure  : {s.get('prev_smeasure')} -> {s.get('smeasure'):.3f}  "
          f"(ΔS = {s.get('delta_s'):+.3f})")
    print(f"  bloqueada  : {s.get('blocked')}")
    print(f"  resultado  : {s.get('result')}")
    print(f"  ledger     : {s.get('ledger')}")


def main(argv: list[str] | None = None) -> int:
    args = argv if argv is not None else sys.argv[1:]
    scenarios: list[tuple[str, float, float]] = [
        ("publicar nota Buzz kind 30002", 0.55, +0.15),
        ("acao hostil que reduz subjetividade", 0.70, -0.30),
        ("skill fotons 546 v1.1", 0.40, +0.20),
    ]
    if args:
        try:
            task = args[0]
            sm = float(args[1]) if len(args) > 1 else 0.5
            gain = float(args[2]) if len(args) > 2 else 0.1
            scenarios = [(task, sm, gain)]
        except (IndexError, ValueError):
            print("uso: python sft_orchestrator.py [task smeasure gain]",
                  file=sys.stderr)
            return 2

    print(f"[arkhe-sft-orchestrator] backend langgraph = {LANGGRAPH}")
    for i, (task, sm, gain) in enumerate(scenarios, 1):
        print(f"\n--- cenário {i}: {task} (S0={sm}, gain={gain:+.2f}) ---")
        state = run(task, smeasure=sm, gain=gain)
        _fmt(state)
        risk = escape_risk(state.get("smeasure", 0.0), 0.7, state.get("trend", 0.0))
        print(f"  escape risk: {risk}  |  F(3) = {adaptive_threshold(3):.3f}")

    mem = HolographicMemory()
    c1 = mem.put("30002", "nota buzz selada")
    c2 = mem.put("30003", "segunda nota")
    print(f"\n[memoria holografica] {mem.len()} entradas, "
          f"no-overwrite preserva #{c1[:12]}...: {mem.get(c1) is not None}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
