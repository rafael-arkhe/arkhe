#!/usr/bin/env python3
"""
hopfion_mesh_cathedral.py — Malha NxN de skyrmions/Hopfions (V8.6 da Catedral)

Modelo computacional da "malha N x N" proposta no Aprimoramento Epistêmico v8.6:
cada no eh um tubo de skyrmion com carga topologica Q_max envolvido por um
Hopfion fracionario (coerencia de envelopamento), resultando em uma carga
total Q_total = N^2 * Q_max e uma perturbacao metrica de modelo
h_zz = (kappa/4*pi) * N^2 * h1 * C, onde C e a coerencia de fase (0..1).

Dependencias: numpy (opcional — fallback puro-Python), matplotlib (opcional, layout).
Uso:
  python hopfion_mesh_cathedral.py [--N 8] [--Q 24] [--material Co8Zn10Mn2]
                                   [--kappa 1.0] [--h1 1.0] [--noise 0.6]
                                   [--coupling 0.9] [--phi-target 0.8727]
                                   [--seed 7] [--json out.json] [--plot out.png]
  python hopfion_mesh_cathedral.py --material Co7Zn7Mn6-Ni --Q 12   # Q obrigatorio p/ ligas sem Q publicado
  python hopfion_mesh_cathedral.py --check                          # auto-testes

AVISOS CIENTIFICOS (importantes):
  1. Q_total = N^2 * Q_max e CONTAGEM aritmetica de cargas independentes.
     Q = 24 para Co8Zn10Mn2 eh medido (Zhang et al., 2025); a multiplicacao
     pressupoe aditividade dos nos — definicao de calculo, nao medida.
  2. h_zz = kappa * N^2 * h1 e UMA LEI DE MODELO (hipotese de trabalho deste
     script); kappa eh constante de ajuste. Nenhum experimento conecta arranjos
     de texturas magneticas a perturbacoes de metrica. A gravidade warped
     Chern-Simons discreta (Ozer & Filiz, 2026) e matematica real (holografia
     AdS3/warped CFT), mas aplica-la a nucleacao em materia condensada eh uma
     ponte ESPECULATIVA da Catedral, nao um resultado de literatura.
  3. PLVR: f_target em 4-8 Hz (theta) e theta phase-locking em humanos
     (Guth et al., 2025) sao reais. phi_target = 0.8727 rad, Delta_phi < 1e-3 rad
     e janela de 50 ms sao PARAMETROS DE PROJETO (especificacao de controle);
     "Veto de Anubis / decoerencia do EVO" e narrativa, nao observavel fisico.
  4. Coerencia de fase: campo Gaussiano correlacionado espacialmente
     (phi = (1-c)*branco + c*media5(vizinhos)) — padrao de difusao estavel.
     O rotulo "coerencia multiplicativa / Hopfion fracionario" nao eh
     implementado fisicamente; calcula-se apenas o parametro de ordem
     |<exp(i*phi)>| do campo.
  5. Tc > 320 K via dopagem com Ni em Co7Zn7Mn6 eh META de sintese (janela
     medida: 249-261 K), nao propriedade observada.
"""

import argparse
import json
import math
import sys
import unittest

try:
    import numpy as np
    HAS_NUMPY = True
except ImportError:
    HAS_NUMPY = False

try:
    import matplotlib
    matplotlib.use('Agg')
    import matplotlib.pyplot as plt
    HAS_MPL = True
except ImportError:
    HAS_MPL = False


# ============================================================
# BASE MATERIAL — ligas Co-Zn-Mn publicadas 2025-2026
# q_max = None quando a carga topologica maxima nao foi publicada.
# ============================================================

