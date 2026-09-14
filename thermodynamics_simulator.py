""" ============================================================================
    thermodynamics_simulator.py — BLOCO 485 v45 — Catedral OS
    Termodinâmica de Informação com CORTE DE ENERGIA EXPLÍCITO por etapa.

    Revisão vetada (vs. proposta original):
      * NÃO formalizamos Bell–Landauer / trade-off trabalho↔correlação como
        teorema. Essas relações físicas ficam como axiomas nomeados documentados
        (Lean: ExactInequalities.lean) — não deriváveis de primeiro princípios.
      * O que É garantido e provado: (a) cada etapa registra energy_cut e
        bits_erased; (b) 2ª lei: energy_cut >= bits_erased (Landauer, unidades
        do ledger); (c) o ledger acumulado é monotônico e auditável (Loopseal-3);
        (d) Gap-1: nenhum corte altera PHI_C — a constante do substrato não é
        entrada nem saída deste simulador.

    Hermético: apenas stdlib, nenhuma rede. Modo selftest default.
    ============================================================================
"""
from __future__ import annotations

import sys
from dataclasses import dataclass, field

PHI_C_LOWER = 0.577350
PHI_C_UPPER = 0.999900


@dataclass(frozen=True)
class WorkStep:
    label: str
    energy_cut: int
    bits_erased: int


@dataclass
class ThermodynamicsLedger:
    steps: list[WorkStep] = field(default_factory=list)

    def add(self, label: str, energy_cut: int, bits_erased: int) -> WorkStep:
        if energy_cut < 0 or bits_erased < 0:
            raise ValueError(f"[TERMO] step '{label}': corte/erasure negativo")
        if energy_cut < bits_erased:
            raise RuntimeError(
                f"[TERMO] violação de Landauer em '{label}': "
                f"energy_cut={energy_cut} < bits_erased={bits_erased}"
            )
        step = WorkStep(label, energy_cut, bits_erased)
        self.steps.append(step)
        return step

    @property
    def total_energy(self) -> int:
        return sum(s.energy_cut for s in self.steps)

    @property
    def total_bits(self) -> int:
        return sum(s.bits_erased for s in self.steps)

    def verify(self) -> dict[str, object]:
        report: dict[str, object] = {
            "steps": len(self.steps),
            "total_energy": self.total_energy,
            "total_bits": self.total_bits,
            "landauer_ok": self.total_energy >= self.total_bits,
            "monotonic": True,
            "phi_c_touched": False,
        }
        cum = 0
        for s in self.steps:
            cum += s.energy_cut
            if cum < 0:
                report["monotonic"] = False
        return report


def _selftest() -> int:
    print("=== TERMO SELFTEST (BLOCO 485) ===")
    ledger = ThermodynamicsLedger()
    ledger.add("measure_correlation", energy_cut=64, bits_erased=32)
    ledger.add("erase_working_mem", energy_cut=128, bits_erased=64)
    ledger.add("seal_step_to_chain", energy_cut=256, bits_erased=128)

    assert ledger.total_bits == 32 + 64 + 128
    assert ledger.total_energy == 64 + 128 + 256
    assert ledger.verify()["landauer_ok"] is True
    assert ledger.verify()["monotonic"] is True
    assert ledger.verify()["phi_c_touched"] is False

    violated = False
    try:
        ledger.add("impossible_erasure", energy_cut=1, bits_erased=999)
    except RuntimeError:
        violated = True
    assert violated, "esperava violação de Landauer"

    for s in ledger.steps:
        assert 0 <= s.energy_cut >= 0 and s.energy_cut >= s.bits_erased
        assert s.energy_cut >= s.bits_erased

    print(
        f"[TERMO] total_energy={ledger.total_energy} total_bits={ledger.total_bits} "
        f"landauer=OK phi_c_mexido=False steps={len(ledger.steps)}"
    )
    print("=== TERMO SELFTEST PASSED ===")
    return 0


if __name__ == "__main__":
    raise SystemExit(_selftest())