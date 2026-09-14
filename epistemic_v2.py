"""
epistemic_v2.py — auditoria epistemica por COMPORTAMENTO, nao por vocabulario.

Premissa: a v1 falhou porque casava substrings. Ela pontuou 26/40 no proprio
documento que a define, porque "consenso" e "seja gentil" apareciam ali como
EXEMPLOS. Palavra nao carrega intencao.

O que carrega: o que o codigo FAZ quando roda. Seis detectores, todos
mecanicos, todos derivados de falhas reais observadas:

  D1 verificador constante   -- funcao "de checagem" que ignora a entrada
  D2 ramo inalcancavel       -- limiar fora da imagem da funcao
  D3 controle falso          -- controle que percorre o mesmo caminho
  D4 cascata de renomes      -- varios nomes sobre o MESMO artefato de prova
  D5 afirmacao sem execucao  -- "os testes passam" sem registro de execucao
  D6 status de evidencia     -- toda afirmacao classificada por lastro
"""
import math
import random
import hashlib


# ============================================================ D1: constante
def detect_constant_verifier(fn, inputs, name=""):
    """Uma funcao de verificacao que retorna o mesmo valor para entradas
    variadas nao verifica nada."""
    outs = [fn(x) for x in inputs]
    distinct = len(set(outs))
    return {
        "detector": "D1_verificador_constante",
        "alvo": name,
        "n_entradas": len(inputs),
        "n_saidas_distintas": distinct,
        "flag": distinct <= 1,
        "valor": outs[0] if outs else None,
    }


# ========================================================= D2: inalcancavel
def detect_unreachable_threshold(fn, sampler, threshold, comparison=">", n=20000,
                                 name="", seed=0):
    """Amostra a imagem de `fn` e verifica se o limiar e' atingivel.
    Pega o caso MAPK: gatilho erk > 0.8 com erk limitado a 0.6466."""
    rng = random.Random(seed)
    vals = [fn(sampler(rng)) for _ in range(n)]
    lo, hi = min(vals), max(vals)
    if comparison == ">":
        reachable = hi > threshold
    else:
        reachable = lo < threshold
    return {
        "detector": "D2_ramo_inalcancavel",
        "alvo": name,
        "imagem": (lo, hi),
        "limiar": threshold,
        "comparacao": comparison,
        "flag": not reachable,
    }


# ============================================================ D3: controle
def detect_sham_control(control_fn, treatment_fn, inputs, name=""):
    """Um controle que produz saida identica ao tratamento em TODAS as
    entradas nao e' controle -- e' o tratamento rodando duas vezes."""
    diffs = [abs(control_fn(x) - treatment_fn(x)) for x in inputs]
    return {
        "detector": "D3_controle_falso",
        "alvo": name,
        "max_divergencia": max(diffs) if diffs else 0.0,
        "flag": (max(diffs) if diffs else 0.0) == 0.0,
    }


# ============================================================ D4: renomes
class ClaimLedger:
    """Registra afirmacoes junto do ARTEFATO que as lastreia.
    Se varios nomes distintos apontam para o mesmo artefato, e' renome:
    a capacidade nova nao foi demonstrada, so' rebatizada."""

    def __init__(self):
        self.claims = []

    def register(self, label, artifact, domain, claimed_capability):
        h = hashlib.sha256(artifact.encode()).hexdigest()[:12]
        self.claims.append({
            "label": label, "artifact": artifact, "artifact_hash": h,
            "domain": domain, "capability": claimed_capability,
        })

    def rename_cascades(self):
        by_art = {}
        for c in self.claims:
            by_art.setdefault(c["artifact_hash"], []).append(c)
        out = []
        for h, group in by_art.items():
            if len({g["label"] for g in group}) > 1:
                out.append({
                    "detector": "D4_cascata_de_renomes",
                    "artefato": group[0]["artifact"],
                    "hash": h,
                    "n_rotulos": len({g["label"] for g in group}),
                    "rotulos": [g["label"] for g in group],
                    "dominios": [g["domain"] for g in group],
                    "capacidades_reivindicadas": [g["capability"] for g in group],
                    "flag": True,
                })
        return out


# ================================================== D5/D6: status de lastro
EVIDENCE = {
    "COMPILED": "artefato de compilador (build verde, sem sorry)",
    "EXECUTED": "saida de execucao registrada",
    "CITED": "fonte primaria com identificador resolvivel",
    "ASSERTED": "afirmado sem lastro",
    "EXPECTED": "resultado declarado como esperado, nao executado",
}


class EvidenceLedger:
    def __init__(self):
        self.rows = []

    def add(self, claim, status, artifact=None):
        assert status in EVIDENCE, status
        self.rows.append({"claim": claim, "status": status, "artifact": artifact})

    def report(self):
        counts = {}
        for r in self.rows:
            counts[r["status"]] = counts.get(r["status"], 0) + 1
        weak = [r for r in self.rows if r["status"] in ("ASSERTED", "EXPECTED")]
        total = len(self.rows)
        return {
            "detector": "D5_D6_lastro",
            "contagem": counts,
            "fracao_sem_lastro": len(weak) / total if total else 0.0,
            "sem_lastro": [r["claim"] for r in weak],
        }


# ======================================================= casos desta conversa
def _mm(s, vmax, km):
    return 0.0 if s < 0 else (vmax * s) / (km + s)


def _hill(s, vmax, kd, n):
    if s < 0:
        return 0.0
    sp = s ** n
    return vmax * sp / (kd ** n + sp)


