# ARKHE — Notas Técnicas do Pipeline Bayesiano M2+ v2.2

Documentação técnica complementar ao `arkhe_bayes_v22.py`. Este arquivo consolida
as decisões de modelagem física e estatística do pipeline de ressonância
plasma-vácuo QED em magnetares.

---

## 1. f_FF — Correção de Campo Finito (Abbassi et al. 2026)

O fator `f_FF` corrige a birrefringência QED para campos `B ≳ B_c`
(`B_c = 4.414e13 G`). A versão v2.2 usa uma **abordagem de três camadas**:

1. **Valor verificado** (`VERIFIED_FF_DATA`): tabelado do abstract de
   Abbassi, Chishtie & Valluri (2026, arXiv:2607.06422) para
   1E 1547.0-5408, 1RXS J1708-4009 e SGR 1806-20.
2. **Interpolação fenomenológica**: polinômio em `B/B_c` ajustado aos três
   pontos verificados, usado **apenas** para `B` dentro do intervalo tabulado.
3. **Valor do usuário** (`user_f_FF`): prioridade máxima.

### Propagação de incerteza

A literatura não publica incertezas para `f_FF`. O pipeline adota estimativas
conservadoras por status (`FF_SIGMA_BY_STATUS`):

| Status | σ_f |
|--------|-----|
| `verified` | 0.01 |
| `interpolated` | 0.05 |
| `extrapolated` | 0.15 |

Na inferência, `f_FF` entra como **parâmetro de nuisance com prior Gaussiano**
`N(f_FF_ref, σ_f)` e escala a profundidade do dip via `f_FF/f_FF_ref`. Assim a
incerteza de `f_FF` é propagada à posteriori e à evidência — mais conservador
do que um valor pontual fixo. Note que um termo de penalidade adicionado
diretamente na log-likelihood (como proposto na análise ARKHE-M2PLUS) é
**matematicamente equivalente** ao prior Gaussiano quando `f_FF` é amostrado.

---

## 2. Borel-Padé — REMOVIDO na v2.2

O Borel-Padé para a birrefringência `Δn` requer derivadas parciais da ação
efetiva em relação aos invariantes de campo `F` e `G`. A implementação
v2.0/v2.1 aplicava Padé **diretamente aos coeficientes de `L_eff`** (que
começam em `x⁴`), o que é incorreto: `Δn` começa em `x²`.

A v2.2 **remove** o Borel-Padé. No lugar, usa:
- valores verificados de Abbassi et al. 2026 quando disponíveis;
- interpolação fenomenológica;
- valor especificado pelo usuário.

Uma implementação especializada do Borel-Padé (derivadas de `L_eff(F,G)`)
fica para trabalho futuro.

---

## 3. Normalização do Perfil de Voigt

`voigt_profile(E, E_res, sigma, gamma)` é normalizado **em área**
(`∫V dE = 1`), usando `Re[wofz]/(sigma·sqrt(2π))`. A altura de pico é
aproximadamente `1/(sqrt(2π)·σ_eff)`, o que garante que a profundidade do dip
(`dPD_dip · eta · angular_factor · f_FF_scale`) seja interpretável como
depleção total de polarização integrada na ressonância, independentemente da
largura.

---

## 4. Escala de Jeffreys (1961) e Kass & Raftery (1995)

`interpret_bayes_factor` classifica `ln BF` na escala **log10** de Jeffreys:

| log10 BF | Evidência |
|----------|-----------|
| < 1.0 | Inconclusiva |
| 1.0–2.5 | Fraca |
| 2.5–5.0 | Substancial |
| 5.0–10.0 | Forte |
| > 10.0 | Decisiva |

A função retorna `(rótulo, log10_BF)` para transparência total. As entradas
`logZ` do dynesty já estão em logaritmo natural; a conversão é
`log10_BF = |ln_BF|/ln(10)`.

---

## 5. Reproducibilidade (Seed no dynesty)

Todos os `NestedSampler` recebem `rng = np.random.default_rng(seed)`. O mesmo
`seed` (default 42) produz resultados idênticos entre execuções. As três
evidências (H1, H1.5, H2) usam `seed`, `seed+1`, `seed+2` para independência.
Para estudos de convergência, varie o `seed` e compare `logZ` dentro de
`±3·logZ_err`.

---

## 6. `phase_averaged` e dados fase-resolvidos

- `phase_averaged=True` (padrão): a dependência angular RVM é integrada
  numericamente sobre `φ ∈ [0, 2π]`; o modelo é o mesmo para todos os pontos.
- `phase_averaged=False`: o modelo avalia `PD(E, φ)` ponto a ponto usando o
  `phi` de cada evento/bin (requer dados fase-resolvidos, e.g. NICER + IXPE
  com folding de período).

Para dados fase-resolvidos, o termo `angular_factor = sin²θ(φ)` modula o dip
diretamente na fase, o que aumenta o poder discriminante do modelo H2.

---

## 7. `dlogz` e custo computacional

`dlogz=0.01` (default) é o critério de convergência mais conservador
(recomendado para a execução final). `dlogz=0.1` custa ~2–3× menos e é
suficiente para triagem. O pipeline (`arkhe_pipeline_full.py`) aceita
`--dlogz` para ajuste fino.

---

## 8. Validação de domínio na likelihood

`_domain_ok` rejeita propostas fora do domínio físico antes da avaliação do
modelo (`-inf`): `eta ∈ [0,1]`, `E_res ∈ (0,20] keV`, `sigma, gamma > 0`,
`dPD_dip ∈ [0,0.5]`, `f_sigma > 0`, `f_FF > 0`, `alpha ∈ (0,π)`,
`beta ∈ (−π/2, π/2)`. Isso evita erros numéricos (ex.: divisão por zero na
Voigt) e não interfere na cobertura dos priors.
