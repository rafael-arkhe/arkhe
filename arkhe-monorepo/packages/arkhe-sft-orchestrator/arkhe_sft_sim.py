#!/usr/bin/env python3
"""
ARKHE SFT SIMULATOR — arkhe_sft_sim.py  v1.0
=============================================

Verificação numérica das fórmulas da Substrate Fission Theory (SFT)
formalizadas em `arkhe-sft/SFTCore.lean` e no crate `arkhe-buzz`:

  * r_spin  = hbar / (2·m·c)            (raio de spin)
  * lambda  = 4π·r_spin                 (comprimento de onda = λ_C)
  * nu_C    = m·c² / hbar               (frequência de Compton)
  * r_S     = 2·g·m / c²                (raio de Schwarzschild)
  * C_A     = π·r_spin²                 (área de captura)
  * A_P     = l_P²                      (área de Planck)
  * critério: 4·g·m² < hbar·c  ⇒  r_S < r_spin  (não-colisão)
  * hierarquia: massa maior ⇒ λ menor e ν maior
  * exatamente nove modos de vibração

Stdlib-only (offline). Python 3.10+ (testado em 3.14).
"""

from __future__ import annotations

import math
import sys

HBAR = 1.054571817e-34
C_LIGHT = 2.99792458e8
G_NEWTON = 6.67430e-11
PLANCK_LENGTH = 1.616255e-35
MODE_COUNT = 9

ELECTRON_MASS = 9.10938356e-31
PROTON_MASS = 1.672621898e-27


def r_spin(m: float) -> float:
    return HBAR / (2.0 * m * C_LIGHT)


def wavelength(m: float) -> float:
    return 4.0 * math.pi * r_spin(m)


def compton_frequency(m: float) -> float:
    return m * C_LIGHT ** 2 / HBAR


def schwarzschild_radius(m: float) -> float:
    return 2.0 * G_NEWTON * m / C_LIGHT ** 2


def capture_area(m: float) -> float:
    return math.pi * r_spin(m) ** 2


def planck_area() -> float:
    return PLANCK_LENGTH ** 2


def is_bh_excluded(m: float) -> bool:
    return 4.0 * G_NEWTON * m ** 2 < HBAR * C_LIGHT


def modes(m: float) -> list[float]:
    """Escada harmónica da frequência de Compton: f_j = (j+1)·ν_C."""
    nu = compton_frequency(m)
    return [(j + 1) * nu for j in range(MODE_COUNT)]


def check(label: str, cond: bool) -> bool:
    status = "PASS" if cond else "FAIL"
    print(f"  [{status}] {label}")
    return cond


def verify_electron() -> bool:
    """Valores esperados para o eletrão (m = 9.109e-31 kg)."""
    m = ELECTRON_MASS
    ok = True
    r = r_spin(m)
    lam = wavelength(m)
    nu = compton_frequency(m)
    rs = schwarzschild_radius(m)
    print(f"  eletrão: r_spin = {r:.4e} m | λ = {lam:.4e} m "
          f"| ν_C = {nu:.4e} Hz")
    print(f"           r_S = {rs:.4e} m | C_A = {capture_area(m):.4e} m² "
          f"| A_P = {planck_area():.4e} m²")
    ok &= check("r_spin > 0", r > 0.0)
    ok &= check("λ > 0", lam > 0.0)
    ok &= check("ν_C > 0", nu > 0.0)
    ok &= check("r_S > 0", rs > 0.0)
    ok &= check("C_A > 0", capture_area(m) > 0.0)
    ok &= check("A_P > 0", planck_area() > 0.0)
    # λ = 4π·r_spin = h/(m·c) → 4π·r_spin·m·c/hbar = 2π
    ok &= check("4π·r_spin·m·c = 2·hbar·π (identidade λ)",
                math.isclose(4.0 * math.pi * r * m * C_LIGHT,
                             2.0 * HBAR * math.pi, rel_tol=1e-12))
    # r_spin·(2·m·c) = hbar  (q_rspin_relation do Lean)
    ok &= check("r_spin·2·m·c = hbar (relação fundamental)",
                math.isclose(r * (2.0 * m * C_LIGHT), HBAR, rel_tol=1e-12))
    ok &= check("4·g·m² < hbar·c ⇒ r_S < r_spin (não colapsa)",
                is_bh_excluded(m) and rs < r)
    return ok


def verify_mass_hierarchy() -> bool:
    """Hierarquia de massa: massa maior ⇒ λ menor e ν maior."""
    lam_e = wavelength(ELECTRON_MASS)
    lam_p = wavelength(PROTON_MASS)
    nu_e = compton_frequency(ELECTRON_MASS)
    nu_p = compton_frequency(PROTON_MASS)
    ok = check(f"λ(protão) < λ(eletrão): {lam_p:.3e} < {lam_e:.3e}",
               lam_p < lam_e)
    ok &= check(f"ν(protão) > ν(eletrão): {nu_p:.3e} > {nu_e:.3e}",
                nu_p > nu_e)
    return ok


def verify_nine_modes() -> bool:
    ms = modes(ELECTRON_MASS)
    ok = check(f"exatamente {len(ms)} modos (Fin 9)",
               len(ms) == MODE_COUNT == 9)
    ok &= check("modos distintos entre si",
                len(set(ms)) == MODE_COUNT)
    return ok


def verify_buzz_ledger() -> bool:
    """No-overwrite: o ledger append-only preserva escritas anteriores."""
    from sft_orchestrator import HolographicMemory, cid_of
    mem = HolographicMemory()
    first = mem.put("30002", "estado A")
    mem.put("30003", "estado B")
    ok = check("duas escritas ⇒ duas entradas (append-only)",
               mem.len() == 2)
    ok &= check("escrita antiga preservada pelo CID (no-overwrite)",
                mem.get(first) is not None and mem.get(first)["content"] == "estado A")
    ok &= check("CID determinístico (sha256)",
                cid_of("estado A") == first)
    return ok


def main(argv: list[str] | None = None) -> int:
    args = argv if argv is not None else sys.argv[1:]
    only = set(args)
    tests = [
        ("eletrão / fotões / buraco negro", verify_electron, "electron"),
        ("hierarquia de massa", verify_mass_hierarchy, "hierarchy"),
        ("nove modos", verify_nine_modes, "modes"),
        ("ledger holográfico (no-overwrite)", verify_buzz_ledger, "ledger"),
    ]
    print("[arkhe-sft-sim] verificação numérica das fórmulas SFT\n")
    results: list[bool] = []
    for label, fn, key in tests:
        if only and key not in only:
            continue
        print(f"--- {label} ---")
        results.append(fn())
        print()
    ok = all(results)
    print(f"RESULTADO GLOBAL: {'PASS' if ok else 'FAIL'} "
          f"({sum(results)}/{len(results)} grupos)")
    return 0 if ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
