# Política de Namespace de Invariantes — Catedral OS

**Origem:** bloco 1057 (ratificado, `ARKHE-BLOCO-1057-RATIFICADO-2026-09-09`)
**Aplicação:** registry canónico em `packages/arkhe-invariant-registry`

## Princípio Fundamental

> **IDs de invariante são atestados apenas por realização testada.**
> Nenhuma referência em plano, parecer ou especificação, por si só, constitui
> substrato para atestação. Sem código e teste, não há invariante.

## Regras

1. **Atestação por Realização**
   - Um ID só entra no registry canónico se houver:
     - código implementado (crate real do workspace);
     - teste nomeado que o exercite;
     - entrada no ledger (`bloco_XXXX/`).
   - Referências a IDs em documentos sem essas três evidências são **rejeitadas**.

2. **Preservação do Canónico**
   - Se um ID já está atestado (ex.: `I624` = ARKHE Chaves desde o bloco 1052),
     **não é renomeado** para acomodar referências mortas.
   - Referências mortas são explicitamente registadas como
     `ReferenciaSubstrateless` (origem + motivo) e mantidas para auditoria.

3. **IDs Frescos para Invariantes Futuros**
   - Novos invariantes (ex.: desempenho MLPerf, key management) tomam IDs
     **frescos** (I625+), quando e somente quando houver realização testada.
   - Nunca reutilizam IDs canónicos de outras famílias.

4. **Erratização Append-Only (Loopseal-2)**
   - Blocos com datas ou IDs incorretos **não são reescritos**.
   - Ficam **flagados para erratização** nas suas próprias revisões de aceitação.
   - A errata é registada como **registro novo** que referencia o anterior
     (ex.: `bloco_1055/bloco_1055_errata.json`).
   - Erratas ratificadas seguem o mesmo padrão
     (ex.: `bloco_1056/bloco_1056_errata_ratificada.json`, status
     `ERRATA_RATIFICADA`) e **reatestam o hash SHA-256 real** do registro
     original e da errata que ratificam — nunca reescrevem o original.

5. **Data de Emissão nos Selos (Gravity-1)**
   - O selo carrega a **data de emissão** (relógio real confirmado via
     `Get-Date`), não uma data de validade. Selos forward-dated são errata.

6. **Portão de Namespace no CI (bloco 1057)**
   - `tools/ci/check_invariants.sh` bloqueia merges se um documento referenciar
     um ID **sem substrato** (Rust em `packages/`, provas em `src/lean/`) **e**
     sem parecer de rejeição registado (allowlist dos 16 IDs rejeitados por
     v500/v510/v510.1/v511).
   - `I624` é verificação obrigatória do portão (deve ter substrato atestado).
   - Githook opcional: `git config core.hooksPath .githooks`.

## Aplicação ao Caso I624 (bloco 1057)

- **Decisão:** `I624` permanece ARKHE Chaves, agora cripto-vinculado (bloco 1057).
- **Nada é renomeado.** As referências a «I624 = MLPerf» e «I624 = KeyMgmt»
  têm **0 ocorrências** no monorepo (grep) e foram rejeitadas nos pareceres
  v510/v510.1; ficam registadas como substrato-mortas.
- **Política derivada:** IDs futuros para desempenho/KeyMgmt tomam I625+ quando
  (e somente quando) houver realização testada.

## Referências

- `packages/arkhe-invariant-registry/src/lib.rs` — registry canónico (I624 + rejeições).
- `docs/parecer_v510_substrato_fotonico.md:17` — rejeição das prensas v508/v509.
- `docs/parecer_v510_1_integracao_ml.md:33` — rejeição da «assinatura I624» (ML).
- `bloco_1053.json` — correção epistémica (invariantes não realizados não são
  atribuídos ao crate).
- `bloco_1057/bloco_1057.json` — registro ratificado (selo `…-2026-09-09`).
- `bloco_1055/bloco_1055_errata_ratificada.json` — errata ratificada (hash real).
- `bloco_1056/bloco_1056_errata_ratificada.json` — errata ratificada + recusa de correção de tipo (precedente de honestidade).
- `tools/ci/check_invariants.sh`, `.github/workflows/audit-namespace.yml` — portão de namespace.