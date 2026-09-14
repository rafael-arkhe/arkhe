# ARKHE OS — Síntese Bibliográfica (5 Papers hep-th, Ago 2026)

**Documento:** `ARKHE-SINTESE-BIBLIOGRAFICA-5-PAPERS-v1.3`
**Data:** 2026-08-11
**Status:** 📌 **AGENDADO PARA v1.3** (pós-Integration Spike)
**Base:** ARKHE-DOCS-v1.2-FINAL-2026-08-11 (CONGELADO)
**Arquivo BibTeX:** `arkhe-biblio-5-papers-2026-08.bib`

> ⚠️ **Aviso de Escopo:** ARKHE v1.2 está **congelado para implementação**. Todas as referências abaixo são **pré-prints** de 6–7 de agosto de 2026, não revisados por pares na data desta síntese. Elas **não** são dependências do roadmap de 24 semanas e devem ser incorporadas apenas na **v1.3** ou como notas de trabalho futuro. Nenhuma claim arquitetural é derivada delas sem validação independente (Princípio "Validação Web", §1.1).

---

## 1. Resumo Executivo

| # | arXiv | Título | Autores | Verificado |
|---|---|---|---|---|
| 1 | [2608.07269](https://arxiv.org/abs/2608.07269) | Tilting Mutations and Quiver-Invariant Dualities in Brane Tilings | S.-J. Lee, R.-K. Seong, B. Suzzoni | ✅ arXiv 2026-08-11 |
| 2 | [2608.06887](https://arxiv.org/abs/2608.06887) | The spectral gap of the ABJM model: A holographic perspective from uplifted higher-dimensional geometries | M. Ghodrati | ✅ arXiv 2026-08-11 |
| 3 | [2608.06605](https://arxiv.org/abs/2608.06605) | Superselected ghost theory: real spectrum | B. Holdom | ✅ arXiv 2026-08-11 |
| 4 | [2608.06465](https://arxiv.org/abs/2608.06465) | Holographic RG flows and wormholes from sinusoidal scalars | P. Leung | ✅ arXiv + cópia local `2608.06465v1.pdf` |
| 5 | [2608.07258](https://arxiv.org/abs/2608.07258) | Spectral Topology and Universal Krylov Dynamics | J. Murugan, H. J. R. Van Zyl, M. Watanabe | ✅ arXiv + cópia local `2608.07258v1.pdf` |

Todos os 5 papers são do domínio **hep-th**. Nenhum foi ainda publicado em periódico revisado por pares na data desta síntese (status de pré-print v1).

---

## 2. Prioridade de Integração

| Prioridade | Ação | Paper | Domínio ARKHE | Seção de Integração (v1.3) |
|:---:|:---|:---|:---:|:---|
| 🔴 **Alta** | Adicionar formalismo Krylov–Lanczos como framework de convergência dos estimadores de shadows | 2608.07258 | A | §2.1–2.3 (Classical Shadows / U-Statistic) |
| 🔴 **Alta** | Citar gap espectral do ABJM como analogia teórica para o threshold $p_{\text{operate}}$ | 2608.06887 | A | §2.6.4 (Threshold Operacional) |
| 🟡 **Média** | Fundamentar o princípio Fail-Closed com superseleção por paridade | 2608.06605 | C | §1.1 (Princípios Fundamentais) |
| 🟡 **Média** | Usar "boomerang RG flow" como metáfora pedagógica da calibração FPGA | 2608.06465 | B | Anexo E (Metáforas Pedagógicas) |
| 🟢 **Baixa** | Mencionar quiver-invariant dualities como metáfora para multiplicidade de shadows | 2608.07269 | A | Anexo E (Metáforas Pedagógicas) |

---

## 3. Análise por Paper

### 3.1 Paper 1 — Brane Tilings (arXiv:2608.07269)
**Relevância: Média (metafórica/estrutural)**

- Identifica **42 fases tóricas** do modelo $H_{1121}$ conectadas por dualidade de Seiberg, com **5 doublets e 1 triplet** de fases que compartilham o mesmo quiver mas possuem superpotenciais distintos.
- A noção de **"quiver-invariant dualities"** é estruturalmente análoga à ideia de múltiplas representações clássicas (classical shadows) para o mesmo estado quântico $\rho$ no Domínio A.
- As mutações de tilting invertem a orientação de zig-zag paths na região de mutação — estrutura combinatorial análoga à permutação de bases de medição no protocolo de shadows.

**Recomendação:** uso estritamente pedagógico (Anexo E). **Proibido:** derivar requisitos de implementação do pipeline de shadows a partir de dualidades de gauge.

### 3.2 Paper 2 — ABJM Spectral Gap (arXiv:2608.06887)
**Relevância: Alta (Domínio A — QPU Emulator)**

- Estuda um **gap espectral** no CFT$_3$ dual a buracos negros carregados $U(1)^4$ em supergravidade 4d. O gap separa comportamento de líquido de Fermi (dentro) de não-Fermi (fora).
- O gap surge da **não-comutatividade de dois limites** (near-horizon ↔ $q' \to 0$), criando uma descontinuidade nos potenciais químicos.
- **Aplicação direta ao ARKHE:** o threshold operacional **$p_{\text{operate}} = 0.0306 \pm 0.0001$** (§2.6.4) é um critério de engenharia análogo a esse gap. A estrutura matemática do gap (limiar onde a física muda de regime) pode enriquecer a justificativa teórica do $D_{KL} = 1.0$ bit como "switch" entre regimes.
- O uplift para 11d ($\text{AdS}_3 \times \mathbb{R}^2$, BTZ × S²) dialoga com a estrutura de dimensões do Domínio B (Hankel em geometria cilíndrica).

**Recomendação:** citação na fundamentação teórica de §2.6.4 como analogia estrutural (regime switching), não como dependência.

### 3.3 Paper 3 — Superselected Ghost Theory (arXiv:2608.06605)
**Relevância: Média-Alta (Domínio C — safe-core-policy)**

- Propõe resolver o problema de **norma negativa** em teorias com ghosts (relevante para gravidade quântica renormalizável) via **superseleção por paridade exata** $Q$ (ghost parity).
- A superseleção define setores físicos disjuntos ($Q = +1$ e $Q = -1$), garantindo probabilidades não-negativas na regra de Born.
- **Conexão com ARKHE:** o princípio **Fail-Closed** (§1.1) é uma forma de superseleção operacional: na dúvida, o sistema para (halt), não permite superposição de estados "seguros" e "inseguros". O paper de Holdom fornece fundamentação matemática para a ideia de que setores de norma/segurança oposta devem ser separados por superseleção, não por filtragem.
- A **perturbação preservando setores** (sector-preserving perturbation theory) é análoga à exigência de que o SandboxContract não permita vazamento entre domínios.

**Recomendação:** integrar como fundamentação formal do Fail-Closed na documentação de política do Domínio C (v1.3).

### 3.4 Paper 4 — Holographic RG Flows & Wormholes (arXiv:2608.06465)
**Relevância: Média (Domínio B — signal-hankel)**

- Constrói **wormholes euclidianas** em AdS sustentadas por fontes escalares sinusoidais.
- Descobre **fluxos de RG "boomerang"**: a teoria deixa o ponto fixo UV, evolui no bulk, e **retorna ao mesmo ponto fixo no IR**.
- **Conexão:** o ciclo de calibração do FPGA (§2.6) — lensing → bit-flip → $D_{KL}$ → threshold — é um fluxo operacional que "sai" do estado ideal, perturba, e "retorna" a um estado calibrado. A metáfora do "boomerang RG flow" pode ser útil no Anexo E para explicar por que o protocolo de calibração não busca um novo ponto fixo, mas retorna ao baseline com margem de ruído conhecida.
- ⚠️ **Atenção:** o paper mostra que wormholes **sempre dominam** quando existem (não há fase desconectada dominante). Isso é um aviso contra a suposição de que "geometrias desconectadas são sempre mais prováveis" — análogo ao princípio de que no ARKHE, a ausência de evidência (disconnect) não é evidência de ausência de ameaça.

**Recomendação:** metáfora pedagógica (Anexo E). **Proibido:** interpretar a dominância de wormholes como princípio físico para o pipeline de detecção de torção.

### 3.5 Paper 5 — Krylov Dynamics (arXiv:2608.07258) ⭐
**Relevância: MUITO ALTA (Domínio A + Domínio B)**

Este é o paper **mais tecnicamente próximo** do ARKHE. Estuda a **complexidade de Krylov** via coeficientes de Lanczos $\{a_n, b_n\}$ e sua conexão com a **topologia da medida espectral**.

**Resultados-chave:**
1. Espectros com cauda exponencial → crescimento linear de Lanczos ($b_n \sim \alpha n$) — "Operator Growth Hypothesis".
2. **Espectros com gap** (multi-cut) → oscilações quase-periódicas em $b_n$, com frequência fixada pela fração de preenchimento das bandas.
3. **Fechamento de gap** → transição de fase em Krylov governada pela solução de Hastings–McLeod de **Painlevé II**, com decaimento anômalo $n^{-1/3}$.
4. Modelo SYK: $b_n = \pi T \sqrt{n(n + 2/q - 1)}$.
5. Framework: formulação **Riemann–Hilbert** de polinômios ortogonais + **Deift–Zhou steepest descent**, recuperando as leis de crescimento de Freud $b_n \sim n^{1/\beta}$.

**Aplicação direta ao ARKHE:**
- O Domínio A usa **classical shadows** e estimadores U-statistic para pureza/entropia de Rényi-2. O formalismo Krylov + polinômios ortogonais + Riemann–Hilbert oferece uma ferramenta analítica para estudar a **convergência dos estimadores** e a **estrutura de correlações** em estados quânticos codificados pelo $[[5,1,3]]$.
- O Domínio B (Hankel) trabalha com **decomposição espectral** (modos $m,n$). A conexão entre topologia do suporte espectral e dinâmica de Krylov pode ser aplicada à análise de estabilidade do pipeline de métricas espectrais (pureza espectral $p$, curvatura $\tilde{R}_{ij}$).
- A **transição de gap** (Painlevé II) pode modelar o **threshold operacional $p_{\text{operate}}$** como uma transição de fase crítica no espaço de parâmetros do ruído — conectando Domínio A (threshold) e Domínio B (espectro).

**Recomendação (v1.3):**
1. Adicionar seção de revisão curta sobre Krylov/Lanczos em §2.1–2.3, com nota explícita de que é framework de análise, não dependência.
2. Investigar se os coeficientes de Lanczos do protocolo de shadows exibem a lei de Freud $b_n \sim n^{1/\beta}$ — possível métrica nova para o `NoiseBudgetPolicy`.

---

## 4. Mapeamento para o Anexo C (BOM)

O Anexo C do ARKHE v1.2 contém apenas **dependências de software/hardware** (crates, pacotes Python, FPGA). Papers de referência **não** pertencem ao BOM técnico, mas a integração de conhecimento segue o mapeamento abaixo:

| Camada | Componente v1.2 | Referência v1.3 | Natureza |
|:---|:---|:---|:---|
| Domínio A | qiskit-aer 0.14.2 | Murugan 2608.07258 (Krylov) | Framework analítico de convergência |
| Domínio A | FPGA Calibração (§2.6) | Ghodrati 2608.06887 (gap ABJM) | Analogia teórica do threshold |
| Domínio C | Fail-Closed (§1.1) | Holdom 2608.06605 (superseleção) | Fundamentação formal |
| Domínio B | Hankel / Torsion | Leung 2608.06465 (RG flows) | Metáfora pedagógica (Anexo E) |
| Domínio A | Classical Shadows | Lee 2608.07269 (quivers) | Metáfora pedagógica (Anexo E) |

**Impacto no BOM:** **Nenhum**. Nenhuma dependência de software é alterada. Se um paper justificar futuramente uma dependência (ex.: pacote de polinômios ortogonais), o procedimento de **Auditoria Contínua** (§1.1) e a validação de fonte primária devem ser aplicados antes da inclusão no Anexo C.

---

## 5. Gate de Integração (v1.3)

A integração destas referências na v1.3 fica sujeita ao seguinte gate:

1. **Post-Integration Spike (Semana 18):** concluído e aprovado.
2. **Status de publicação:** cada paper reavaliado — priorizar versões aceitas em periódicos revisados por pares.
3. **Validação independente:** claims matemáticas relevantes (ex.: leis de Freud, transição Painlevé II) verificadas por reprodução numérica antes de qualquer uso no pipeline.
4. **Manutenção do Anexo E:** toda metáfora derivada destes papers deve ser adicionada com a declaração de escopo pedagógico obrigatória.

---

## 6. Metadados Verificados

**Fonte de verificação:** páginas de abstract do arXiv (acesso em 2026-08-11) e cópias locais `2608.06465v1.pdf` e `2608.07258v1.pdf`.

| arXiv | Submetido | Páginas | Instituição(ões) |
|:---|:---|:---:|:---|
| 2608.07269 | 7 Aug 2026 | 62 | UNIST / CGP |
| 2608.06887 | 7 Aug 2026 | 36 | — |
| 2608.06605 | 6 Aug 2026 | 14 | University of Toronto |
| 2608.06465 | 6 Aug 2026 | — | University of British Columbia |
| 2608.07258 | 7 Aug 2026 | — | UCT / University of Tokyo |

**DOIs:** todos os papers possuem DOI arXiv-issued via DataCite (registro pendente): `10.48550/arXiv.<id>`.

---

## 7. Changelog

| Data | Versão | Alteração |
|:---|:---|:---|
| 2026-08-11 | v1.0 | Criação da síntese; 5 referências verificadas no arXiv; prioridades de integração definidas; mapeamento para Anexo C documentado. |

---

**Selo:** `ARKHE-SINTESE-BIBLIOGRAFICA-5-PAPERS-v1.3`  
**Status:** 📌 AGENDADO — não bloqueia implementação v1.2  
**Aviso final:** Nenhuma analogia deste documento deve ser interpretada como dependência arquitetural sem validação cruzada contra o estado real do ecossistema.
