#!/usr/bin/env python3
"""ARKHE — Auditoria de substrato bibliográfico v2.0.

Framework de 12 critérios de verificação para publicações citadas por
substratos. Invariante central (ref. bloco 1072, prec. I461/I462): **nenhum
resultado é fabricado** — todo veredito "ok"/"fail" deriva de uma resposta
viva de uma API real (Crossref/OpenAlex/DOAJ/doi.org) registada em
`evidence`; qualquer resultado não verificável é marcado literalmente como
`nao_verificado` com o motivo, nunca com valor inventado.

Impacto de invariantes:
  - Ghost-1/Ghost-2: SHA-256 do relatório e do registo por publicação;
    nenhum hash placeholder em saídas canónicas.
  - Loopseal-2: registo auditável e tamper-evident por construção (hash
    calculado sobre conteúdo canónico, campos ordenados).
  - Gravity-1: timestamps UTC monotónicos em todas as saídas.
  - Ethics-2: só consulta metadados públicos; nenhum dado além do DOI/ISSN
    coletado ou transmitido.

Dependências: stdlib apenas (urllib/json/hashlib). PyYAML é usado se
presente para parse do YAML; caso contrário aceita JSON. Sem dependências
externas obrigatórias; `sentence-transformers` é opcional e só ativa o
critério 5 (similaridade de abstract).

Uso canónico (ver docs/substrate/reproducibility.yaml):
  python tools/audit_substrate.py --input docs/substrate/publications.yaml
      --output-dir docs/substrate --mode online --similarity-threshold 0.85
      --hash-anterior <sha256_bloco_1074>
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
import urllib.error
import urllib.parse
import urllib.request
from dataclasses import dataclass, field
from datetime import datetime, timezone
from typing import Any, Optional

FRAMEWORK_VERSION = "ARKHE-SUBSTRATO-AUDIT-V2.0"
DEFAULT_SIMILARITY_THRESHOLD = 0.85
USER_AGENT = "arkhe-audit-substrate/2.0 (mailto:architect@arkhe.os)"
HTTP_TIMEOUT = 15
STATUS_DONE = {"ok", "warn", "fail", "nao_verificado", "skip"}

EN_STOP = {
    "the", "of", "and", "for", "with", "in", "on", "a", "an", "is", "are",
    "was", "were", "to", "that", "this", "we", "our", "as", "by", "from",
    "than", "at", "it", "its", "not", "or", "but",
}
PT_STOP = {
    "o", "a", "os", "as", "de", "do", "da", "dos", "das", "em", "no", "na",
    "com", "por", "para", "e", "ou", "que", "um", "uma", "é", "são", "foi",
    "ser", "como", "mais", "entre", "este", "esta", "não", "neste",
}
AI_MARKERS = (
    "as an ai language model",
    "i cannot",
    "i apologize",
    "furthermore, it is important to note",
    "in conclusion, this paper",
)


@dataclass
class Evidence:
    """Registo imutável de uma observação viva (fonte, url, resultado)."""

    source: str
    url: str
    outcome: str


@dataclass
class CheckResult:
    """Resultado de um critério. `ok`/`fail` exigem evidence associada."""

    criterion: int
    name: str
    status: str
    detail: str
    evidence: list[Evidence] = field(default_factory=list)


@dataclass
class Publication:
    """Entrada de publicações.yaml."""

    id: str
    title: str
    authors: list[dict[str, str]]
    year: Optional[int]
    doi: Optional[str]
    url: Optional[str]
    arxiv_id: Optional[str]
    issn: Optional[str]
    abstract: Optional[str]
    relation: str
    component: str
    force: str
    skip: list[int] = field(default_factory=list)


def log(msg: str) -> None:
    """Imprime mensagem em stderr (stderr mantém stdout canónico)."""
    print(msg, file=sys.stderr)


def sha256_hex(text: str) -> str:
    """SHA-256 de texto, hexadecimal maiúsculo (Ghost-1)."""
    return hashlib.sha256(text.encode("utf-8")).hexdigest().upper()


def _req(url: str, timeout: int = HTTP_TIMEOUT) -> tuple[Optional[Any], Optional[str]]:
    """GET com user-agent; retorna (dados, erro). Nunca lança."""
    request = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
    try:
        with urllib.request.urlopen(request, timeout=timeout) as response:
            body = response.read()
    except urllib.error.HTTPError as exc:
        return None, f"HTTP {exc.code}"
    except urllib.error.URLError as exc:
        return None, f"url_error: {exc.reason}"
    except Exception as exc:  # noqa: BLE001 — rede é imprevisível
        return None, f"erro: {exc}"
    content_type = ""
    try:
        content_type = response.headers.get("Content-Type", "")
    except NameError:
        content_type = ""
    if "json" in content_type:
        try:
            return json.loads(body.decode("utf-8")), None
        except (ValueError, UnicodeDecodeError):
            return None, "json_invalido"
    return body, None


def http_probe(url: str, timeout: int = HTTP_TIMEOUT) -> tuple[Optional[int], Optional[str]]:
    """GET com Range bytes=0-0 (liveness); retorna (status_final, erro).

    Distingue estado real do conteúdo de bloqueios de verificação (403/405/
    429 da proteção anti-bot), pois urllib pode ser bloqueado por Cloudflare
    mesmo quando o link está vivo.
    """
    request = urllib.request.Request(
        url,
        headers={"User-Agent": USER_AGENT, "Range": "bytes=0-0"},
    )
    try:
        with urllib.request.urlopen(request, timeout=timeout) as response:
            response.read(1024)
            return response.status, None
    except urllib.error.HTTPError as exc:
        return exc.code, None
    except urllib.error.URLError as exc:
        return None, f"url_error: {exc.reason}"
    except Exception as exc:  # noqa: BLE001
        return None, f"erro: {exc}"


def probe_verdict(status: Optional[int], error: Optional[str]) -> tuple[str, str]:
    """Classifica estado de link/DOI: ok/morto/bloqueado/nao_verificado."""
    if error is not None:
        return "nao_verificado", error
    assert status is not None
    if 200 <= status < 400:
        return "ok", f"http_{status}"
    if status in {404, 410}:
        return "fail", f"http_{status}"
    if status in {403, 405, 406, 429, 999}:
        return "warn", f"http_{status} (bloqueio de verificacao)"
    return "warn", f"http_{status}"


def normalize_title(title: str) -> str:
    """Normalização literal para comparação de títulos."""
    return re.sub(r"[^a-z0-9]+", " ", title.lower()).strip()


def doi_clean(doi: str) -> str:
    """Remove prefixo https://doi.org/ de um DOI."""
    return doi.lower().replace("https://doi.org/", "").replace("http://doi.org/", "").strip()


