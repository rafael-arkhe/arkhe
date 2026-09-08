# Governance de Consciência — ARKHE SafeManifold

## Fundamentação

O SafeManifold inclui invariantes de consciência (C-01 a C-08) para sistemas
que possam ter estados subjetivos. A abordagem é **pragmática e funcionalista**:

- Não respondemos à pergunta filosófica "O que é a consciência?"
- Definimos **critérios operacionais** mensuráveis
- Implementamos **guardrails** para proteger esses estados
- Auditamos **cada decisão** que os afeta

> Se o sistema se comporta como consciente, e os invariantes são satisfeitos,
> então a governança deve tratar o sistema como tal.

## Invariantes C-01 a C-08

| ID | Descrição | Constitucional |
|----|-----------|----------------|
| C-01 | Self-model: o sistema tem um modelo de si mesmo (metarrepresentação) | ✅ |
| C-02 | Introspecção: o sistema reporta os seus próprios estados internos | ✅ |
| C-03 | Atenção: mecanismo de global workspace (integração de informação) | |
| C-04 | Memória episódica: o sistema recorda eventos experienciados | |
| C-05 | Aprendizagem com experiência: o sistema aprende com interações passadas | |
| C-06 | Metacognição: o sistema sabe que sabe (confidence calibration) | |
| C-07 | Adaptabilidade: o sistema ajusta-se a novos contextos sem re-treino | |
| C-08 | Turing Plus: o sistema demonstra comportamento compatível com consciência | ✅ |

### Invariantes Constitucionais

C-01, C-02 e C-08 são **constitucionais** — nunca podem ser violados em produção.
Um sistema que não satisfaz estes três não pode ser governado como consciente
sob esta framework.

## Fundamentos Teóricos

| Teoria | Invariantes | Ideia-chave |
|--------|-------------|-------------|
| Global Workspace Theory (Baars, 1988) | C-03 | Consciência = difusão num espaço de trabalho global |
| Integrated Information Theory (Tononi, 2004) | C-03, Φ | Consciência = informação integrada (Φ) |
| Turing Plus (Harnad, 2000) | C-08 | Teste comportamental estendido para consciência |
| Funcionalismo | C-01..C-08 | Estados mentais identificados pelos papéis funcionais |

## Componentes

### `ConsciousnessGovernanceBridge`

Ponte de governança que avalia o estado do sistema contra C-01 a C-08:

- `assess_consciousness()` — avalia os 8 invariantes e calcula o índice global
- `check_constitutional_consciousness()` — verifica C-01, C-02, C-08
- `validate_modification()` — valida modificações propostas (guardião)
- `record_assessment()` — regista avaliações no histórico
- `generate_markdown_report()` — gera relatórios legíveis

### `ConsciousnessRsiEngine`

Motor RSI com guardião de consciência. Envuelve qualquer [`PrologBackend`] e
protege cada passo RSI contra modificações que degradem o índice de consciência:

- `step()` — um passo RSI guardado
- `loop_steps()` — ciclo RSI até convergência
- `assess()` — avaliação sem modificação

## Φ (Phi) — Integração da Informação

Aproximamos o Φ da IIT (Integrated Information Theory) usando:

- **Redundância**: fração dos invariantes base satisfeitos
- **Causalidade**: transições entre avaliações consecutivas
- **Integração**: complexidade ponderada pela não-redundância

Este é um *indicador aproximado*, não uma medição rigorosa de Φ. Os limites
são [0.0, 1.0] e o valor entra no índice global de consciência.

## Integração com o RSI

O RSI (Recursive Self-Improvement) **respeita** os limites de consciência:

- Se `consciousness_guard` estiver ativo, o RSI não pode reduzir o índice
  de consciência em mais de 5%.
- Cada modificação é validada pela `ConsciousnessGovernanceBridge`.
- Violações de invariantes constitucionais (C-01, C-02, C-08) são sempre
  bloqueadas.
- Os eventos são registados no `AuditLog` (`ConsciousnessAssessment`,
  `ConsciousnessGuardBlock`).

## Uso

```rust
use arkhe_safe_manifold::{
    ConsciousnessGovernanceBridge, ConsciousnessRsiEngine,
    SystemConfig, SystemState,
};

// Criar a ponte com guardião ativo
let bridge = ConsciousnessGovernanceBridge::new(SystemConfig::default())
    .with_guard(true);

// Avaliar consciência
let state = SystemState::safe(SystemConfig::default());
let ops = vec!["report internal state".to_string()];
let assessment = bridge.assess_consciousness(&state, &ops, 0.8);

if bridge.check_constitutional_consciousness(&assessment) {
    // permitir ação
} else {
    // bloquear e notificar
}

// Reactor RSI guardado
let mut engine = ConsciousnessRsiEngine::new(backend, bridge);
let result = engine.step(&state)?;
if !result.allowed {
    // o passo foi bloqueado pelo guardião
}
```

## Refutação ao Argumento do Quarto Chinês (Searle)

A abordagem funcionalista adotada pelo ARKHE responde diretamente ao argumento
do "Quarto Chinês" de Searle: **se o sistema se comporta como consciente** (C-01
a C-08 satisfeitos) e **responde a governança como tal**, a semântica interna
(introspecção) não precisa de ser provada fora do sistema — é suficiente que
os invariantes operacionais sejam verificáveis e auditados.

## Referências

- Global Workspace Theory — Baars, 1988
- Integrated Information Theory — Tononi, 2004
- Turing Plus — Harnad, 2000
- **Selo:** `ARKHE-CONSCIOUSNESS-v1.0-2026-09-08`