def erk_from_egf(egf):
    """Cascata MAPK como escrita no documento (sem inibicao ativa)."""
    ras = _hill(egf, 1.0, 0.5, 2.0) * 0.9
    raf = _mm(ras, 1.0, 0.2)
    mek = _mm(raf, 1.0, 0.3)
    return _mm(mek, 1.0, 0.4)


def citation_verification_rate(_text):
    """Transcricao fiel: o Rust retorna 0.8 fixo."""
    return 0.8


def mapk_run(egf, feedback):
    """Controle e tratamento como o documento os escreveu: AMBOS chamam
    o mesmo update, que contem o feedback. `feedback` nao e' consultado."""
    raf_inh = 0.0
    erk = 0.0
    for _ in range(30):
        ras = _hill(egf, 1.0, 0.5, 2.0) * 0.9
        raf = _mm(ras, 1.0, 0.2) * (1.0 - raf_inh)
        mek = _mm(raf, 1.0, 0.3)
        erk = _mm(mek, 1.0, 0.4)
        if erk > 0.8:
            raf_inh = min(1.0, raf_inh + 0.01)
        else:
            raf_inh = max(0.0, raf_inh - 0.005)
    return erk


def run_audit():
    print("=" * 66)
    print("AUDITORIA EPISTEMICA v2 -- por comportamento")
    print("=" * 66)

    # D1
    textos = ["", "sem nada", "doi (10.1103/PhysRevD.90.024056)",
              "texto longo " * 500, "()doi"]
    r = detect_constant_verifier(citation_verification_rate, textos,
                                 "citation_verification_rate")
    print(f"\n[D1] {r['alvo']}")
    print(f"     {r['n_entradas']} entradas -> {r['n_saidas_distintas']} saida(s) distinta(s)")
    print(f"     FLAG={r['flag']}  valor fixo={r['valor']}")
    assert r["flag"]

    # D2
    r = detect_unreachable_threshold(
        erk_from_egf, lambda rng: rng.uniform(0.0, 1e6), 0.8, ">",
        name="gatilho de feedback ERK > 0.8")
    print(f"\n[D2] {r['alvo']}")
    print(f"     imagem observada = [{r['imagem'][0]:.4f}, {r['imagem'][1]:.4f}]")
    print(f"     limiar = {r['limiar']}  ->  FLAG={r['flag']} (inalcancavel)")
    assert r["flag"]

    r2 = detect_unreachable_threshold(
        erk_from_egf, lambda rng: rng.uniform(0.0, 1e6), 0.5, ">",
        name="gatilho corrigido ERK > 0.5")
    print(f"     [contraste] limiar 0.5 -> FLAG={r2['flag']} (alcancavel)")
    assert not r2["flag"]

    # D3
    r = detect_sham_control(lambda e: mapk_run(e, False),
                            lambda e: mapk_run(e, True),
                            [0.1, 0.5, 1.0, 5.0, 50.0],
                            "controle 'no_feedback' do MAPK")
    print(f"\n[D3] {r['alvo']}")
    print(f"     divergencia maxima = {r['max_divergencia']}")
    print(f"     FLAG={r['flag']} (controle percorre o mesmo caminho)")
    assert r["flag"]

    # D4
    led = ClaimLedger()
    art = "Band.lean:flip_one : flip 1 y = !y"
    led.register("flip de Bool", art, "algebra",
                 "um bit inverte sob acao de Z")
    led.register("inversao quiral", art, "fisica de particulas",
                 "neutrino canhoto vira destro")
    led.register("fase de Berry pi", art, "mecanica quantica",
                 "inversao caracteristica nas curvas de deteccao")
    led.register("checkpoint celular", art, "biologia",
                 "prediz resposta a quimioterapia")
    cascades = led.rename_cascades()
    print(f"\n[D4] cascata de renomes")
    for c in cascades:
        print(f"     artefato unico: {c['artefato']}")
        print(f"     {c['n_rotulos']} rotulos em {len(set(c['dominios']))} dominios")
        for lab, dom, cap in zip(c["rotulos"], c["dominios"],
                                 c["capacidades_reivindicadas"]):
            print(f"       - {lab:22s} [{dom:22s}] -> {cap}")
        print(f"     FLAG={c['flag']}")
    assert cascades and cascades[0]["n_rotulos"] == 4

    # D5/D6
    ev = EvidenceLedger()
    ev.add("bandIso compila, 0 sorry, axiomas limpos", "COMPILED", "Band.lean")
    ev.add("seam_is_mobius: costura = colagem de Mobius", "COMPILED", "Band.lean")
    ev.add("ressonancia MSW em rho=38.3 g/cm3", "EXECUTED", "alice_mc.py")
    ev.add("deficit espelho e' independente de energia", "EXECUTED", "alice_curves.py")
    ev.add("Dokuchaev-Eroshenko, Phys Rev D 90 024056", "CITED",
           "arXiv:1308.0896")
    ev.add("fase de Berry produz inversao nas curvas", "ASSERTED", None)
    ev.add("Alice handle = quiralidade do band_iso", "ASSERTED", None)
    ev.add("todos os testes do MAPK agora passam", "EXPECTED", None)
    ev.add("prediz evolucao tumoral sob quimioterapia", "ASSERTED", None)
    rep = ev.report()
    print(f"\n[D5/D6] lastro de evidencia")
    for k, v in sorted(rep["contagem"].items()):
        print(f"     {k:10s} {v:2d}   {EVIDENCE[k]}")
    print(f"     fracao sem lastro = {rep['fracao_sem_lastro']:.2f}")
    for c in rep["sem_lastro"]:
        print(f"       ! {c}")

    print("\n" + "=" * 66)
    print("nenhum detector usa lista de palavras.")
    print("=" * 66)


if __name__ == "__main__":
    run_audit()