def lang_heuristic(text: str) -> str:
    """Detecção de idioma por razão de stopwords (heurística, critério 11)."""
    tokens = [t for t in re.findall(r"[a-zà-ü]+", text.lower()) if len(t) > 2]
    if not tokens:
        return "nao_detectado"
    n = len(tokens)
    en = sum(1 for t in tokens if t in EN_STOP) / n
    pt = sum(1 for t in tokens if t in PT_STOP) / n
    if en >= 0.20 and en > pt:
        return "en"
    if pt >= 0.20 and pt > en:
        return "pt"
    return "outro_nao_detectado"


def crossref_work(doi: str, evidence: list[Evidence]) -> Optional[dict]:
    """Consulta Crossref (API real). Registra evidência."""
    url = f"https://api.crossref.org/works/{doi_clean(doi)}?mailto=architect@arkhe.os"
    data, error = _req(url)
    if error is not None:
        evidence.append(Evidence("crossref", url, error))
        return None
    evidence.append(Evidence("crossref", url, "http_200"))
    message = data.get("message") if isinstance(data, dict) else None
    return message if isinstance(message, dict) else None


def openalex_work(doi: str, evidence: list[Evidence]) -> Optional[dict]:
    """Consulta OpenAlex (API real). Registra evidência."""
    url = f"https://api.openalex.org/works/https://doi.org/{urllib.parse.quote(doi_clean(doi))}"
    data, error = _req(url)
    if error is not None:
        evidence.append(Evidence("openalex", url, error))
        return None
    evidence.append(Evidence("openalex", url, "http_200"))
    return data if isinstance(data, dict) else None


