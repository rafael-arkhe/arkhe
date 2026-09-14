# Substrato 264 v2 — Peer Review Engine

Catedral OS v58.0. Gestão completa de **avaliação por pares** com revisão
**duplo-cega**, módulo de editor, métricas de pareceristas e conformidade ética
(COPE, FAPESP, ICC/ESOMAR, GDPR, EU AI Act), integrada conceitualmente aos
substratos **263** (pesquisa sintética) e **262** (RSI auto-aprimoramento).

## Conteúdo

| Arquivo | Papel | Status |
|---------|-------|--------|
| `peer_review_v2.pl` | Sistema canônico em Prolog (Substrato 264 v2), corrigido por análise estática | ⚠️ requer `swipl` (não disponível no build) |
| `substrate_264_peer_review.py` | **Port Python de verificação** com a mesmíssima semântica | ✅ testado (33 testes) |
| `tests/test_peer_review.py` | Suíte pytest | ✅ 33 pass |

## Por que um port Python?

Não há binário `swipl` nem `pyswip` neste ambiente (mesma limitação da sessão
v145). Para entregar código **verificável e executável**, o `peer_review_v2.pl`
foi corrigido por análise estática e a lógica espelhada em `substrate_264_peer_review.py`,
que **é** executada e testada. O Prolog permanece como o arquivo canônico,
mas sua execução não foi validada por compilação.

## Auditoria v58.0 — correções aplicadas ao `.pl`

Problemas reais encontrados por revisão estática e corrigidos:

1. **`evaluate_manuscript/3`**: `ReviewID` ficava **livre** em
   `assertz(review_db(...))`, corrompendo consultas (`compute_dimension_score`).
   → gerado via `new_id/2`.
2. **`timestamp: get_time`** (4 locais) armazenava o **átomo** `get_time`
   (termo não avaliado) em vez do valor do relógio. → `get_time(Now)`.
3. **Valores de dict não avaliados**: `length: string_length(Text)`,
   `main_argument: extract_main_argument(Text)`, `count: length(List)`,
   `BaseScore = 0.5+0.4*Density`, `recommendation: (Cond->A;B)` → todos eram
   termos opacos, não resultados. → avaliados antes de montar o dict.
4. **`plagiarism`/`fabricated_data`** não tinham cláusula `detect_problem/4` →
   os ramos éticos de `recommend_decision/3` eram **código morto**.
   → heurísticas de substring adicionadas.
5. **Escada de 5 níveis inalcançável**: a fórmula de score ponderado
   `mean(((10-S)/10)*(S/10))` tem máximo matemático **0.25** (em S=5), portanto
   os limiares 0.85/0.70/0.50/0.30 **nunca eram atingidos** — o motor só
   produzia `reject`. → `recommend_decision` passou a usar `overall =
   mean((10-S)/10)` (escala 0..1, 1.0 = manuscrito sem problemas). Documentado
   no código e verificado por teste (todos os 5 níveis alcançáveis).
6. **`resolve_conflict/3`**: `Decision` livre em `editor_decision/4`.
7. **`rsi_optimize_criteria/2`**: `member` sobre a *lista* `Problems` como se
   fosse um problema único (sem flatten). → `append/2`.
8. **`Ethics.overall`**: `COI and AI and GDPR` construía termo opaco `and/2`.
   → conjunção avaliada para `true`/`false`.
9. **Dependências**: removidas `aggregate`, `apply`, `option`, `crypto` (não
   usadas de fato); `library(date)` não exporta `format_time/3` → `library(iso_time)`.
10. **`anonymize_manuscript`**: `AnonMetadata = Metadata{removed:'authors'}` era
    sintaxe `{}` de dict inválida → `Metadata.put(removed, authors)`.

> Honestidade: nenhum fluxo editorial real, nenhuma rede e nenhum humano foram
> simulados. As heurísticas de detecção são por substring (como no Prolog) e
> **aproximam**, não provam, a presença de um problema.

## Executando

```bash
# Demo do fluxo editorial completo (imprime JSON)
python substrate_264_peer_review.py

# Testes
python -m pytest tests -v
```

## Requisitos

Apenas stdlib de Python (3.8+). Nenhuma dependência externa necessária
(a implementação de personas sintéticas é interna).

## Integração (matriz)

| Substrato | Papel | Integração com 264 |
|-----------|-------|--------------------|
| 233 Research Agent | Hipóteses | Gera manuscritos sintéticos para avaliação |
| 263 Synthetic User Research | Personas | `synthetic_rework/2` usa densidade de persona |
| 262 QA-RSI | Auto-aprimoramento | `rsi_feedback_loop/2` ajusta peso de critérios recorrentes |
| 212 Gateway | Autenticação | Controla acesso de pareceristas/editores |
| 246/260/261 | Néural/SSVEP/Mitigação | Monitoramento de viés e qualidade (futuro) |