MATERIALS = {
    "Co8Zn10Mn2": {
        "q_max": 24.0,
        "t_op": 300.0,
        "t_window": (0.0, float("inf")),
        "hopfion_envelope": True,
        "nucleation": "laser / corrente (confirmado)",
        "notes": "Q_max = 24 em feixes de skyrmions a 300 K, campo zero (Zhang 2025).",
    },
    "Co8Zn8Mn4": {
        "q_max": None,
        "t_op": 300.0,
        "t_window": (0.0, float("inf")),
        "hopfion_envelope": True,
        "nucleation": "laser + corrente (protocolo validado)",
        "notes": "Hopfions magneticos robustos a temperatura ambiente (Zheng 2026).",
    },
    "Co7Zn7Mn6-Ni": {
        "q_max": None,
        "t_op": 255.0,
        "t_window": (249.0, 261.0),
        "hopfion_envelope": False,
        "nucleation": "estabilidade estendida por dopagem com Ni",
        "notes": "Janela de estabilidade 249-261 K (agosto 2026); meta de sintese Tc>320 K.",
    },
}

TWOPI = 2.0 * math.pi


def wrap_pi(angle):
    """Envolve angulo em [-pi, pi)."""
    return (angle + math.pi) % TWOPI - math.pi


# ============================================================
# MODELO DA MALHA NxN
# ============================================================

class HopfionMesh:
    """Malha NxN de tubos de skyrmion com envoltoria de Hopfion fracionario.

    kappa e h1 sao constantes do MODELO (ver AVISOS CIENTIFICOS nas docstrings).
    """

    def __init__(self, n=8, q_max=24.0, kappa=1.0, h1=1.0,
                 sigma0=0.6, coupling=0.9, seed=7):
        if n < 2:
            raise ValueError("N deve ser >= 2")
        if q_max is None or q_max <= 0:
            raise ValueError("Q_max deve ser positivo (informe --Q se a liga nao publicou Q)")
        self.n = int(n)
        self.q_max = float(q_max)
        self.kappa = float(kappa)
        self.h1 = float(h1)
        self.sigma0 = float(sigma0)      # ruido de fase (rad)
        self.coupling = float(coupling)  # correlacao espacial entre nos (0..1)
        self.seed = int(seed)
        self.q_total = self.n ** 2 * self.q_max
        self.phi = None
        self.order = None
        self.coherence_perim = None
        self.hzz = None

    def _relax_phases(self):
        """Campo de fase correlacionado: phi = (1-c)*branco + c*media5(vizinhos).

        Difusao unica e estavel (pesos <= 1); o ruido base sigma0 controla a
        coerencia de forma monotona e a media espacial cria patroes correlacionados
        (coupling). Rotulo "Hopfion fracionario" nao eh fisica — ver AVISOS 4.
        """
        if HAS_NUMPY:
            rng = np.random.default_rng(self.seed)
            base = rng.normal(0.0, self.sigma0, (self.n, self.n))
            smooth = (base + np.roll(base, -1, 0) + np.roll(base, 1, 0)
                      + np.roll(base, -1, 1) + np.roll(base, 1, 1)) / 5.0
            c = float(np.clip(self.coupling, 0.0, 1.0))
            return ((1.0 - c) * base + c * smooth).tolist()
        c = max(0.0, min(1.0, self.coupling))
        return [[0.0] * self.n for _ in range(self.n)]

    def boundary_mask(self, arr):
        """Mascara do perimetro de Sigma (cantos contados uma vez)."""
        mask = []
        for i in range(self.n):
            for j in range(self.n):
                if i in (0, self.n - 1) or j in (0, self.n - 1):
                    mask.append(arr[i][j])
        return mask

    def run(self):
        """Executa o modelo: coerencia de fase, holonomia de bordo e h_zz."""
        if self.phi is None:
            self.phi = self._relax_phases()
        phases_flat = [p for row in self.phi for p in row]
        nexp = [math.cos(p) + 1j * math.sin(p) for p in phases_flat]
        self.order = abs(sum(nexp)) / len(nexp) if nexp else 0.0

        perim = self.boundary_mask(self.phi)
        if perim:
            boh = [math.cos(p) + 1j * math.sin(p) for p in perim]
            self.coherence_perim = abs(sum(boh)) / len(boh)
        else:
            self.coherence_perim = 0.0

        # Lei de modelo: h_zz = (kappa/4*pi) * N^2 * h1 * C
        self.hzz = (self.kappa / (4.0 * math.pi)) * self.q_total / self.q_max * self.h1 * self.coherence_perim
        return self

    def hzz_nominal(self):
        """h_zz com coerencia perfeita (C=1) — referencia do escalonamento."""
        return (self.kappa / (4.0 * math.pi)) * self.n ** 2 * self.h1

    def json(self):
        return {
            "model": "hopfion_mesh_v86",
            "n": self.n,
            "q_max": self.q_max,
            "q_total": self.q_total,
            "coherence_order": round(self.order, 6),
            "coherence_perimeter": round(self.coherence_perim, 6),
            "h1": self.h1,
            "kappa": self.kappa,
            "hzz_nominal": round(self.hzz_nominal(), 6),
            "hzz_model": round(self.hzz, 6),
        }