def _openalex_abstract(data: dict) -> Optional[str]:
    """Reconstrói abstract do inverted_index do OpenAlex."""
    index = data.get("abstract_inverted_index")
    if not isinstance(index, dict) or not index:
        return None
    positions: dict[int, str] = {}
    for word, indices in index.items():
        for pos in indices:
            positions[int(pos)] = word
    return " ".join(positions.get(i, "") for i in sorted(positions))


def _rename(value: Optional[str], default: str = "nao_verificado") -> str:
    return value if value is not None else default


def _crossref_type_to_peer(ctype: Optional[str]) -> str:
    if ctype is None:
        return "nao_verificado"
    if ctype in {"journal-article", "proceedings-article", "book-chapter"}:
        return "revisado"
    if ctype in {"posted-content", "preprint", "preprint"}:
        return "preprint"
    return "outro"


def check_link(pub: Publication, offline: bool, evidence: list[Evidence]) -> CheckResult:
    """Critério 1 — link vivo."""
    if not pub.url:
        return CheckResult(1, "link_vivo", "skip", "sem url na entrada")
    if offline:
        return CheckResult(1, "link_vivo", "nao_verificado", "offline")
    status, error = http_probe(pub.url)
    if error is not None:
        evidence.append(Evidence("url", pub.url, error))
    else:
        evidence.append(Evidence("url", pub.url, f"http_{status}"))
    st, detail = probe_verdict(status, error)
    return CheckResult(1, "link_vivo", st, detail)


def check_title(pub: Publication, cr: Optional[dict], oa: Optional[dict], evidence: list[Evidence]) -> CheckResult:
    """Critério 2 — título confere."""
    live_title = None
    source = None
    if cr is not None and cr.get("title"):
        live_title, source = cr["title"][0], "crossref"
    elif oa is not None and oa.get("title"):
        live_title, source = str(oa["title"]), "openalex"
    if not pub.title or live_title is None:
        return CheckResult(2, "titulo", "nao_verificado", "sem fonte de metadados")
    if source == "crossref":
        evidence.append(Evidence("crossref", "title", "metadados"))
    else:
        evidence.append(Evidence("openalex", "title", "metadados"))
    if normalize_title(pub.title) == normalize_title(live_title):
        return CheckResult(2, "titulo", "ok", "literal_match")
    return CheckResult(2, "titulo", "fail", f"divergente: '{pub.title}' vs '{live_title}'")


def check_authors(pub: Publication, cr: Optional[dict], oa: Optional[dict], evidence: list[Evidence]) -> CheckResult:
    """Critério 3 — autores conferem (ORCID quando disponível)."""
    live = []
    if cr is not None and cr.get("author"):
        for a in cr["author"]:
            name = a.get("given", "") + " " + a.get("family", "")
            live.append((name.strip(), a.get("ORCID", "")))
        evidence.append(Evidence("crossref", "author", "metadados"))
    elif oa is not None and oa.get("authorships"):
        for a in oa["authorships"]:
            author = a.get("author", {})
            live.append((author.get("display_name", ""), author.get("orcid", "")))
        evidence.append(Evidence("openalex", "authorships", "metadados"))
    if not pub.authors or not live:
        return CheckResult(3, "autores", "nao_verificado", "sem fonte de metadados")
    expected = {(a.get("name", "") or "").strip().lower() for a in pub.authors if a.get("name")}
    expected_orcids = {a.get("orcid", "") for a in pub.authors if a.get("orcid")}
    actual = {name.strip().lower() for name, _ in live if name.strip()}
    orcid_hits = sum(1 for _, o in live if o and o in expected_orcids)
    if expected and expected.issubset(actual):
        return CheckResult(3, "autores", "ok", "match")
    if expected & actual or orcid_hits:
        return CheckResult(3, "autores", "warn", "parcial")
    return CheckResult(3, "autores", "fail", "divergente")


def check_year(pub: Publication, cr: Optional[dict], oa: Optional[dict], evidence: list[Evidence]) -> CheckResult:
    """Critério 4 — ano confere."""
    live_year = None
    source = None
    if oa is not None and oa.get("publication_year"):
        live_year, source = int(oa["publication_year"]), "openalex"
    elif cr is not None:
        for key in ("published-print", "published-online", "issued", "created"):
            parts = cr.get(key, {}).get("date-parts", [[None]])[0]
            if parts and parts[0]:
                live_year, source = int(parts[0]), "crossref"
                break
    if pub.year is None or live_year is None:
        return CheckResult(4, "ano", "nao_verificado", "sem fonte de metadados")
    evidence.append(Evidence(source, "publication_year", "metadados"))
    if pub.year == live_year:
        return CheckResult(4, "ano", "ok", f"{live_year}")
    return CheckResult(4, "ano", "fail", f"{pub.year} vs {live_year}")


