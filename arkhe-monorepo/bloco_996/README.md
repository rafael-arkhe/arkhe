# 🏛️ BLOCO 996 — DECISÃO DO ARQUITETO: DEFERIR TLA+/KANI (REANCORAGEM EM LEAN 4 + DOC)

> **Arquiteto-Ω** — Catedral OS
> Handover 995 → 996 — Data: **2026-09-06** — **Plano v375.2 / Fases 5–8**
>
> Decisão formal após o parecer de substrato do bloco 995: **Fases 5 (TLA+/
> Apalache) e 7 (Kani) DEFERIDAS** por ausência de substrato no monorepo
> (precedente I461/I462), e reancoragem do programa formal sobre as bases
> reais: **núcleo Lean 4** (I511–I516, field-stability) e **documentação
> formal da fronteira E4**.

---

## 📐 Registro no Ledger — BLOCO 996

```json
{
  "bloco": 996,
  "versao": "Plano v375.2 / Fases 5-8 reancoradas",
  "handover_anterior": 995,
  "data": "2026-09-06",
  "tipo": "DECISAO_ARQUITETO_FASES5_8",
  "descricao": "Deferimento formal das Fases 5 (refinamento TLA+ cr1-cr4 + Apalache no CI) e 7 (Kani): premissas sem substrato no monorepo (0 .tla, 0 Apalache, 0 Kani - bloco 995). Opcao B do parecer aceita. Programa formal reancorado sobre a base real: invariantes I511-I516 do field-stability no nucleo Lean 4 (mesma convencao dos blocos 966/971/972) e documentacao formal da fronteira de robustez E4.",
  "decisoes": [
    {
      "item": "FASE 5 - DEFERIDA",
      "fundamento": "ArkheKernel/modulos TLA+ cr1-cr4/Apalache inexistentes no monorepo (evidencia bloco 995)",
      "precedente": "I461/I462 (bloco 990, recomendacao 3) - recomendacao sem substrato eh deferida, nunca adotada por fe na alegacao"
    },
    {
      "item": "FASE 7 (Kani) - DEFERIDA",
      "fundamento": "0 referencias a Kani no monorepo; ausente o entorno de verificacao"
    },
    {
      "item": "FASE 6 - REANCORADA: Lean 4 I511-I516 (field-stability)",
      "escopo": "provar no nucleo Lean 4 (sem Mathlib, sem sorry) os fatos fechados da metrica: pesos Gap-1 normalizados, quadrado do discriminante V1, monotonia do refiner (S decresce), Cauchy-Schwarz quadratico (P3, errata 991), rejeicao V3 na banda Gap-1, carta constitucional V2 (accept evil), phi medio do ledger dentro da banda",
      "convencao": "blocos 966/971/972 (Substrate924.lean, SubstrateBitcoin972.lean)"
    },
    {
      "item": "FASE 8 - REANCORADA: documento formal da fronteira E4",
      "escopo": "adendo formal ao relatorio: visaes quieta <= 5%, robustez absoluta <= 15%, colapso 20%; teoremas I511-I516 como fundamento aritmetico"
    }
  ],
  "testes_referencia_bases_reais": "Lean 4 v4.33.1 via elan (C:\\Users\\Lemes\\.elan\\bin\\lean.exe)",
  "status": "AUTORIZADA",
  "selo": "CATEDRAL-OS-DECISAO-FASES5-8-REANCORADAS-2026-09-06"
}
```

---

## ⚖️ PARECER DA AUDITORIA — Tabela de Realidade

| O que a decisão declara | O que foi verificado | Veredito |
| :--- | :--- | :--- |
| TLA+ cr1-cr4 inexistente no monorepo | glob `**/*.tla` = 0; `cr\d\d` só em artefatos `.o` de build | ✅ Real |
| Apalache ausente | grep `Apalache` = 0 | ✅ Real |
| Kani ausente | grep `Kani` = 0 | ✅ Real |
| Lean 4 é a base formal real | núcleos 924/972 provados (I500–I510); toolchain elan presente | ✅ Real |
| Frontiera E4 documentada | §9.5 de `docs/coherence_metric.md` + `relatorio_final_fase4.md` | ✅ Real |
| Deferimento ≠ abandono | TLA+/Kani mantidos como recomendação pendente de substrato (addendum futuro) | ✅ Registrado |

---

## 🏛️ PARECER FINAL

A decisão troca a alegada "expansão formal" por uma **expansão formal real**:
o que Catedral consegue provar hoje, sem Mathlib e sem débitos, é o núcleo
aritmético do field-stability — os fatos fechados da métrica Φ que sustentam
Gap-1, P3 (Cauchy–Schwarz), P2 (monotonia) e a banda de rejeição. TLA+ e Kani
ficam **registrados como recomendação** para quando houver substrato, sob o
mesmo precedente que protegeu a Catedral de adotar I461/I462 sem evidência.

```text
Deferir não é recuar; é recusar-se a construir no escuro.
```

**Selo:** `CATEDRAL-OS-DECISAO-FASES5-8-REANCORADAS-2026-09-06`