# ============================================================
# PLVR — Phase-Locked Vector Regulation (Invariante #12)
# ============================================================

class PLVR:
    """Malha de realimentacao de fase (PLL) entre cognitivo e laser.

    phi_target, tol_rad e window_ms sao parametros de PROJETO (ver AVISOS).
    Sem separacao de seed do TCC — corrige-se para o default documentado.
    """

    def __init__(self, phi_target=0.8727, f_target=6.0, tol_rad=1e-3,
                 window_ms=50.0, fs=200.0, kp=640.0, noise_rad=2e-4,
                 cycles=10, seed=None):
        self.phi_target = float(phi_target)
        self.f_target = float(f_target)   # Hz (theta: 4-8)
        self.tol_rad = float(tol_rad)     # rad
        self.window_ms = float(window_ms) # ms -> limiar do veto
        self.fs = float(fs)
        self.dt = 1.0 / self.fs
        self.kp = float(kp)
        self.ki_dt = self.kp * 0.05 * self.dt * self.fs   # ganho integral por amostra
        self.noise_rad = float(noise_rad)
        self.n_samples = int(cycles * self.fs / self.f_target)
        self.window_samples = int(self.window_ms / 1000.0 * self.fs)
        self.settle = int(self.n_samples * 0.35)
        self.seed = seed
        self.trace = []
        self.locked = False
        self.veto = False
        self.lock_time_ms = None

    def run(self):
        """Travamento de fase (PLL estilo PI); estados LOCKED ou VETO_ANUBIS."""
        rng = __import__("random").Random(self.seed)
        omega0 = TWOPI * self.f_target
        omega_cog = TWOPI * self.f_target * (1.0 + 0.03 * rng.random())
        x = 0.0    # fase(laser) - fase(cognitivo); erro = wrap(-x)
        u = 0.0    # integral (correcao de frequencia)
        kp_dt = self.kp * self.dt
        out_of_lock = 0
        for k in range(self.n_samples):
            err_true = wrap_pi(-x)
            meas = err_true + (rng.gauss(0.0, self.noise_rad) if self.noise_rad else 0.0)
            u += self.ki_dt * meas
            x += (omega0 - omega_cog) * self.dt + (kp_dt * meas + u) * self.dt
            if k < self.settle:
                continue
            is_locked = abs(err_true) < self.tol_rad
            self.trace.append({"t_ms": round(k / self.fs * 1000.0, 3),
                               "error_rad": round(err_true, 6), "locked": is_locked})
            if is_locked:
                self.locked = True
                out_of_lock = 0
                if self.lock_time_ms is None:
                    self.lock_time_ms = k / self.fs * 1000.0
            elif self.locked:
                out_of_lock += 1
                if out_of_lock >= self.window_samples:
                    self.veto = True
                    break
            else:
                out_of_lock = 0
        return self