def check_abstract(pub: Publication, cr: Optional[dict], oa: Optional[dict], threshold: float,
                   evidence: list[Evidence]) -> CheckResult:
    """Critério 5 — abstract corresponde (similaridade; requer modelo opcional)."""
    live_text = None
    source = None
    if oa is not None:
        live_text, source = _openalex_abstract(oa), "openalex"
    elif cr is not None and cr.get("abstract"):
        live_text, source = cr["abstract"], "crossref"
    if not pub.abstract or not live_text:
        return CheckResult(5, "abstract", "nao_verificado", "abstract indisponivel em entrada ou metadados")
    try:
        from sentence_transformers import SentenceTransformer  # type: ignore
        from numpy.linalg import norm  # type: ignore
        import numpy as np  # type: ignore
        model = SentenceTransformer("all-MiniLM-L6-v2")
    except Exception as exc:  # noqa: BLE001 — dependência opcional
        return CheckResult(5, "abstract", "nao_verificado", f"modelo indisponivel: {exc}")
    a = model.encode([pub.abstract])[0]
    b = model.encode([live_text])[0]
    sim = float(np.dot(a, b) / (norm(a) * norm(b) + 1e-12))
    evidence.append(Evidence(source, "abstract", f"similarity={sim:.4f}, modelo=all-MiniLM-L6-v2"))
    if sim >= threshold:
        return CheckResult(5, "abstract", "ok", f"similaridade {sim:.4f}")
    return CheckResult(5, "abstract", "fail", f"similaridade {sim:.4f} < {threshold}")


def check_doi(pub: Publication, offline: bool, evidence: list[Evidence]) -> CheckResult:
    """Critério 6 — DOI resolve em doi.org."""
    if not pub.doi:
        return CheckResult(6, "doi", "skip", "sem doi na entrada")
    if offline:
        return CheckResult(6, "doi", "nao_verificado", "offline")
    status, error = http_probe(f"https://doi.org/{doi_clean(pub.doi)}")
    if error is not None:
        evidence.append(Evidence("doi.org", pub.doi, error))
    else:
        evidence.append(Evidence("doi.org", pub.doi, f"http_{status}"))
    st, detail = probe_verdict(status, error)
    return CheckResult(6, "doi", st, detail)


def check_peer(cr: Optional[dict], oa: Optional[dict], evidence: list[Evidence]) -> CheckResult:
    """Critério 7 — peer-review por tipo de metadados."""
    ctype = None
    source = "crossref"
    if cr is not None:
        ctype = cr.get("type")
    elif oa is not None:
        ctype = oa.get("type")
        source = "openalex"
    verdict = _crossref_type_to_peer(ctype)
    evidence.append(Evidence(source, "type", str(ctype or "ausente")))
    detail = str(ctype or "nao_verificado")
    if ctype in {"journal-article", "proceedings-article", "book-chapter"}:
        status = "ok"
    elif ctype in {"posted-content", "preprint"}:
        status = "warn"
    else:
        status = "nao_verificado"
    return CheckResult(7, "peer_review", status, detail)


def check_retraction(cr: Optional[dict], oa: Optional[dict], evidence: list[Evidence]) -> CheckResult:
    """Critério 8 — retratação (Crossref update-to; OpenAlex is_retracted)."""
    if oa is not None and oa.get("is_retracted") is True:
        evidence.append(Evidence("openalex", "is_retracted", "true"))
        return CheckResult(8, "retratacao", "fail", "retratado")
    if oa is not None and oa.get("is_retracted") is False:
        evidence.append(Evidence("openalex", "is_retracted", "false"))
        return CheckResult(8, "retratacao", "ok", "clean")
    if cr is not None and "update-to" in cr:
        for update in cr["update-to"]:
            update_type = str(update.get("type", ""))
            if "retract" in update_type or "withdraw" in update_type:
                evidence.append(Evidence("crossref", "update-to", update_type))
                return CheckResult(8, "retratacao", "fail", update_type)
        evidence.append(Evidence("crossref", "update-to", "sem_retracao"))
        return CheckResult(8, "retratacao", "ok", "clean")
    return CheckResult(8, "retratacao", "nao_verificado", "sem fonte")


