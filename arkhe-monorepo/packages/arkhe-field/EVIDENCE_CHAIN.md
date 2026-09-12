# ARKHE Field 1.0 — Cadeia de Evidências (2026)

Ancoragem constitucional do runtime nas medidas físicas e matemáticas de 2026.
Cada elo da cadeia aponta para a constante/predicado no crate e para a fonte
primária. Invariantes afetados: **Ghost-3** (referência cruzada), **Correlation-1**
(coerência entre elos), **Provenance-1** (notarização/fontes), **Gap-2**
(entropia/unidades), **Loopseal-1/2** (timestamps, append-only).

---

## 1. X(2370) como glueball 0⁻⁺ dominante — BESIII (ICHEP 2026)

| # | Evidência | Constante no crate | Fonte |
|---|-----------|--------------------|-------|
| 1.1 | M = **2370 ± 2** (stat) MeV/c² no canal π⁰π⁰η | `x2370::X_MASS_SIGNATURE_MEV = 2370.0` | arXiv:2503.13286; Nucl.Phys.B 1028:117495 (2026) |
| 1.2 | Γ = **133 ± 8** (stat) MeV (π⁰π⁰η) | `X_WITH_SIGNATURE_MEV = 133.0` | idem |
| 1.3 | J^PC = **0⁻⁺** (significância > 9.8σ com 10¹⁰ J/ψ; > 10.1σ vs. alternativas) | `SPIN_PARITY_JP`, `SPIN_PARITY_SIGNIFICANCE_SIGMA` | arXiv:2607.20366 §7; Rev.Lett. 134, 131901 (2025) |
| 1.4 | **Flavor-singlete** (primeiro hádron leve singlete acima de 1 GeV/c²) | `FLAVOR_SINGLET_MIN_MEV = 1000.0` | arXiv:2607.20366 (22 jul 2026) |
| 1.5 | Supressão `X(2370) → K*(892)⁰K̄⁰`: **B < 2.7 × 10⁻⁶** (90% CL) | `B_JPSI_GAMMA_X_TIMES_B_KSTAR_LIMIT` | idem — anúncio ICHEP 2026 (5 ago 2026) |
| 1.6 | 5 modos "dourados" PPP flavo-simétricos, sem modo dominante | `glueball::GOLDEN_PPP_MODES = 5`; `VseprGlueballCorrespondence` | arXiv:2503.13286 §8 |
| 1.7 | Produção rica em decaimentos radiativos de J/ψ (consistente com LQCD) | `X2370Evidence.jpsi_events = 10¹⁰` | arXiv:2503.13286 §9 |

**Validação**: `glueball::validate_glueball(&besiii_evidence(), 3.0)` — todas as
verificações passam dentro de 3σ e `glueball_consistent == true`.

---

## 2. Correspondência VSEPR ↔ Glueball

A camada de valência constitucional (227-F) é modelada pela geometria VSEPR
**`AX₅E₀`** (bipiramidal trigonal): cinco domínios de pares equivalentes, sem par
livre ⇒ dipolo nulo. O espelho físico é a **flavor-singlete** do glueball 0⁻⁺:
cinco modos PPP equivalentes e supressão de `K*(892)` ⇒ "dipolo de cor" nulo.

```
VSEPR AX₅E₀  : 5 domínios equivalentes + 0 pares livres → dipolo elétrico nulo
                      ‖ (isomorfia de simetria)
glueball 0⁻⁺ : 5 modos PPP + singlete de sabor           → "dipolo de cor" nulo
```

**Validação**: `VseprGlueballCorrespondence::from_glueball_evidence` ⇒
`is_coherent() == true` (teste `vsepr_correspondence_is_ax5`).

---

## 3. JUNO — Primeira medição simultânea de alta precisão (Nature 2026)

Fonte: The JUNO Collaboration, *Nature* **654**, 343–348 (2026),
DOI 10.1038/s41586-026-10538-z (cover, 10 jun 2026). 59,1 dias de dados desde
agosto de 2025; 20 kton de cintilador; 52,5 km dos núcleos de reatores.

| Constante | Valor | No crate |
|-----------|-------|----------|
| sin²θ₁₂ | **0.3092 ± 0.0087** | `juno::JUNO_SIN2_THETA12`, `JUNO_SIN2_THETA12_ERR` |
| Δm²₂₁ | **(7.50 ± 0.12) × 10⁻⁵ eV²** | `JUNO_DELTA_M21_SQ_EV2`, `JUNO_DELTA_M21_ERR` |
| Ordenamento | normal (análise) | `JunoMeasurement.normal_mass_ordering` |
| Ganho | precisão × **1.6** vs. soma de medidas anteriores | `JUNO_PRECISION_IMPROVEMENT` |
| Exposição | 59,1 dias | `JUNO_DAYS` |

---

## 4. Matriz de mistura entre camadas (análogo PMNS)

Camadas (linhas): `Quantum → Classical → Logical`; eixos próprios (colunas):
`Ghost → Loopseal → Gravity`. Usa θ₁₂ de JUNO; θ₁₃ (sin²θ₁₃ ≈ 0.02225) e
θ₂₃ (sin²θ₂₃ ≈ 0.547) de NuFIT 5.2; δ CP nominal 227°.

**Validação** (Nature 2026):
- `unitarity_residual() < 1e-9` — matriz unitária.
- `reconstruct_sin2_theta12()` retorna o valor de JUNO com 1e-9.
- `ReactorValidation.all_pass`: determina Δm²₂₁ a partir da curva de
  sobrevivência a 52,5 km e reproduz o par publicado dentro de 3σ.
