#!/usr/bin/env python3
"""
Substrato 264 v2 — Peer Review Engine (port Python testável)

Espelha fielmente a lógica do `peer_review_v2.pl` (versão corrigida por
análise estática). Permite testar em Python a MESMA semântica das 12
categorias de problemas, 5 recomendações, qualidade de parecer (4 dimensões),
métricas de pareceristas, ética (COPE/FAPESP/ICC-ESOMAR/GDPR) e a integração
conceitual com os substratos 262 (RSI) e 263 (personas sintéticas).

NOTA DE AUDITORIA (honestidade):
 - Este módulo é um PORT de verificação; o sistema canônico é o arquivo
   Prolog. Nenhum fluxo editorial real, nenhuma comunicação em rede e
   nenhuma simulação de humanos foi executada.
 - As heurísticas de detecção são por substring (como no Prolog) e, portanto,
   aproximam — não provam — a presença de um problema.
 - As dimensões de qualidade do parecer usam aleatoriedade determinística
   (seed fixa), tornando os testes reprodutíveis.
"""

from __future__ import annotations

import hashlib
import json
import random
import time
import uuid
from dataclasses import dataclass, field
from typing import Any, Dict, List, Optional, Tuple

# ============================================================================
# Severidades (espelham problem_severity/2 do Prolog)
# ============================================================================
SEVERITY: Dict[str, int] = {
    # Estruturais
    "methodological_flaw": 10,
    "insufficient_data": 9,
    "unclear_contribution": 9,
    "unclear_objectives": 8,
    # Argumentativos
    "internal_incoherence": 7,
    "external_incoherence": 7,
    "poor_contextualization": 6,
    # Teóricos
    "weak_theory": 6,
    "poor_lit_review": 5,
    "concept_misuse": 5,
    # Estilo/Forma
    "style_issues": 3,
    "poor_title_abstract": 3,
    "superficial_analysis": 4,
    # Éticos (crítico)
    "plagiarism": 10,
    "fabricated_data": 10,
}

DECISION_TYPE = ["approve", "minor_revision", "major_revision",
                 "reject_resubmit", "reject"]

REVIEW_DIMENSIONS = ["summary", "qualities", "insufficiencies", "potentialities"]


# ============================================================================
# Detecção de problemas (espelha detect_problem/4 do Prolog)
# ============================================================================
def _has(text: str, *needles: str) -> bool:
    t = (text or "").lower()
    return any(n in t for n in needles)


def detect_problems(abstract: str, text: str) -> List[str]:
    """Retorna a lista de problemas detectados por heurística de substring."""
    problems: List[str] = []

    # Estruturais
    if not (_has(text, "metodologia") or _has(text, "método")):
        problems.append("methodological_flaw")
    if not (_has(text, "dados") or _has(text, "amostra")):
        problems.append("insufficient_data")
    if not (_has(abstract, "contribui") or _has(abstract, "avança")):
        problems.append("unclear_contribution")
    if not (_has(abstract, "objetivo") or _has(abstract, "pergunta")):
        problems.append("unclear_objectives")

    # Argumentativos
    if _has(text, "porém") or _has(text, "contradiz"):
        problems.append("internal_incoherence")
    if _has(text, "contraria") or _has(text, "divergente"):
        problems.append("external_incoherence")
    if not (_has(text, "contexto") or _has(text, "histórico")):
        problems.append("poor_contextualization")

    # Teóricos
    if not (_has(text, "teoria") or _has(text, "framework")):
        problems.append("weak_theory")
    if not (_has(text, "bibliografia") or _has(text, "referências")):
        problems.append("poor_lit_review")
    if _has(text, "equivocado") or _has(text, "impreciso"):
        problems.append("concept_misuse")

    # Estilo/Forma
    if _has(text, "jargão") or _has(text, "prolixo"):
        problems.append("style_issues")
    if len((abstract or "")) < 50:
        problems.append("poor_title_abstract")

    # Éticos
    if _has(text, "plágio") or _has(text, "copiado"):
        problems.append("plagiarism")
    if _has(text, "fabricado") or _has(text, "inventado"):
        problems.append("fabricated_data")

    return problems