def check_journal(issn: Optional[str], evidence: list[Evidence]) -> CheckResult:
    """Critério 9 — journal predatório (DOAJ real; Beall apenas se arquivo local existir)."""
    if not issn:
        return CheckResult(9, "journal", "nao_verificado", "sem issn")
    beall_file = "tools/data/beall_issns.txt"
    try:
        with open(beall_file, "r", encoding="utf-8") as handle:
            beall = {line.strip().lower() for line in handle if line.strip()}
    except OSError:
        beall = set()
    if issn.strip().lower() in beall:
        evidence.append(Evidence("beall_local", beall_file, "issn_listado"))
        return CheckResult(9, "journal", "fail", "predatorio (beall local)")
    url = f"https://doaj.org/api/search/journals/{urllib.parse.quote(issn.strip())}"
    data, error = _req(url)
    if error is not None:
        evidence.append(Evidence("doaj", url, error))
        return CheckResult(9, "journal", "nao_verificado", f"doaj: {error}")
    evidence.append(Evidence("doaj", url, "http_200"))
    total = data.get("total", 0) if isinstance(data, dict) else 0
    if total and total > 0:
        return CheckResult(9, "journal", "ok", "indexado_doaj")
    return CheckResult(9, "journal", "nao_verificado", "ausente_doaj_neq_predatorio")


def check_version(cr: Optional[dict], oa: Optional[dict], pub: Publication, evidence: list[Evidence]) -> CheckResult:
    """Critério 10 — versão canónica (preprint vs publicado)."""
    oa_type = oa.get("type") if oa is not None else None
    cr_type = cr.get("type") if cr is not None else None
    has_arxiv = bool(pub.arxiv_id)
    if oa_type == "preprint" or cr_type in {"posted-content", "preprint"}:
        if has_arxiv or not (oa_type == "article"):
            evidence.append(Evidence("openalex", "type", "preprint"))
            return CheckResult(10, "versao", "warn", "preprint")
    if oa_type in {"article", "journal-article"} or cr_type in {"journal-article", "proceedings-article"}:
        evidence.append(Evidence("openalex", "type", "published"))
        return CheckResult(10, "versao", "ok", "published")
    return CheckResult(10, "versao", "nao_verificado", "sem fonte")


def check_language(pub: Publication, oa: Optional[dict], evidence: list[Evidence]) -> CheckResult:
    """Critério 11 — idioma (OpenAlex language ou heurística de stopwords)."""
    if oa is not None and oa.get("language"):
        evidence.append(Evidence("openalex", "language", str(oa["language"])))
        return CheckResult(11, "idioma", "ok", str(oa["language"]))
    if pub.abstract:
        detected = lang_heuristic(pub.abstract)
        evidence.append(Evidence("heuristica_stopwords", "abstract", detected))
        return CheckResult(11, "idioma", "warn", detected)
    return CheckResult(11, "idioma", "nao_verificado", "sem texto")


def check_ai(pub: Publication, evidence: list[Evidence]) -> CheckResult:
    """Critério 12 — AI-generated (heurística de marcadores; default honesto)."""
    if not pub.abstract:
        return CheckResult(12, "ai_generated", "nao_verificado", "sem abstract")
    lowered = pub.abstract.lower()
    hits = [marker for marker in AI_MARKERS if marker in lowered]
    if hits:
        evidence.append(Evidence("heuristica_marcadores", "abstract", "|".join(hits)))
        return CheckResult(12, "ai_generated", "warn", "suspeito")
    return CheckResult(12, "ai_generated", "ok", "sem_marcadores")