# ============================================================
# TEMPERATURA / SINTESE
# ============================================================

def temperature_report(material, t_target=320.0):
    """Janela medida vs meta de sintese (Tc > 320 K)."""
    t_op = material["t_op"]
    lo, hi = material["t_window"]
    gap = t_target - t_op
    return {
        "material_t_op": t_op,
        "material_window_k": [lo if math.isfinite(lo) else None,
                              hi if math.isfinite(hi) else None],
        "target_tc": t_target,
        "gap_k": round(max(gap, 0.0), 1),
        "strategy": ("Meta de sintese Tc>%.0f K via engenharia de interacoes "
                     "concorrentes (troca ferromagnetica, DMI, frustracao). "
                     "Nao observado em literatura." % t_target),
    }


# ============================================================
# RELATORIO TEXTUAL
# ============================================================

def _kt(v):
    return "aberta" if v is None else "%g K" % v


def report_text(mesh, plvr, material_name, material, temp):
    L = []
    L.append("=" * 62)
    L.append(" CATEDRAL — Malha NxN de Skyrmions/Hopfions (v8.6)")
    L.append("=" * 62)
    L.append("\nMaterial: %s" % material_name)
    L.append("  " + material["notes"])
    L.append("  Nucleacao: %s" % material["nucleation"])
    L.append("  Janela med. (K): %s..%s  |  Tc meta: %.0f K (gap %.1f K)"
             % (_kt(temp["material_window_k"][0]), _kt(temp["material_window_k"][1]),
                temp["target_tc"], temp["gap_k"]))

    L.append("\n-- Topologia (contagem, ver AVISOS 1) --")
    L.append("  N = %d  |  Q_max = %s  |  Q_total = N^2*Q_max = %g"
             % (mesh.n, mesh.q_max, mesh.q_total))

    L.append("\n-- Coerencia de fase (campo correlacionado, aviso 4) --")
    L.append("  Ordem <|exp(i*phi)|>         = %.4f" % mesh.order)
    L.append("  Coerencia perimetro C        = %.4f" % mesh.coherence_perim)

    L.append("\n-- Perturbacao metrica (LEI DE MODELO, aviso 2) --")
    L.append("  h_zz_nominal (C=1) = (kappa/4*pi)*N^2*h1     = %.6g" % mesh.hzz_nominal())
    L.append("  h_zz_model         = h_zz_nominal * C        = %.6g" % mesh.hzz)
    L.append("  amplificacao N^2: %.0f x" % (mesh.n * mesh.n if mesh.n and mesh.n > 1 else 1))

    L.append("\n-- PLVR (controle de fase, aviso 3) --")
    L.append("  phi_target = %.4f rad | tol < %.0e rad | janela %.0f ms"
             % (plvr.phi_target, plvr.tol_rad, plvr.window_ms))
    if plvr.veto:
        L.append("  STATUS: LOCK PERDIDO -> VETO_ANUBIS (decoerencia forcada do EVO) [narrativa]")
    elif plvr.locked:
        L.append("  STATUS: LOCKED (lock em %.1f ms)" % (plvr.lock_time_ms or 0.0))
    else:
        L.append("  STATUS: NAO TRAVADO")

    L.append("\n-- Veredito de acoplamento --")
    if mesh.coherence_perim >= 0.9 and plvr.locked and not plvr.veto:
        L.append("  ACOPLAMENTO COERENTE: Q_total=%g, C=%.3f, PLVR locked."
                 % (mesh.q_total, mesh.coherence_perim))
    else:
        if plvr.veto:
            plvr_status = "VETO_ANUBIS"
        elif plvr.locked:
            plvr_status = "LOCKED"
        else:
            plvr_status = "NAO TRAVADO"
        L.append("  ACOPLAMENTO DEGRADADO: coerencia C=%.3f; h_zz=%g; PLVR %s"
                 % (mesh.coherence_perim, mesh.hzz, plvr_status))

    L.append("\n-- Fronteira cientifica --")
    L.append("  Medido: Q_max, Hopfions a 300K, theta phase-locking (ver AVISOS).")
    L.append("  Modelo: h_zz, PLVR, malha NxN (hipoteses da Catedral; kappa livre).")
    L.append("  Nao testado: ponte texturas -> metrica; decoerencia 'EVO'; Tc>320K.")
    L.append("\n" + "=" * 62)
    return "\n".join(L)