def compute_scores(problems: List[str]) -> Tuple[List[float], float, float]:
    """(scores, overall, weighted) — espelha compute_scores/4 do Prolog.

    CORREÇÃO (documentada na auditoria v58.0):
      O rascunho usava `Weighted = mean(((10-S)/10) * (S/10))`, cujo máximo
      matemático é 0.25 (em S=5), tornando impossível atingir os limiares da
      escada de 5 níveis (0.85/0.70/0.50/0.30) — o motor só conseguia produzir
      "reject". Para concretizar a tabela documentada, `overall` passa a ser
      `mean((10-S)/10)` em escala 0..1 (1.0 = sem problemas = aprovar) e é essa
      a métrica que dirige as recomendações. `weighted` mantém a fórmula literal
      como indicador secundário de magnitude de severidade (não usado na escada).
    """
    if not problems:
        return [0.0], 1.0, 1.0
    scores = [(10 - SEVERITY[p]) / 10 for p in problems]
    overall = sum(scores) / len(scores)
    weighted_terms = [((10 - SEVERITY[p]) / 10) * (SEVERITY[p] / 10)
                      for p in problems]
    weighted = sum(weighted_terms) / len(weighted_terms)
    return scores, overall, weighted


def can_be_resubmitted(problems: List[str]) -> bool:
    return ("methodological_flaw" not in problems
            and "insufficient_data" not in problems)


def recommend_decision(problems: List[str]) -> Tuple[str, str]:
    """Espelha recommend_decision/3 do Prolog (árvore corrigida, 5 níveis).

    Usa `overall` (escala 0..1 corrigida) na escada de decisão, em vez do
    `weighted` original cuja escala tornava a escada inalcançável.
    """
    _, overall, _ = compute_scores(problems)

    if "plagiarism" in problems:
        return "reject", "Plágio detectado — rejeição imediata por violação ética."
    if "fabricated_data" in problems:
        return "reject", "Dados fabricados — rejeição por má conduta científica."
    if "methodological_flaw" in problems:
        return "reject", "Falha metodológica estrutural inviabiliza validação dos resultados."

    if overall >= 0.85:
        return "approve", "Manuscrito com alta qualidade e contribuição clara."
    if overall >= 0.70:
        return "minor_revision", ("Manuscrito com qualidades, mas com pequenos "
                                  "ajustes de forma ou argumentação.")
    if overall >= 0.50:
        return "major_revision", ("Manuscrito com potencial, mas requer revisões "
                                  "substantivas na estrutura ou análise.")
    if overall >= 0.30:
        if can_be_resubmitted(problems):
            return "reject_resubmit", ("Manuscrito com problemas estruturais, mas "
                                       "com potencial para ressubmissão.")
        return "reject", "Dados insuficientes para suportar as conclusões; rejeitado."
    return "reject", "Manuscrito não atende aos padrões mínimos de qualidade e contribuição."


# ============================================================================
# Entidades
# ============================================================================
@dataclass
class Manuscript:
    id: str
    title: str
    abstract: str
    text: str
    authors: List[str]
    metadata: Dict[str, Any]
    version: int = 1

    def anonymize(self) -> "Manuscript":
        meta = dict(self.metadata)
        meta["removed"] = "authors"
        return Manuscript(
            id=f"ANON-{uuid.uuid4().hex[:6]}",
            title=self.title, abstract=self.abstract, text=self.text,
            authors=[], metadata=meta, version=self.version)


@dataclass
class Reviewer:
    id: str
    name: str
    expertise: str
    active: bool = True


@dataclass
class Review:
    id: str
    manuscript_id: str
    reviewer_id: str
    scores: List[float]
    problems: List[str]
    decision: str
    quality_overall: float
    timestamp: float


@dataclass
class Assignments:
    manuscript_id: str
    reviewer_id: str
    due_date: str