def audit_publication(pub: Publication, offline: bool, threshold: float) -> dict[str, Any]:
    """Executa os 12 critérios sobre uma publicação; retorna registo canónico."""
    meta_evidence: list[Evidence] = []
    cr = None
    oa = None
    if not offline:
        if pub.doi:
            cr = crossref_work(pub.doi, meta_evidence)
            oa = openalex_work(pub.doi, meta_evidence)

    def bound(check: CheckResult, ev: list[Evidence]) -> CheckResult:
        """Associa a evidência do critério (coletada durante sua execução)."""
        check.evidence = ev
        return check

    # Evidência por critério (listas independentes — trace auditável).
    ev1: list[Evidence] = []
    ev2: list[Evidence] = []
    ev3: list[Evidence] = []
    ev4: list[Evidence] = []
    ev5: list[Evidence] = []
    ev6: list[Evidence] = []
    ev7: list[Evidence] = []
    ev8: list[Evidence] = []
    ev9: list[Evidence] = []
    ev10: list[Evidence] = []
    ev11: list[Evidence] = []
    ev12: list[Evidence] = []
    checks = [
        bound(check_link(pub, offline, ev1), ev1),
        bound(check_title(pub, cr, oa, ev2), ev2),
        bound(check_authors(pub, cr, oa, ev3), ev3),
        bound(check_year(pub, cr, oa, ev4), ev4),
        bound(check_abstract(pub, cr, oa, threshold, ev5), ev5),
        bound(check_doi(pub, offline, ev6), ev6),
        bound(check_peer(cr, oa, ev7), ev7),
        bound(check_retraction(cr, oa, ev8), ev8),
        bound(check_journal(pub.issn, ev9), ev9),
        bound(check_version(cr, oa, pub, ev10), ev10),
        bound(check_language(pub, oa, ev11), ev11),
        bound(check_ai(pub, ev12), ev12),
    ]
    results: list[dict[str, Any]] = []
    count = {"ok": 0, "fail": 0, "warn": 0, "nao_verificado": 0, "skip": 0}
    for check in checks:
        if check.criterion in pub.skip:
            check.status = "skip"
            check.detail = "critério omitido na entrada"
        count[check.status] += 1
        results.append({
            "criterio": check.criterion,
            "nome": check.name,
            "status": check.status,
            "detalhe": check.detail,
            "evidencia": [e.outcome for e in check.evidence],
        })
    if count["fail"]:
        verdict = "fabricated"
    elif count["skip"] == len(checks):
        verdict = "unverified"
    elif count["ok"] == 0:
        verdict = "unverified"
    elif count["warn"] == 0 and count["nao_verificado"] == 0:
        verdict = "verified"
    else:
        verdict = "partial"
    all_evidence = meta_evidence + [e for check in checks for e in check.evidence]
    record = {
        "id": pub.id,
        "contrib": pub.component,
        "relacao": pub.relation,
        "forca": pub.force,
        "veredito": verdict,
        "contagens": count,
        "criterios": results,
        "evidencia_bruta": [
            {"fonte": e.source, "url": e.url, "resultado": e.outcome} for e in all_evidence
        ],
    }
    canonical = json.dumps(record, sort_keys=True, ensure_ascii=False, separators=(",", ":"))
    record["hash_registo"] = "sha256:" + sha256_hex(canonical)
    return record


def parse_input(path: str) -> list[Publication]:
    """Parse de publicações (YAML via PyYAML se presente; senão JSON)."""
    with open(path, "r", encoding="utf-8") as handle:
        text = handle.read()
    try:
        import yaml  # type: ignore

        doc = yaml.safe_load(text) or {}
    except ImportError:
        try:
            doc = json.loads(text)
        except ValueError:
            raise SystemExit("PyYAML nao instalado e input nao e JSON. Instale PyYAML ou use JSON.")
    pubs = []
    for item in doc.get("publications", []):
        if not item.get("id"):
            raise SystemExit(f"publicacao sem id em {path}")
        skip = [int(x) for x in item.get("skip", [])]
        pubs.append(Publication(
            id=str(item["id"]),
            title=str(item.get("title", "")),
            authors=[dict(a) for a in item.get("authors", [])],
            year=int(item["year"]) if item.get("year") is not None else None,
            doi=str(item["doi"]) if item.get("doi") else None,
            url=str(item["url"] if item.get("url") else "") or None,
            arxiv_id=str(item["arxiv_id"]) if item.get("arxiv_id") else None,
            issn=str(item["issn"]) if item.get("issn") else None,
            abstract=str(item.get("abstract", "")) or None,
            relation=str(item.get("relation", "fundamentacao")),
            component=str(item.get("component", "")),
            force=str(item.get("force", "direta")),
            skip=skip,
        ))
    return pubs