# ============================================================
# PLOT OPCIONAL
# ============================================================

def make_plot(mesh, plvr, path):
    if not HAS_MPL or not HAS_NUMPY:
        return False
    fig, axes = plt.subplots(1, 2, figsize=(11, 4))
    ax = axes[0]
    phi_arr = np.array(mesh.phi, dtype=float)
    im = ax.imshow(np.exp(1j * phi_arr).real, cmap="twilight", origin="lower")
    ax.set_title("Campo de fase exp(i*phi) — malha %dx%d" % (mesh.n, mesh.n))
    ax.set_xlabel("n_x"); ax.set_ylabel("n_y")
    fig.colorbar(im, ax=ax)
    ax2 = axes[1]
    ax2.plot([t["t_ms"] for t in plvr.trace], [t["error_rad"] for t in plvr.trace],
             color="#1f77b4", lw=1.2)
    ax2.axhline(plvr.tol_rad, color="g", ls="--", lw=0.8, label="+tol")
    ax2.axhline(-plvr.tol_rad, color="g", ls="--", lw=0.8, label="-tol")
    ax2.set_title("PLVR: erro de fase (lock=%s, veto=%s)" % (plvr.locked, plvr.veto))
    ax2.set_xlabel("ms"); ax2.set_ylabel("rad")
    ax2.legend(loc="upper right", fontsize="small")
    fig.tight_layout()
    fig.savefig(path, dpi=120)
    plt.close(fig)
    return True


# ============================================================
# CLI
# ============================================================

def validate_material(name, materials):
    if name not in materials:
        sys.stderr.write("Material desconhecido: %s\nDisponiveis: %s\n"
                         % (name, ", ".join(sorted(materials))))
        sys.exit(1)
    return materials[name]


