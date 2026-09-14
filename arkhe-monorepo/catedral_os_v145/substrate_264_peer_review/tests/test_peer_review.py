#!/usr/bin/env python3
"""
Suíte de testes do Substrato 264 v2 — Peer Review Engine (port Python).

Rode de dentro de substrate_264_peer_review/ :
    python -m pytest tests -v
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import pytest

from substrate_264_peer_review import (
    PeerReviewEngine, SEVERITY, DECISION_TYPE, REVIEW_DIMENSIONS,
    detect_problems, compute_scores, recommend_decision, run_demo_flow,
    can_be_resubmitted,
)


# ---------------------------------------------------------------------------
# Detecção de problemas (12+2 categorias)
# ---------------------------------------------------------------------------
CLEAN_TEXT = ("Apresentamos metodologia detalhada, dados empíricos robustos, "
              "análise estatística, contexto histórico e teoria; o framework "
              "empregado está bem fundamentado na bibliografia com muitas "
              "referências. Objetivo e pergunta de pesquisa claros.")
CLEAN_ABS = ("Artigo com metodologia, dados e análise, objetivos definidos e "
             "contribuição que avança o campo.")


def basic_engine():
    return PeerReviewEngine(seed=7)


def make_ms(engine, text=CLEAN_TEXT, abstract=CLEAN_ABS):
    return engine.create_manuscript("Título", abstract, text, ["A"],
                                    {"ai_used": "yes", "gdpr": "compliant",
                                     "fapesp_compliant": "yes",
                                     "icc_esomar": "yes"})


def test_all_severities_defined():
    # every detected problem type must have a severity mapping
    for p in ["methodological_flaw", "insufficient_data", "unclear_contribution",
              "unclear_objectives", "internal_incoherence", "external_incoherence",
              "poor_contextualization", "weak_theory", "poor_lit_review",
              "concept_misuse", "style_issues", "poor_title_abstract",
              "plagiarism", "fabricated_data"]:
        assert p in SEVERITY
        assert 1 <= SEVERITY[p] <= 10


def test_clean_manuscript_no_problems():
    assert detect_problems(CLEAN_ABS, CLEAN_TEXT) == []


def test_detect_plagiarism():
    problems = detect_problems(CLEAN_ABS, CLEAN_TEXT + " trecho copiado.")
    assert "plagiarism" in problems


def test_detect_fabricated_data():
    problems = detect_problems(CLEAN_ABS, CLEAN_TEXT + " dado fabricado.")
    assert "fabricated_data" in problems


def test_detect_methodological_flaw():
    bare = "Um manuscrito onde não há descrição de procedimento de coleta."
    problems = detect_problems(CLEAN_ABS, bare)
    assert "methodological_flaw" in problems


def test_detect_poor_title_abstract():
    problems = detect_problems("curto.", CLEAN_TEXT)
    assert "poor_title_abstract" in problems


def test_detect_internal_incoherence():
    problems = detect_problems(CLEAN_ABS, CLEAN_TEXT + " Porém, contradiz-se.")
    assert "internal_incoherence" in problems


# ---------------------------------------------------------------------------
# Scores e consumo consistente
# ---------------------------------------------------------------------------
def test_clean_score_is_one():
    scores, overall, weighted = compute_scores([])
    assert scores == [0.0]
    assert overall == pytest.approx(1.0)
    assert weighted == pytest.approx(1.0)


def test_score_reflects_severity():
    # um problema de severidade 5 => (10-5)/10 = 0.5
    _, overall, _ = compute_scores(["concept_misuse"])
    assert overall == pytest.approx(0.5)


def test_more_severe_lower_score():
    o_low = compute_scores(["style_issues"])[1]     # sev 3 -> 0.7
    o_high = compute_scores(["unclear_objectives"])[1]  # sev 8 -> 0.2
    assert o_low > o_high


# ---------------------------------------------------------------------------
# Escada de 5 níveis — todos os ramos alcançáveis (auditoria v58.0)
# ---------------------------------------------------------------------------
def test_decision_approve_reachable():
    decision, _ = recommend_decision([])
    assert decision == "approve"


def test_decision_minor_revision():
    decision, _ = recommend_decision(["style_issues"])  # sev 3 -> 0.7
    assert decision == "minor_revision"


def test_decision_major_revision():
    decision, _ = recommend_decision(["concept_misuse"])  # sev 5 -> 0.5
    assert decision == "major_revision"


def test_decision_reject_resubmit():
    decision, _ = recommend_decision(["poor_contextualization"])  # sev 6 -> 0.4
    assert decision == "reject_resubmit"


def test_decision_reject_low():
    decision, _ = recommend_decision(["unclear_objectives"])  # sev 8 -> 0.2
    assert decision == "reject"


def test_decision_reject_methodological():
    decision, _ = recommend_decision(["methodological_flaw"])
    assert decision == "reject"


def test_decision_reject_plagiarism():
    decision, _ = recommend_decision(["plagiarism"])
    assert decision == "reject"


def test_decision_reject_fabricated():
    decision, _ = recommend_decision(["fabricated_data"])
    assert decision == "reject"


def test_decision_invalid_resubmit_without_data():
    decision, _ = recommend_decision(["insufficient_data", "poor_contextualization"])
    # sev9+sev6 -> overall 0.35; mas insufficient_data impede ressubmissão => reject
    assert decision == "reject"


# ---------------------------------------------------------------------------
# Engine — gestão de manuscritos + duplo-cega
# ---------------------------------------------------------------------------
def test_create_and_anonymize():
    engine = basic_engine()
    ms = make_ms(engine)
    anon = engine.anonymize(ms.id)
    assert anon.id != ms.id
    assert anon.authors == []
    assert "removed" in anon.metadata


def test_update_version():
    engine = basic_engine()
    ms = make_ms(engine)
    v0 = ms.version
    engine.update_manuscript(ms.id, CLEAN_TEXT + " v2")
    assert ms.version == v0 + 1


def test_assign_and_review():
    engine = basic_engine()
    ms = make_ms(engine)
    rv = engine.create_reviewer("Prof. X", "Metodologia")
    assert engine.assign_reviewer(ms.id, rv.id, "2026-12-01")
    report = engine.evaluate_manuscript(ms.id, rv.id)
    assert report["manuscript_id"] == ms.id
    assert report["reviewer_id"] == rv.id
    assert report["review_quality"]["status"] in ("excellent", "adequate",
                                                  "needs_improvement")


# ---------------------------------------------------------------------------
# Qualidade do parecer (4 dimensões)
# ---------------------------------------------------------------------------
def test_review_dimensions_count():
    engine = basic_engine()
    ms = make_ms(engine)
    rv = engine.create_reviewer("Prof. X", "Metodologia")
    report = engine.evaluate_manuscript(ms.id, rv.id)
    dims = report["review_quality"]["dimensions"]
    assert len(dims) == 4
    names = [d for d, _ in dims]
    assert set(names) == set(REVIEW_DIMENSIONS)
    assert 0.0 <= report["review_quality"]["overall"] <= 1.0


# ---------------------------------------------------------------------------
# Métricas e relatórios
# ---------------------------------------------------------------------------
def test_reviewer_stats_update():
    engine = basic_engine()
    ms = make_ms(engine)
    rv = engine.create_reviewer("Prof. X", "Metodologia")
    engine.evaluate_manuscript(ms.id, rv.id)
    stats = engine.reviewer_metrics(rv.id)
    assert stats["total"] == 1
    assert 0.0 <= stats["avg_quality"] <= 1.0


def test_journal_metrics_acceptance():
    engine = basic_engine()
    ms = make_ms(engine)  # manuscrito limpo => approve
    rv = engine.create_reviewer("Prof. X", "Metodologia")
    engine.evaluate_manuscript(ms.id, rv.id)
    m = engine.journal_metrics()
    assert m["total_reviews"] == 1
    assert m["acceptance_rate"] == pytest.approx(1.0)


def test_transparency_report():
    engine = basic_engine()
    rep = engine.transparency_report(2026)
    assert rep["year"] == 2026
    assert "submissions" in rep and "acceptance_rate" in rep


# ---------------------------------------------------------------------------
# Ética
# ---------------------------------------------------------------------------
def test_ethics_coi_violation():
    engine = basic_engine()
    rv = engine.create_reviewer("Prof. X", "Metodologia")
    ms = engine.create_manuscript("T", CLEAN_ABS, CLEAN_TEXT, ["A"],
                                  {"excluded_reviewers": [rv.id],
                                   "ai_used": "yes", "gdpr": "compliant"})
    eth = engine.ethics_check(ms.id, rv.id)
    assert eth["conflict_of_interest_ok"] is False
    assert eth["overall"] is False


def test_ethics_all_clear():
    engine = basic_engine()
    rv = engine.create_reviewer("Prof. X", "Metodologia")
    eth = engine.ethics_check(make_ms(engine).id, rv.id)
    assert eth["cope_guidelines"] is True
    assert eth["overall"] is True


# ---------------------------------------------------------------------------
# Integração 262 (RSI) e 263 (personas)
# ---------------------------------------------------------------------------
def test_rsi_criteria_adjustment():
    engine = basic_engine()
    ms = make_ms(engine)
    rv = engine.create_reviewer("Prof. X", "Metodologia")
    engine.evaluate_manuscript(ms.id, rv.id)  # manuscrito limpo => sem ajustes
    adj = engine.rsi_optimize_criteria(ms.id)
    assert adj == "Nenhum ajuste necessário."


def test_synthetic_review_bounds():
    engine = basic_engine()
    ms = make_ms(engine)
    out = engine.synthetic_rework(ms.id, "p_001")
    assert out is not None
    assert 0.0 <= out["score"] <= 1.0
    assert out["recommendation"] in DECISION_TYPE


def test_synthetic_review_unknown_persona():
    engine = basic_engine()
    assert engine.synthetic_rework(make_ms(engine).id, "p_nope") is None


# ---------------------------------------------------------------------------
# Fluxo de demonstração (end-to-end)
# ---------------------------------------------------------------------------
def test_demo_flow_approves_clean():
    out = run_demo_flow(seed=42)
    assert out["decision"] == "approve"
    assert out["weighted_score"] == pytest.approx(1.0)
    assert out["transparency"]["acceptance_rate"] == pytest.approx(1.0)
    assert out["ethics"]["overall"] is True


# ---------------------------------------------------------------------------
# Consistência de IDs únicos
# ---------------------------------------------------------------------------
def test_unique_ids():
    engine = basic_engine()
    ids = {make_ms(engine).id for _ in range(5)}
    assert len(ids) == 5  # seeds/ids não colidem