class PeerReviewEngine:
    """Implementação em Python do Substrato 264 v2."""

    def __init__(self, seed: int = 42, synthetic_personas: Optional[Dict] = None):
        self._rand = random.Random(seed)
        self.manuscripts: Dict[str, Manuscript] = {}
        self.reviewers: Dict[str, Reviewer] = {}
        self.assignments: List[Assignments] = []
        self.reviews: List[Review] = []
        self.editor_decisions: Dict[str, Tuple[str, str, str]] = {}
        self.ethics_log: List[Dict] = []
        self.reviewer_stats: Dict[str, Dict[str, float]] = {}
        # Integração conceitual com Substrato 263 (personas sintéticas)
        self.personas: Dict[str, Dict[str, Any]] = synthetic_personas or {
            "p_001": {"name": "Persona 001", "density": 0.6,
                      "traits": ["metodológico", "quantitativo"]},
            "p_002": {"name": "Persona 002", "density": 0.85,
                      "traits": ["teórico", "qualitativo"]},
        }

    # ---- geradores de id ----
    def _msid(self) -> str:
        return f"MS-{int(time.time()*1000)}-{self._rand.randint(1000,9999)}"

    def _rvid(self) -> str:
        return f"RV-{int(time.time()*1000)}-{self._rand.randint(1000,9999)}"

    def _revid(self) -> str:
        return f"REV-{int(time.time()*1000)}-{self._rand.randint(1000,9999)}"

    # ---- manuscritos ----
    def create_manuscript(self, title, abstract, text, authors,
                          metadata) -> Manuscript:
        ms = Manuscript(self._msid(), title, abstract, text, list(authors),
                        dict(metadata))
        self.manuscripts[ms.id] = ms
        return ms

    def list_manuscripts(self) -> List[str]:
        return list(self.manuscripts.keys())

    def update_manuscript(self, ms_id, new_text) -> Optional[Manuscript]:
        ms = self.manuscripts.get(ms_id)
        if not ms:
            return None
        ms.text = new_text
        ms.version += 1
        return ms

    def anonymize(self, ms_id) -> Optional[Manuscript]:
        ms = self.manuscripts.get(ms_id)
        if not ms:
            return None
        anon = ms.anonymize()
        self.manuscripts[anon.id] = anon
        return anon

    # ---- pareceristas ----
    def create_reviewer(self, name, expertise) -> Reviewer:
        rv = Reviewer(self._rvid(), name, expertise)
        self.reviewers[rv.id] = rv
        self.reviewer_stats[rv.id] = {
            "total": 0, "avg_quality": 0.0, "avg_time_days": 0.0,
            "acceptance_rate": 0.0}
        return rv

    def list_reviewers(self) -> List[str]:
        return [rid for rid, r in self.reviewers.items() if r.active]

    def assign_reviewer(self, ms_id, reviewer_id, due_date) -> bool:
        if ms_id not in self.manuscripts or reviewer_id not in self.reviewers:
            return False
        self.assignments.append(Assignments(ms_id, reviewer_id, due_date))
        return True

    # ---- avaliação ----
    def evaluate_manuscript(self, ms_id, reviewer_id) -> Optional[Dict]:
        ms = self.manuscripts.get(ms_id)
        rv = self.reviewers.get(reviewer_id)
        if not ms or not rv:
            return None

        problems = detect_problems(ms.abstract, ms.text)
        scores, overall, weighted = compute_scores(problems)
        decision, justification = recommend_decision(problems)

        # 4 elementos do parecer (espelha generate_structured_review/5)
        summary = {
            "title": ms.title, "abstract": ms.abstract,
            "length": len(ms.text),
            "main_argument": ("main argument identified" if (
                _has(ms.text, "argumento") or _has(ms.text, "conclusão"))
                else "not explicitly stated")}
        qualities = [p for p in problems if SEVERITY[p] <= 3]
        insufficiencies = [p for p in problems if SEVERITY[p] >= 6]
        potentials = [p for p in problems if 4 <= SEVERITY[p] < 6]

        # qualidade do parecer (4 dimensões, aleatoriedade com seed)
        qdims = self._review_quality_dimensions()
        qoverall = sum(s for _, s in qdims) / len(qdims)
        qstatus = ("excellent" if qoverall >= 0.7 else
                   "adequate" if qoverall >= 0.5 else "needs_improvement")

        now = time.time()
        review = Review(self._revid(), ms_id, reviewer_id, scores, problems,
                        decision, qoverall, now)
        self.reviews.append(review)

        # atualiza estatísticas do parecerista (simulação de aceite)
        self._update_reviewer_stats(reviewer_id, qoverall, 14.0,
                                    decision == "approve")

        return {
            "manuscript_id": ms_id, "reviewer_id": reviewer_id,
            "reviewer_name": rv.name, "title": ms.title, "version": ms.version,
            "summary": summary, "qualities": qualities,
            "insufficiencies": insufficiencies, "potentials": potentials,
            "problems": problems, "scores": scores, "overall": overall,
            "weighted_score": weighted, "decision": decision,
            "justification": justification,
            "review_quality": {"dimensions": qdims, "overall": qoverall,
                               "status": qstatus},
            "timestamp": now,
        }

    def _review_quality_dimensions(self) -> List[Tuple[str, float]]:
        # matches Prolog compute_dimension_score/3 (summary uses history)
        n = sum(1 for r in self.reviews)
        if n > 0:
            s_dim = 0.7 + 0.3 * self._rand.random()
        else:
            s_dim = 0.5
        q_dim = 0.6 + 0.4 * self._rand.random()
        i_dim = 0.5 + 0.5 * self._rand.random()
        p_dim = 0.5 + 0.5 * self._rand.random()
        return [("summary", s_dim), ("qualities", q_dim),
                ("insufficiencies", i_dim), ("potentialities", p_dim)]

    # ---- editor ----
    def editor_decision(self, ms_id, decision, justification, editor_id):
        self.editor_decisions[ms_id] = (decision, justification, editor_id)
        return self.editor_decisions[ms_id]

    def resolve_conflict(self, ms_id, editor_id) -> str:
        decisions = [r.decision for r in self.reviews
                     if r.manuscript_id == ms_id]
        unique = sorted(set(decisions))
        if len(unique) > 1:
            final = unique[0]
            self.editor_decision(ms_id, final,
                                 "Decisão editorial para resolver conflito",
                                 editor_id)
            return f"Conflito resolvido pelo editor. Decisão final: {final}"
        return "Nenhum conflito detectado."

    # ---- métricas ----
    def _update_reviewer_stats(self, reviewer_id, qscore, days, accepted):
        s = self.reviewer_stats[reviewer_id]
        old_t, old_q, old_d, old_r = s["total"], s["avg_quality"], \
            s["avg_time_days"], s["acceptance_rate"]
        new_t = old_t + 1
        s["total"] = new_t
        s["avg_quality"] = (old_q * old_t + qscore) / new_t
        s["avg_time_days"] = (old_d * old_t + days) / new_t
        s["acceptance_rate"] = (old_r * old_t + int(accepted)) / new_t

    def reviewer_metrics(self, reviewer_id) -> Optional[Dict]:
        s = self.reviewer_stats.get(reviewer_id)
        if not s:
            return None
        return {"reviewer_id": reviewer_id, **s}

    def journal_metrics(self) -> Dict:
        n = len(self.reviews)
        if n > 0:
            acc = sum(1 for r in self.reviews if r.decision == "approve") / n
            rej = sum(1 for r in self.reviews if r.decision == "reject") / n
        else:
            acc = rej = 0.0
        return {"total_submissions": len(self.manuscripts),
                "total_reviews": n, "acceptance_rate": acc,
                "rejection_rate": rej, "avg_time_to_decision": 30.0}

    def transparency_report(self, year: int) -> Dict:
        m = self.journal_metrics()
        return {"year": year, "submissions": m["total_submissions"],
                "reviews": m["total_reviews"],
                "acceptance_rate": m["acceptance_rate"],
                "rejection_rate": m["rejection_rate"],
                "avg_decision_days": m["avg_time_to_decision"],
                "timestamp": time.time()}

    # ---- ética ----
    def ethics_check(self, ms_id, reviewer_id) -> Dict:
        ms = self.manuscripts.get(ms_id)
        if not ms:
            return {}
        meta = ms.metadata
        coi = not any(rv == reviewer_id
                      for rv in meta.get("excluded_reviewers", []))
        ai = meta.get("ai_used", "no") == "yes"
        gdpr = meta.get("gdpr", "compliant") == "compliant"
        cope = "plagiarism" not in detect_problems(ms.abstract, ms.text)
        fapesp = meta.get("fapesp_compliant", "yes") == "yes"
        icc = meta.get("icc_esomar", "yes") == "yes"
        overall = bool(coi and ai and gdpr)
        compliance = {
            "conflict_of_interest_ok": coi, "ai_disclosed": ai,
            "gdpr_compliant": gdpr, "cope_guidelines": cope,
            "fapesp_code": fapesp, "icc_esomar": icc, "overall": overall}
        self.ethics_log.append({"manuscript_id": ms_id,
                                "reviewer_id": reviewer_id,
                                "compliance": compliance,
                                "timestamp": time.time()})
        return compliance

    # ---- integração 263 (personas sintéticas) ----
    def synthetic_rework(self, ms_id, persona_id) -> Optional[Dict]:
        persona = self.personas.get(persona_id)
        ms = self.manuscripts.get(ms_id)
        if not persona or not ms:
            return None
        problems = detect_problems(ms.abstract, ms.text)
        base = 0.5 + 0.4 * persona["density"]
        noise = 0.1 * self._rand.random()
        score = max(0.0, min(1.0, base + noise))
        rec = "approve" if score > 0.7 else "minor_revision"
        return {"persona": persona_id, "density": persona["density"],
                "problems_found": problems, "score": score,
                "recommendation": rec}

    # ---- integração 262 (RSI) ----
    def rsi_optimize_criteria(self, ms_id) -> Any:
        all_problems = [p for r in self.reviews
                        if r.manuscript_id == ms_id for p in r.problems]
        if not all_problems:
            return "Nenhum ajuste necessário."
        return [{"problem": p, "new_severity": min(10, SEVERITY[p] + 1)}
                for p in all_problems]

    def rsi_feedback_loop(self, ms_id) -> Dict:
        return {"manuscript_id": ms_id,
                "criteria_adjustments": self.rsi_optimize_criteria(ms_id),
                "timestamp": time.time()}