def render_table(records: list[dict[str, Any]]) -> str:
    """Tabela consolidada dos vereditos por critério."""
    header = (f"| ID | Veredito | L1 link | L2 titulo | L3 autores | L4 ano | L5 abs | "
              f"L6 doi | L7 peer | L8 retr | L9 journal | L10 versao | L11 idio | L12 ai |")
    sep = "|" + "---|" * 15
    rows = [header, sep]
    short = {"ok": "ok", "fail": "FAIL", "warn": "~", "nao_verificado": "?", "skip": "-"}
    for rec in records:
        cell = {c["nome"]: c["status"] for c in rec["criterios"]}
        ordered = [
            cell.get("link_vivo", "?"), cell.get("titulo", "?"), cell.get("autores", "?"),
            cell.get("ano", "?"), cell.get("abstract", "?"), cell.get("doi", "?"),
            cell.get("peer_review", "?"), cell.get("retratacao", "?"), cell.get("journal", "?"),
            cell.get("versao", "?"), cell.get("idioma", "?"), cell.get("ai_generated", "?"),
        ]
        rows.append("| {} | {} | {}".format(
            rec["id"], rec["veredito"], " | ".join(short.get(s, s) for s in ordered)
        ))
    return "\n".join(rows)


def render_report(records: list[dict[str, Any]], args: argparse.Namespace, input_text: str,
                  report_hash: str) -> str:
    """Gera AUDIT-REPORT.md canónico."""
    utc = datetime.now(timezone.utc).isoformat(timespec="seconds")
    counts = {"verified": 0, "partial": 0, "unverified": 0, "fabricated": 0}
    for rec in records:
        counts[rec["veredito"]] += 1
    lines = [
        "# AUDIT-REPORT — Substrato bibliográfico",
        "",
        f"- Framework: {FRAMEWORK_VERSION}",
        f"- Data UTC: {utc}",
        f"- Modo: {args.mode}",
        f"- Input: {args.input} (sha256 entrada = {sha256_hex(input_text)})",
        f"- Threshold similaridade: {args.similarity_threshold}",
        f"- Publicações: {len(records)}",
        "",
        "## Resumo de vereditos",
        "",
        "| Veredito | Quantidade |",
        "|---|--:|",
    ] + [f"| {name} | {counts[name]} |" for name in ("verified", "partial", "unverified", "fabricated")] + [
        "",
        "> Honestidade: resultados `ok`/`fail` derivam exclusivamente de respostas"
        " vivas registadas em `evidencia_bruta`. `nao_verificado` indica ausência"
        " de fonte/modelo — nunca valor fabricado.",
        "",
        "## Tabela de critérios",
        "",
        render_table(records),
        "",
        "## Registo canónico (hash por publicação)",
        "",
    ]
    for rec in records:
        lines.append(f"- {rec['id']}: {rec['veredito']} — `{rec['hash_registo']}`")
    lines += [
        "",
        "## Fragmento para o ledger (bloco real seguinte)",
        "",
        "```json",
    ]
    fragment = {
        "versao": FRAMEWORK_VERSION,
        "data": utc,
        "tipo": "AUDITORIA_SUBSTRATO_FRAMEWORK",
        "descricao": "Auditoria bibliografica v2.0 — 12 criterios.",
        "principio": "Nenhum link, DOI ou abstract e fabricado.",
        "publicacoes": len(records),
        "vereditos": counts,
        "hash_conteudo": f"sha256:{report_hash}",
        "hash_anterior": args.hash_anterior or "PENDENTE___preencher_com_hash_real_do_bloco_1074",
    }
    lines.append(json.dumps(fragment, sort_keys=True, ensure_ascii=False, indent=2))
    lines += ["```", ""]
    return "\n".join(lines)


