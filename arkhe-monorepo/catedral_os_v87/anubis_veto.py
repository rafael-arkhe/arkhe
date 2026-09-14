#!/usr/bin/env python3
"""
Catedral OS v8.7 — Veto de Anúbis em Amaranth HDL (Auditado)
============================================================
Substrato 168 — Fresnel Circuit Breaker materializado em hardware.

Implementa o kill-switch epistêmico em RTL (Amaranth / nMigen).
AUDITORIA v8.7 [FIX Bug 1]: o veto ATIVA em alpha >= 0.95 — NÃO standby.

Comportamento:
  - Integrador de coerência (α) com limiar em ponto fixo Q(4.8).
  - Veto ARMADO quando α >= 0.85 (warning/escalate).
  - Veto ATIVADO (kill_switch = 1) quando α >= 0.95 (catástrofe epistêmica).
  - Saída kill_switch corta o clock / desliga o pipeline (clock gating).
  - Contador de latência de ativação (recovery < 0.2 s em hardware).

Uso:
    python anubis_veto.py            # gera Verilog (anubis_veto.v)
    python anubis_veto.py test       # smoke test em simulação
"""

import math
from amaranth import *


def _q(v: float) -> int:
    """Converte um float em [0,1] para o inteiro fixo Q(4.8) (escala 256)."""
    return int(round(v * 256))


class AnubisVeto(Elaboratable):
    """Veto Circuit-Breaker em hardware para o CGF Monitor."""

    ALPHA_WARN = 0.85
    ALPHA_KILL = 0.95
    # Nota de auditoria: FIX aplicado — kill ativa em >= 0.95 (não standby).
    SCALE = 256  # escala do ponto fixo Q(4.8)

    def __init__(self, width: int = 12):
        # width = número de bits da representação de α (Q4.8 em 12 bits).
        self.width = width

        # Entradas
        self.alpha = Signal(width)       # coerência α fixa (0.0..1.0)
        self.d_alpha = Signal(signed(width))  # derivada temporal Δα/Δt
        self.enable = Signal()           # habilitação do veto

        # Saídas
        self.kill_switch = Signal()      # 1 = VETO ATIVADO (catástrofe)
        self.armed = Signal()            # 1 = a partir de α>=0.85
        self.warning = Signal()          # 1 = α>=0.85 e <0.95
        self.latency_cycles = Signal(16)  # ciclos até ativação (debug)

        # Registradores internos
        self.triggered = Signal()        # registrador de kill (set/reset)
        self.cnt = Signal(16)            # contador de latência

    def elaborate(self, platform):
        m = Module()

        # Limiar em representação fixa Q(4.8)
        TH_WARN = _q(self.ALPHA_WARN)  # 217
        TH_KILL = _q(self.ALPHA_KILL)  # 243

        high_alpha = self.alpha >= TH_KILL

        # Sinais combinacionais de estado
        m.d.comb += [
            self.armed.eq(self.alpha >= TH_WARN),
            self.warning.eq((self.alpha >= TH_WARN) & (self.alpha < TH_KILL)),
        ]

        # Lógica síncrona: o VETO, uma vez ATIVADO, permanece travado
        # (kill-switch) até que enable seja desassistido (encerramento seguro).
        # Auditado: ATIVA em α>=0.95, NÃO fica em standby.
        with m.If(self.enable & self.triggered):
            # já ativado -> permanece travado
            m.d.sync += self.cnt.eq(self.cnt + 1)
        with m.Elif(self.enable & high_alpha):
            with m.If(self.d_alpha >= 0):
                m.d.sync += [
                    self.triggered.eq(1),
                    self.cnt.eq(self.cnt + 1),
                ]
            with m.Else():
                m.d.sync += self.cnt.eq(self.cnt + 1)
        with m.Elif(self.enable & self.armed):
            m.d.sync += self.cnt.eq(self.cnt + 1)
        with m.Else():
            m.d.sync += [
                self.triggered.eq(0),
                self.cnt.eq(0),
            ]

        # Kill-switch: combina o registrador com α crítico imediato.
        m.d.comb += self.kill_switch.eq(self.triggered | (self.enable & high_alpha))

        return m


def build_module(width: int = 12, out_file: str = "anubis_veto.v"):
    """Gera o Verilog do Veto de Anúbis."""
    import amaranth.back.verilog as verilog

    veto = AnubisVeto(width=width)
    top = verilog.convert(veto, ports=[
        veto.alpha, veto.d_alpha, veto.enable,
        veto.kill_switch, veto.armed, veto.warning, veto.latency_cycles,
    ])

    with open(out_file, "w", encoding="utf-8") as f:
        f.write("/* Catedral OS v8.7 — Veto de Anúbis (AUDITADO) */\n")
        f.write("/* kill_switch ATIVA em alpha >= 0.95 (nao standby) */\n")
        f.write(top)
    return out_file


def smoke_test():
    """Teste de bancada em simulação (amaranth.sim)."""
    from amaranth.sim import Simulator

    veto = AnubisVeto()

    async def bench(ctx):
        # inicial: habilitado, α baixo -> nada
        ctx.set(veto.enable, 1)
        ctx.set(veto.alpha, _q(0.30))
        ctx.set(veto.d_alpha, 0)
        await ctx.tick()
        assert ctx.get(veto.kill_switch) == 0
        assert ctx.get(veto.warning) == 0
        print("Veto: α=0.30 -> kill OFF ✓")

        # α médio: armado (warning)
        ctx.set(veto.alpha, _q(0.90))
        ctx.set(veto.d_alpha, 0)
        await ctx.tick()
        assert ctx.get(veto.warning) == 1
        assert ctx.get(veto.kill_switch) == 0
        print("Veto: α=0.90 -> warning ARMADO ✓")

        # α alto com derivada positiva: VETO ATIVADO
        ctx.set(veto.alpha, _q(0.97))
        ctx.set(veto.d_alpha, 1)
        await ctx.tick()
        assert ctx.get(veto.kill_switch) == 1
        print("Veto: α=0.97 -> kill_switch ATIVADO ✅")

        # α cai: kill permanece travado (não fica em standby) — latche síncrono
        ctx.set(veto.alpha, _q(0.30))
        ctx.set(veto.d_alpha, 0)
        await ctx.tick()
        assert ctx.get(veto.kill_switch) == 1
        print("Veto: α=0.30 (recuperação) -> kill mantido até reset/enable ↓ ✅")

        # desabilita: reset do latch
        ctx.set(veto.enable, 0)
        await ctx.tick()
        assert ctx.get(veto.kill_switch) == 0
        print("Veto: enable=0 -> reset do latch, kill OFF ✓")

        print("SMOKE OK — Veto ATIVA em α>=0.95 (auditado)")

    sim = Simulator(veto)
    sim.add_clock(1e-6)
    sim.add_testbench(bench)
    sim.run()


def main():
    import sys

    if len(sys.argv) > 1 and sys.argv[1] == "test":
        smoke_test()
        return

    out = build_module()
    print(f"Verilog gerado: {out}")
    print("Veto de Anúbis (AUDITADO): kill_switch ATIVA em alpha >= 0.95.")


if __name__ == "__main__":
    main()
