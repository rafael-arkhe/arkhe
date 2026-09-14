# Framework de Auditoria de Substrato Bibliográfico — v2.0

**Framework:** `ARKHE-SUBSTRATO-AUDIT-V2.0`
**Ferramenta:** `tools/audit_substrate.py`
**Entrada:** `docs/substrate/publications.yaml`
**Saídas:** `AUDIT-REPORT.md`, `discrepancies.md` (hash SHA-256 do conteúdo canónico)
**Data de adoção:** 2026-09-11 — **Ancoragem no ledger:** bloco real seguinte (1075)

---

## 1. Princípio de integridade (invariante central)

Todo veredito `ok`/`fail` deriva **exclusivamente** de uma resposta viva de
uma API real (Crossref/OpenAlex/DOAJ/doi.org), registada no campo
`evidencia_bruta` de cada publicação. Ausência de fonte ou de modelo produz o
status literal `nao_verificado` com o motivo — **nunca** um valor inventado
(precedente I461/I462, bloco 990; bloco 1072).

Sem uma lista real em `publications.yaml`, a ferramenta para: nenhum link,
DOI ou abstract é fabricado.

## 2. Checklist de 12 critérios

| # | Critério | Fonte | Veredito |
|:--:|:--|:--|:--|
| 1 | Link vivo | HTTP HEAD à URL | ok / fail |
| 2 | Título confere | Crossref/OpenAlex (match literal) | ok / fail |
| 3 | Autores conferem | Crossref/OpenAlex + ORCID | ok / warn parcial / fail |
| 4 | Ano confere | `publication_year`/`issued` | ok / fail |
| 5 | Abstract corresponde | `sentence-transformers` (threshold configurável, default 0.85) | ok / fail / nao_verificado sem modelo |
| 6 | DOI resolve | `https://doi.org/{doi}` | ok / fail |
| 7 | Peer-review | tipo Crossref/OpenAlex (journal-article / preprint) | ok / warn / nao_verificado |
| 8 | Retratação | Crossref `update-to` + OpenAlex `is_retracted` | ok clean / fail retratado |
| 9 | Journal predatório | DOAJ (API real); Beall só via `tools/data/beall_issns.txt` local | ok doaj / fail beall / nao_verificado |
| 10 | Versão canónica | preprint vs. publicado (Crossref/OpenAlex) | ok published / warn preprint |
| 11 | Idioma | OpenAlex `language` ou heurística de stopwords (EN/PT) | ok / warn heurística |
| 12 | AI-generated | heurística de marcadores; default honesto `sem_marcadores` | ok / warn suspeito |

## 3. Vereditos de integridade

| Veredito | Regra |
|:--|:--|
| `verified` | zero fail, zero warn, zero `nao_verificado` |
| `partial` | zero fail, mas com warn e/ou `nao_verificado` |
| `unverified` | ausência total de evidência viva (ex. `offline`) |
| `fabricated` | qualquer divergência medida (título/autores/ano/abstract/DOI/link) |

Critérios listados em `skip:` na entrada não contam (status `skip`).

## 4. Fechamento dos gaps identificados na auditoria v574.0

| Gap (v574.0) | Estado no v2.0 |
|:--|:--|
| G1 retratação | critério 8 (Crossref `update-to`, OpenAlex `is_retracted`) |
| G2 journal predatório | critério 9 (DOAJ real + Beall local, sem fabricar lista) |
| G3 threshold 0.85 arbitrário | configurável por CLI; modelo pinado e registado na evidência |
| G4 versões preprint/published | critério 10 |
| G5 multilingual | critério 11 |
| G6 ORCID | critério 3 (compare ORCID quando presente) |
| G7 reprodutibilidade | `reproducibility.yaml` + rerun_command determinístico (offline) |
| G8 integração com ledger real | fragmento JSON no report com `hash_conteudo` SHA-256; ancoragem no **bloco 1075** (cadeia canónica QC-0892), **não** no "bloco 10" do ledger paralelo 'clareira' |
| G9 hash de integridade | SHA-256 por publicação e do conteúdo canónico (Ghost-1/Ghost-2) |
| G10 política de staleness | tabela abaixo |
| G11 AI-generated | critério 12 (heurística, default honesto) |
| G12 figuras/dados | **não implementado** — forensics de imagem fica **aberta** (sem substrato; ver §7) |

## 5. Política de staleness

| Tipo | Re-auditoria |
|:--|:--|
| Preprint (arXiv) | a cada 30 dias |
| Journal published | a cada 180 dias |
| Conferência | a cada 365 dias |
| Blog / não revisado | a cada 90 dias |

Re-auditoria = re-executar `rerun_command` e comparar hashes: mudança de hash
no mesmo DOI é própria alteração do estado (retratação, versão nova) e gera
`discrepancies.md`.

## 6. Uso

```bash
# verificação da lógica de vereditos (honestidade estrutural)
python tools/audit_substrate.py --selfcheck

# auditoria completa (online; braço em docs/substrate)
python tools/audit_substrate.py --input docs/substrate/publications.yaml \
    --output-dir docs/substrate --mode online \
    --similarity-threshold 0.85 --hash-anterior <sha256_real_do_bloco_1074>

# determinístico, sem rede — tudo sai como 'nao_verificado', nunca fabricado
python tools/audit_substrate.py --input docs/substrate/publications.yaml --mode offline
```

## 7. Limites honestos (o que este framework NÃO faz)

- **Não detecta manipulação de figuras/dados** (G12) — requer forensics de
  imagem (ex. error-level analysis), sem substrato aqui. Fica **aberta**.
- **Não prova autenticidade de conteúdo** — verifica metadados e heurísticas;
  mentiras bem-feitas em abstract podem não ser detetadas.
- **Ausência na DOAJ ≠ predatório** — ausência é `nao_verificado`, não prova.
- **`--selfcheck` valida a lógica, não o mundo**: executar sobre uma lista
  real é o único teste da integridade da auditoria.

## 8. Selo

`ARKHE-FRAMEWORK-AUDITORIA-SUBSTRATO-V2-2026-09-11`