# ============================================================================
# Relatório completo do fluxo (espelha run_peer_review_tests/0)
# ============================================================================
def run_demo_flow(seed: int = 42) -> Dict:
    """Executa o fluxo editorial completo e retorna um resumo estruturado."""
    engine = PeerReviewEngine(seed=seed)

    good_text = ("Apresentamos metodologia detalhada, dados empíricos robustos, "
                 "análise estatística, contexto histórico e teoria; o framework "
                 "empregado está bem fundamentado na bibliografia com muitas "
                 "referências. Objetivo e pergunta de pesquisa são claros, e a "
                 "narrativa é coerente, sem as lacunas típicas de manuscritos "
                 "prematuros, com discussão substancial dos resultados.")
    good_abs = ("Este artigo investiga o impacto da IA na avaliação acadêmica, "
                "apresenta metodologia, dados e análise, define objetivos claros "
                "e contribui para o avanço do campo de revisão por pares.")

    ms = engine.create_manuscript(
        "O impacto da IA na avaliação acadêmica", good_abs, good_text,
        ["Autor A", "Autor B"],
        {"excluded_reviewers": [], "ai_used": "yes", "gdpr": "compliant",
         "fapesp_compliant": "yes", "icc_esomar": "yes"})

    rv1 = engine.create_reviewer("Prof. Carlos Silva", "Metodologia")
    rv2 = engine.create_reviewer("Dra. Ana Oliveira", "Teoria Política")

    engine.assign_reviewer(ms.id, rv1.id, "2026-12-01")
    report = engine.evaluate_manuscript(ms.id, rv1.id)
    ethics = engine.ethics_check(ms.id, rv1.id)
    engine.editor_decision(ms.id, "minor_revision",
                           "Revisões menores sugeridas pelo parecerista",
                           "Editor-Chefe")
    transp = engine.transparency_report(2026)
    synth = engine.synthetic_rework(ms.id, "p_001")
    rsi = engine.rsi_feedback_loop(ms.id)

    return {
        "manuscript_id": ms.id,
        "decision": report["decision"],
        "weighted_score": report["weighted_score"],
        "problems_detected": report["problems"],
        "review_quality_status": report["review_quality"]["status"],
        "ethics": ethics,
        "transparency": transp,
        "synthetic_review": synth,
        "rsi_adjustments": rsi["criteria_adjustments"],
    }


if __name__ == "__main__":
    out = run_demo_flow()
    print(json.dumps(out, indent=2, default=str))