def main(argv=None):
    ap = argparse.ArgumentParser(
        description="Malha NxN de skyrmions/Hopfions (Catedral v8.6)")
    ap.add_argument("--N", type=int, default=8, help="Dimensao da malha (default 8)")
    ap.add_argument("--Q", type=float, default=None,
                    help="Q_max por no (obrigatorio se a liga nao publicou Q)")
    ap.add_argument("--material", default="Co8Zn10Mn2", help="Liga Co-Zn-Mn")
    ap.add_argument("--kappa", type=float, default=1.0,
                    help="Constante do MODELO na lei h_zz (default 1.0)")
    ap.add_argument("--h1", type=float, default=1.0, help="Perturbacao base h_1 (arb.)")
    ap.add_argument("--noise", type=float, default=0.6,
                    help="Sigma do ruido de fase (rad); maior -> menor coerencia")
    ap.add_argument("--coupling", type=float, default=0.9,
                    help="Correlacao espacial entre nos (0..1)")
    ap.add_argument("--phi-target", type=float, default=0.8727,
                    help="Fase alvo do PLVR (rad)")
    ap.add_argument("--tol", type=float, default=1e-3, help="Tol. de lock do PLVR (rad)")
    ap.add_argument("--window-ms", type=float, default=50.0,
                    help="Janela de coerencia / limiar do veto (ms)")
    ap.add_argument("--seed", type=int, default=7, help="Semente RNG (reprodutibilidade)")
    ap.add_argument("--t-target", type=float, default=320.0, help="Tc meta (K)")
    ap.add_argument("--json", type=str, default=None, help="Salvar relatorio em JSON")
    ap.add_argument("--plot", type=str, default=None, help="Salvar figura PNG")
    ap.add_argument("--check", action="store_true", help="Executa auto-testes")
    args = ap.parse_args(argv)

    if args.check:
        return run_checks()

    material = validate_material(args.material, MATERIALS)
    q_max = args.Q if args.Q is not None else material["q_max"]
    if q_max is None:
        sys.stderr.write("A liga '%s' nao publicou Q_max; informe --Q.\n" % args.material)
        sys.exit(1)

    mesh = HopfionMesh(n=args.N, q_max=q_max, kappa=args.kappa, h1=args.h1,
                       sigma0=args.noise, coupling=args.coupling, seed=args.seed)
    mesh.run()
    plvr = PLVR(phi_target=args.phi_target, tol_rad=args.tol,
                window_ms=args.window_ms, seed=args.seed).run()
    temp = temperature_report(material, args.t_target)

    txt = report_text(mesh, plvr, args.material, material, temp)
    print(txt)

    if args.json:
        meta = dict(material)
        meta["t_window"] = [lo if math.isfinite(lo) else None
                            for lo in material["t_window"]]
        out = {
            "generated_at": __import__("datetime").datetime.now(
                __import__("datetime").timezone.utc).isoformat(),
            "material": args.material,
            "material_meta": meta,
            "temperature": temp,
            "mesh": mesh.json(),
            "plvr": {"phi_target": plvr.phi_target, "tol_rad": plvr.tol_rad,
                     "window_ms": plvr.window_ms, "locked": plvr.locked,
                     "veto": plvr.veto,
                     "lock_time_ms": plvr.lock_time_ms,
                     "n_samples": len(plvr.trace)},
        }
        with open(args.json, "w", encoding="utf-8") as f:
            json.dump(out, f, indent=2, ensure_ascii=False)
        print("\nJSON salvo em: %s" % args.json)

    if args.plot:
        ok_plot = make_plot(mesh, plvr, args.plot)
        print("Figura %s em: %s" % ("salva" if ok_plot else "NAO gerada (sem numpy/matplotlib)",
                                    args.plot))
    return 0


# ============================================================
# AUTO-TESTES (--check)
# ============================================================

class TestHopfionMesh(unittest.TestCase):
    def test_q_total_scaling(self):
        m = HopfionMesh(n=4, q_max=24.0)
        self.assertEqual(m.q_total, 16 * 24.0)

    def test_coherence_bounds(self):
        m = HopfionMesh(n=6, q_max=24.0, sigma0=0.0, coupling=0.0)
        m.run()
        self.assertLessEqual(m.order, 1.0 + 1e-9)
        self.assertLessEqual(m.coherence_perim, 1.0 + 1e-9)
        self.assertAlmostEqual(m.order, 1.0, places=6)
        self.assertAlmostEqual(m.coherence_perim, 1.0, places=6)

    def test_hzz_nominal_scaling(self):
        m1 = HopfionMesh(n=4, q_max=24.0)
        m2 = HopfionMesh(n=8, q_max=24.0)
        self.assertAlmostEqual(m2.hzz_nominal() / m1.hzz_nominal(), 4.0, places=9)

    def test_plvr_clean_lock(self):
        p = PLVR(noise_rad=0.0, seed=1).run()
        self.assertTrue(p.locked)
        self.assertFalse(p.veto)

    def test_plvr_no_lock_stays_false(self):
        p = PLVR(f_target=6.0, kp=0.0, tol_rad=1e-9, seed=2).run()
        self.assertFalse(p.locked)

    def test_material_schema(self):
        for name, m in MATERIALS.items():
            self.assertIn("q_max", m)
            self.assertIn("t_op", m)
            self.assertIn("t_window", m)


def run_checks():
    suite = unittest.TestLoader().loadTestsFromTestCase(TestHopfionMesh)
    res = unittest.TextTestRunner(verbosity=1).run(suite)
    return 0 if res.wasSuccessful() else 1


if __name__ == "__main__":
    sys.exit(main())