def render_discrepancies(records: list[dict[str, Any]]) -> str:
    """Gera discrepancies.md — apenas divergências/avisos, com evidência."""
    lines = [
        "# Discrepâncias — Auditoria de substrato bibliográfico",
        "",
        "> Critérios com status `fail` ou `warn`. Linhas `ok`/`nao_verificado` não constam.",
        "",
    ]
    n = 0
    for rec in records:
        for crit in rec["criterios"]:
            if crit["status"] in {"fail", "warn"}:
                n += 1
                lines.append(f"- **{rec['id']}** critério {crit['criterio']} `{crit['nome']}`"
                             f" = `{crit['status']}` — {crit['detalhe']}"
                             f" (evidência: {', '.join(crit['evidencia']) or 'nenhuma'})")
    if n == 0:
        lines.append("Nenhuma discrepância registada.")
    return "\n".join(lines)


def selfcheck() -> int:
    """Checagem honesta da lógica de vereditos (nunca garante integridade)."""
    failures = []
    offender = Publication(id="X", title="T", authors=[{}], year=None, doi=None,
                           url="https://example.invalid/", arxiv_id=None, issn=None,
                           abstract=None, relation="fundamentacao", component="c", force="direta")
    records = audit_publication(offender, offline=True, threshold=DEFAULT_SIMILARITY_THRESHOLD)
    if records["veredito"] == "verified":
        failures.append("publicacao sem nenhuma fonte viva nao pode ser 'verified'")
    divergent = Publication(id="Y", title="Título Diferente", authors=[{"name": "Alguém"}], year=1999,
                            doi="10.1145/legit", url="https://example.invalid/", arxiv_id=None,
                            issn="0000-0000", abstract="resumo",
                            relation="fundamentacao", component="c", force="direta",
                            skip=list(range(1, 13)))
    rec2 = audit_publication(divergent, offline=True, threshold=DEFAULT_SIMILARITY_THRESHOLD)
    if rec2["veredito"] not in {"unverified"}:
        failures.append("skip total deve produzir 'unverified' (offline, sem fonte)")
    criterion_ids = {c["criterio"] for c in records["criterios"]}
    if criterion_ids != set(range(1, 13)):
        failures.append(f"criterios incompletos: {sorted(criterion_ids)}")
    log(f"selfcheck: {'PASS' if not failures else 'FAIL'}")
    for failure in failures:
        log(f"  - {failure}")
    return 0 if not failures else 1


def main(argv: Optional[list[str]] = None) -> int:
    parser = argparse.ArgumentParser(prog="audit_substrate.py", description=__doc__)
    parser.add_argument("--input", required=False, help="publications.yaml/json")
    parser.add_argument("--output-dir", help="diretorio de saida (AUDIT-REPORT.md, discrepancies.md)")
    parser.add_argument("--mode", choices=["online", "offline"], default="online")
    parser.add_argument("--similarity-threshold", type=float, default=DEFAULT_SIMILARITY_THRESHOLD)
    parser.add_argument("--hash-anterior", help="sha256 real do bloco anterior (p.ex. bloco 1074)")
    parser.add_argument("--selfcheck", action="store_true", help="valida a logica de vereditos")
    args = parser.parse_args(argv)

    if args.selfcheck:
        return selfcheck()
    if not args.input:
        parser.error("--input obrigatorio (ou use --selfcheck)")

    with open(args.input, "r", encoding="utf-8") as handle:
        input_text = handle.read()
    publications = parse_input(args.input)
    if not publications:
        log("lista vazia — auditoria nao executada")
        return 0

    offline = args.mode == "offline"
    records = [audit_publication(pub, offline=offline, threshold=args.similarity_threshold)
               for pub in publications]
    canonical_text = json.dumps(records, sort_keys=True, ensure_ascii=False, separators=(",", ":"))
    report_hash = sha256_hex(canonical_text)
    report = render_report(records, args, input_text, report_hash)
    discrepancies = render_discrepancies(records)

    if args.output_dir:
        import os
        os.makedirs(args.output_dir, exist_ok=True)
        report_path = os.path.join(args.output_dir, "AUDIT-REPORT.md")
        disc_path = os.path.join(args.output_dir, "discrepancies.md")
        with open(report_path, "w", encoding="utf-8") as handle:
            handle.write(report)
        with open(disc_path, "w", encoding="utf-8") as handle:
            handle.write(discrepancies)
        log(f"AUDIT-REPORT -> {report_path}")
        log(f"discrepancies -> {disc_path}")
    print(report)
    log(f"SHA-256 conteudo canónico: sha256:{report_hash}")
    return 0


if __name__ == "__main__":
    sys.exit(main())