- Sobrevivência estável `P(ν̄e→ν̄e)` é **insensível** a δ (protocolo cego à fase);
  e o invariante de Jarlskog `J` captura a sensibilidade a δ (ver §6).

---

## 5. Complet ação do amalgam G₃ — Monster (arXiv:2607.27256)

Critério G₃ (Goldschmidt 1980): existem A, B ⩽ G com `A ≅ B ≅ Sym₄`,
`A ∩ B ≅ Dih₈`, `O₂(A) ≠ O₂(B)` e `⟨A,B⟩ = G`.

| Empírico | Valor | No crate | Fonte |
|----------|-------|----------|-------|
| Completadores (2001) | 15 de 26 | `PARKER_ROWLEY_COMPLETERS_2001` | J.Algebra 235:131–153 (2001) |
| Não-completadores (2001) | 10 de 26 | `PARKER_ROWLEY_NONCOMPLETERS_2001` | idem |
| Caso aberto (2001) | 1 (Monster) | `OPEN_SPORADIC_BEFORE_2026` | idem |
| Monster é completador | ✓ (2026) | `COMPLETERS_2026 = 16` | arXiv:2607.27256 |
| |PSL₂(71)| | 178920, 71 | e 47 ∤ | `psl2_order(71)` | arXiv:2607.27256; Adv.Math. 469:110214 (2025) |
| Ordem do Monster | 8.08×10⁵³ | `MONSTER_ORDER_STR/F64` | padrão |

**Validação**: `verify_monster_classification().classification_complete == true`
(aritmética do submáximo + contagem da classificação) e
`GoldschmidtG3::completion_result().is_complete` com base nos critérios G₃;
`TOONChainMonster::is_monster_closed()` lacra a corrente no Monster.

---

## 6. Analogia de rotação de fase (ECDSA·MITM) — `arkhe-ecdsa-mitm`

A fase CP δ da matriz de mistura age como uma rotação de fase no espaço de
criptografia: `arkhe-ecdsa-mitm::PhaseRotation` deriva λ de δ e *blende* o digest
da assinatura ECDSA. Demonstração dupla:

- **protocolo cego à fase** (P(ν̄e→ν̄e) de reator, e ECDSA com λ = 1 = identidade)
  não detecta a rotação;
- **verificação sensível à fase** (Jarlskog J ≠ 0, e assinatura com λ ≠ 1)
  detecta a rotação — o MITM que substitui o λ esperado falha a verificação.

Teste de integração: `packages/arkhe-field/tests/phase_rotation.rs`.

---

## 7. Núcleo 3 — Sete Selos de agosto de 2026 (gap, qhttp, tzinor)

Mapeamento dos sete artigos de 12–17 de agosto de 2026 para os crates Rust do
Núcleo 3 (todos com testes e proveniência primária ancorada):

| Selo | Fonte | Cráte/módulo | Constante/Protocolo |
|:-:|:---|:---|:---|
| #1 | arXiv:2608.13997 (iluminação quântica, fase desconhecida) | `arkhe-tzinor`/`illumination` | ganho 6 dB só com fase conhecida; heteródina satura o pior caso (0 dB) |
| #2 | arXiv:2608.13907 (números extremais robustos) | `arkhe-gap`/`extremal` | D_A = 2^|A|·Tr(ρ²)−1; Qex(8,4)=56 (ε<1/5); Qex(9,4)≤120 (ε<1/17); Σ D ≥ 1 (|T|=2m+1) |
| #3 | arXiv:2608.13781 (assimetria de emaranhamento, anomalia quiral) | `arkhe-qhttp`/`coherence` | λ₂=0.9991 + δ; assimetria não nula no TL → pegada da projeção C→Z |
| #4 | arXiv:2608.12445 (PURSUE/UAP) | `arkhe-qhttp`/`anomaly` | 112 clipes; 4 parâmetros; incerteza de fase < 0.1 rad |
| #5 | arXiv:2608.14387 (QSP linearizado / UHSVT) | `arkhe-qhttp`/`uhsvt` | UHSVT com condição única f(0)=0; f = (λ₂+δ)·s |
| #6 | PRX 16, 031040 (blindagem Coulombiana em TBG) | `arkhe-gap`/`shielding` | ângulo mágico ~1.1°; escala ~2 nm; supressão completa → controle ativo do gap |
| #7 | arXiv:2608.14110 (quantum switch / ICO) | `arkhe-tzinor`/`switch` | negatividade ICO > mistura clássica; região ICO-exclusiva; ampliação por unitária Pauli local |

**Validação end-to-end**: `packages/arkhe-qhttp/tests/nucleo3_e2e.rs` fecha as
cinco camadas (Rotação τ via PMNS/JUNO → Campo ℂ λ₂ → Gap C-Z/blindagem →
qhttp UHSVT → Tzinor ICO) e ancora a integridade com a assinatura rotacionada
por δ de `arkhe-ecdsa-mitm` (MITM falha na verificação).

**Notas de mapeamento**: os módulos `arkhe-structure` (G₃/Monstro) e
`arkhe-rotation` (PMNS/JUNO) do plano Núcleo 3 já estão implementados em
`arkhe-field` (`groups.rs`, `monster.rs`, `mixing.rs`, `juno.rs`) e não foram
duplicados em crates novos.

---

## Sanidade final

```
cargo test --all --exclude arkhe-ia-dtn   # Núcleos 1 (13) + 2 (field/ecdsa-mitm) + 3 (gap/qhttp/tzinor)
clippy --all -- -D warnings --exclude ... # nenhum warning
```

> **Nota de rastreio**: o ID citado inicialmente como *arXiv:2607.27252* é, na
> verdade, **arXiv:2607.27256** (H. Dietrich, math.GR, 28 Jul 2026). Todas as
> constantes usam o ID correto.