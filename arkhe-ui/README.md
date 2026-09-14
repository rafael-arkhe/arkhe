# Arkhe UI — Fase 1 (design system)

Tokens, primitivos e verificação de contraste do design system Arkhe.
Projecto local dentro do workspace, ao lado de `arkhe-monorepo/` e
`safe-core-monorepo/`. Não é entrega de website e não toca em
`projects/projects.json`.

---

## 1. Escopo da Fase 1

**Entra:**

| Área | O que foi entregue |
|---|---|
| Tokens | Arquitectura em duas camadas (primitivos + semânticos), dark-first, tema claro opt-in |
| Tema Tailwind | Ligação via `@theme inline` (Tailwind v4, sem `tailwind.config.js`) |
| Primitivos | `Button`, `Input`, `Badge`, `Card`, `IconButton` com variantes `cva` + `data-slot` |
| Utilitário | `cn()` (clsx + tailwind-merge) |
| Storybook | 26 stories, `@storybook/addon-a11y` com `test: 'error'` |
| Testes | 34 testes (contraste WCAG + comportamento dos primitivos), cobertura 100% |
| CI | Workflow com pins exactos de actions |

**Não entra (fases seguintes):** composição de aplicação, gráficos com `d3`,
camadas Radix (`radix-ui` está instalado mas ainda não é usado), integração
com o motor de verificação, tokens de movimento/elevação, documentação
automática de props.

---

## 2. Requisitos

| | Versão |
|---|---|
| Node (CI e spec) | `24.21.0` (LTS Krypton) |
| Node (máquina local onde isto foi construído) | **`22.22.0`** |
| npm | `11.16.0` |

A divergência de Node é real e está detalhada na secção 7.
`package.json` declara `engines.node: ">=22.22.2"` (o mínimo exigido pelo
`jsdom@30.0.1`).

---

## 3. Como rodar

```bash
npm install          # instala a árvore exacta (pins, sem faixas)

# Ciclo de desenvolvimento — o Storybook é a superfície de trabalho,
# porque uma biblioteca sem index.html não tem "app" para servir.
npm run dev          # storybook dev na porta 6006  (verificado: 200, 26 stories)
npm run storybook    # alias do anterior

# Verificação
npm run typecheck    # tsc nos dois tsconfig (app + node)
npm test             # vitest run
npm run test:coverage # vitest run --coverage (thresholds aplicados)
npm run build        # vite build → dist/index.js + dist/arkhe-ui.css
npm run build-storybook

# Auditoria de contraste (imprime a tabela medida de todos os pares)
ARKHE_CONTRAST_REPORT=1 npx vitest run test/contrast.test.ts
```

### Scripts e o que realmente fazem

- `build` — build de **biblioteca** (`dist/index.js` ESM + `dist/arkhe-ui.css`).
  `react`/`react-dom` ficam externos. Não emite `.d.ts` (ver secção 7).
- `preview` — `vite preview` serve `dist/`, não o Storybook.

---

## 4. Tokens: duas camadas

`src/tokens/arkhe-tokens.css` é a fonte de verdade.

```
Camada 1 — PRIMITIVOS   --palette-*
  valores brutos, sem significado de uso. Nenhum componente os consome.

Camada 2 — SEMÂNTICOS   --surface, --panel, --control, --text-*, --brand,
                        --skill, --subagent, --warning, --danger, --status-*
  é a ÚNICA camada que os componentes consomem
```

**Regra:** um tema reatribui **apenas** a camada semântica. Os primitivos são
partilhados pelos dois temas — e isto é verificado por teste
(`os primitivos são partilhados: o tema claro só reatribui a camada semântica`).

**Dark-first:** `:root` já entrega o tema escuro. O tema claro é *opt-in* via
`[data-theme='light']`. Verificado por teste.

### Paleta Arkhe (primitivos de superfície — valores fixados na spec)

| Token | Valor |
|---|---|
| `arkhe-canvas` | `#09090b` |
| `arkhe-panel` | `#18181b` |
| `arkhe-control` | `#27272a` |
| `arkhe-border-default` | `rgba(255,255,255,0.06)` |
| `arkhe-border-strong` | `rgba(255,255,255,0.12)` |

### Acentos semânticos — hex escolhidos, e porquê

Os primitivos de acento existem em duas variantes da **mesma matiz**: uma
para superfície escura, outra para superfície clara. A escolha não foi
estética — foi forçada pelo contraste medido.

| Semântica | Uso | Tema escuro | Tema claro | Razão (escuro) | Razão (claro) |
|---|---|---|---|---|---|
| `--brand` | navegação / acção primária | `#22d3ee` | `#0e7490` | 11.01:1 s/ canvas | 5.13:1 s/ canvas |
| `--skill` | skills | `#2dd4bf` | `#0f766e` | 9.52:1 s/ painel | 5.47:1 s/ painel |
| `--subagent` | sub-agentes | `#a78bfa` | `#6d28d9` | 6.51:1 s/ painel | 7.10:1 s/ painel |
| `--warning` | aviso / parcial | `#fbbf24` | `#9a4a08` | 10.61:1 s/ painel | 6.26:1 s/ painel |
| `--danger` | destrutivo / erro | `#fb7185` | `#be123c` | 6.58:1 s/ painel | 6.29:1 s/ painel |

Critério de escolha:

1. No **escuro** os acentos são tons claros e saturados (`-400`): sobre
   `#09090b`/`#18181b` é preciso luminância alta para passar 4.5:1.
2. No **claro** são tons profundos (`-700`/`-800`): sobre branco é preciso
   luminância baixa. Usar o mesmo `#22d3ee` sobre branco daria ~1.6:1 e
   falharia.
3. **Correcção feita por medição:** `--text-muted` começou em `#71717a`, que
   dá **3.67:1** sobre `--panel` — abaixo de 4.5:1. Foi escurecido para
   `#94949e` (5.90:1 sobre painel, 4.96:1 sobre controlo). Sem o teste de
   contraste isto teria passado despercebido.

### Independência do Tailwind

A paleta é **própria do projecto**. Não é a escala `zinc-*` nem qualquer
escala upstream: está declarada em `:root` como valores literais e existe
mesmo que o Tailwind desapareça. Um teste falha se algum primitivo
referenciar `zinc` ou `var(--color-*)`.

A ligação ao Tailwind acontece **só no fim do ficheiro**, em `@theme inline`:

```css
@theme inline {
  --color-surface: var(--surface);
  --color-fg:      var(--text-primary);
  /* … */
}
```

`inline` faz o Tailwind emitir `var(--surface)` no utilitário em vez de copiar
o valor — logo os utilitários **acompanham a troca de tema**.

Nota de nomenclatura: no Tailwind v4 o namespace `--text-*` é o de
`font-size`, por isso os tokens de cor de texto (`--text-primary`) são
projectados no namespace `--color-*` como `--color-fg*` → daí `text-fg`,
`text-fg-muted`, `text-fg-subtle`.

---

## 5. Primitivos

Todos com `data-slot`, variantes via `cva`, e override de `className` passado
por `cn()` (o `className` do consumidor vence via tailwind-merge).

| Componente | Variantes | Acessibilidade |
|---|---|---|
| `Button` | `primary` / `secondary` / `ghost` / `destructive` · `sm`/`md`/`lg`/`icon` | `focus-visible` ring, `disabled` |
| `IconButton` | as mesmas 4 · `sm`/`md`/`lg` | `aria-label` **obrigatório por tipo**; alvo ≥24px (SC 2.5.8) |
| `Input` | `default` / `strong` · `sm`/`md`/`lg` | `aria-invalid` estilizado (não só cor) |
| `Badge` | `verified` / `partial` / `pending` / `error` | estado é textual, não só cromático (SC 1.4.1) |
| `Card` | `panel` / `surface` / `interactive` · paddings | `interactive` mostra `focus-within` |

Decisões deliberadas:

- `Button`/`IconButton` usam `type="button"` por omissão, para evitar submits
  acidentais dentro de formulários. Sobrescrevível com `type`.
- `Input` **não** inventa nome acessível: o consumidor tem de dar um
  `<label htmlFor>` ou `aria-label`. Inventá-lo produziria um nome enganador.
- `--radius-control` / `--radius-panel` são tokens, não valores soltos.

---

## 6. Acessibilidade: o teste de contraste

`test/contrast.test.ts` **lê `src/tokens/arkhe-tokens.css`** (a fonte de
verdade), resolve a cadeia de `var(--…)` por tema, e calcula a razão de
contraste WCAG a partir da luminância relativa
(`(L_claro + 0.05) / (L_escuro + 0.05)`).

Falha se algum par ficar abaixo de:

- **4.5:1** — texto normal (SC 1.4.3)
- **3:1** — texto grande / elementos não textuais (SC 1.4.11, anéis de foco,
  indicadores de estado)

Pares verificados: **28 por tema × 2 temas = 56**. O mínimo medido é 4.95:1
(badge `verified` no tema claro) e o anel de foco dá 11.01:1 no escuro.

### Esta é a cobertura, e o limite dela

> **O teste cobre o critério de contraste de cor — e apenas esse critério.**

Não é conformidade WCAG 2.2 AA. Não verifica, entre outros: nomes
acessíveis em geral, ordem de foco, gestão de foco em modais, semântica de
landmarks, tamanho de alvo fora dos componentes testados, movimento,
reflow/zoom, texto alternativo, ou estrutura de cabeçalhos. A meta
"WCAG 2.2 AA" **continua a ser intenção** nas restantes dimensões: o que esta
fase converteu em verificação executável foi só o contraste.

O Storybook tem `@storybook/addon-a11y` com `test: 'error'` e três regras
`axe-core` reais activadas (`color-contrast`, `focus-order-semantics`,
`tabindex`). Essas regras **só correm quando as stories são executadas** — o
que exige o caminho de teste de stories, que está bloqueado (secção 7).

### Lacuna conhecida: contornos de controlo (SC 1.4.11)

Medido, na paleta escura aprovada:

| Par | Razão | Mínimo | Estado |
|---|---|---|---|
| `--control` vs `--surface` | **1.34:1** | 3:1 | **abaixo** |
| `--control` vs `--panel` | **1.19:1** | 3:1 | **abaixo** |

A fronteira de um `Input` em repouso **não cumpre** 1.4.11: nem o fundo
`#27272a` sobre `#09090b`, nem os contornos com alfa `0.06`/`0.12` chegam a
3:1. (Para chegar a 3:1 sobre o canvas o contorno teria de rondar `#71717a`.)

O **estado** de foco, esse, cumpre (11.01:1 via `--focus-ring`), e há um teste
a garantir isso.

Isto está fixado por teste que **regista** a lacuna em vez de a esconder:
se a paleta mudar, o teste obriga a revisitar a decisão. Fica a requerer uma
decisão de design sobre a paleta da Fase 1 — as opções são reforçar os
contornos (visivelmente mais claros) ou aceitar e documentar a excepção.

---

## 7. Não feito / não verificado

Lista honesta. Nada aqui foi omitido por conveniência.

1. **Browsers do Playwright NÃO estão instalados** (centenas de MB).
   `playwright.config.ts` (chromium/firefox/webkit) e `e2e/smoke.spec.ts`
   existem, mas **nenhum teste e2e foi alguma vez executado**.
   Para instalar: `npx playwright install` (e `npm run build-storybook`
   antes, porque a suite serve `storybook-static/`).
2. **`@storybook/addon-vitest` NÃO está instalado — bloqueio real.**
   A spec fixa `@storybook/addon-vitest@10.6.0` **e** `vitest@5.0.0`, mas o
   addon declara `peer vitest: ^3.0.0 || ^4.0.0`. São incompatíveis:
   `npm install` falha com `ERESOLVE`. Confirmado que é o **único** conflito
   (todo o resto instala limpo). Adicionalmente o addon exige
   `@vitest/browser` + browsers do Playwright, que esta fase não instala.
   Consequência: `npx vitest run --project=storybook` →
   `Error: No projects matched the filter "storybook"` (exit 1).
   O passo está comentado em `.github/workflows/ci.yml` com o motivo.
   Decisão tomada com o responsável: **adiar** o addon e manter `vitest@5.0.0`,
   em vez de forçar com `--legacy-peer-deps`.
3. **Fase 5 bloqueada: `arkhe-verify-wasm` não existe.** Verificado por busca
   no workspace (nomes de directórios e referências em ficheiros de texto):
   nenhuma ocorrência. Sem esse componente não há integração de verificação
   a ligar.
4. **ABC Diatype é uma fonte comercial licenciada** e **não está incluída**
   neste repositório. `--font-sans` declara-a em primeiro lugar mas cai para a
   stack de sistema (`ui-sans-serif`, `system-ui`, …) até a licença ser
   adquirida. Nenhuma verificação visual foi feita com a fonte real —
   portanto **as métricas tipográficas não foram validadas**.
5. **O teste de contraste cobre apenas o critério de contraste de cor da
   WCAG** — não é conformidade WCAG 2.2 AA (detalhado na secção 6).
6. **O workflow de CI provavelmente NÃO corre onde está.** O GitHub Actions só
   lê workflows em `.github/workflows/` **na raiz do repositório**. O ficheiro
   foi criado em `arkhe-ui/.github/workflows/ci.yml` conforme a estrutura
   pedida, por isso **não será executado** enquanto não for movido para a raiz.
   Não foi movido porque a tarefa proíbe alterações fora de `arkhe-ui/`.
7. **Nenhum `.d.ts` é emitido.** `npm run build` produz JS + CSS, mas não
   declarações de tipos; `package.json` não declara o campo `types` para não
   prometer um ficheiro que não existe. Consumidores TypeScript não terão
   autocomplete tipado até isto ser ligado.
8. **`radix-ui` e `d3` estão instalados mas não são usados.** Foram pedidos
   pela spec; ficam disponíveis para fases seguintes. Nenhuma linha de código
   depende deles.
9. **`storybook-addon-design-system-docs@1.0.2` está declarado mas não
   registado** em `.storybook/main.ts`. É um addon da comunidade (publicado
   em 2026-01) que gera docs a partir de config do Tailwind; como o v4
   dispensa `tailwind.config.js`, registá-lo era risco sem benefício nesta
   fase.
10. **Documentação automática de props não está configurada** (sem
    `@storybook/addon-docs`), pelo que não há páginas de props geradas.
11. **Node local é 22.22.0, abaixo do `engines` do `jsdom@30.0.1`**
    (`^22.22.2`). Com `engine-strict: false` o npm não avisa nem bloqueia.
    Verificado que o jsdom **carrega e funciona** nesta versão (os testes de
    primitivos correm em jsdom e passam). Ainda assim é uma divergência
    nominal, e o CI (`24.21.0`) está dentro da faixa.
12. **`npm audit` reporta 5 vulnerabilidades moderadas** na árvore
    transitiva da toolchain. Não foram investigadas nem corrigidas —
    `npm audit fix --force` implicaria alterações de versões fora dos pins.
13. **Alvos mínimos, ordem de foco e navegação por teclado foram testados
    apenas ao nível de unidade** (classes e atributos), não em browser real.
    Sem os browsers do Playwright não houve verificação de comportamento
    efectivo de foco/teclado.

---

## 8. Estado verificado

Comandos executados nesta máquina (Node 22.22.0, npm 11.16.0, Windows):

| Comando | Exit |
|---|---|
| `npm install` | 0 |
| `npx tsc --noEmit -p tsconfig.json` | 0 |
| `npx tsc --noEmit -p tsconfig.node.json` | 0 |
| `npm run build` | 0 |
| `npx vitest run` | 0 — 34 testes, 2 ficheiros |
| `npx vitest run --coverage` | 0 — 100% stmts/branch/funcs/lines |
| `npx storybook build` | 0 — 26 stories indexadas |
| `npm run dev` (probe HTTP) | 200, 26 stories |
| `npm ls --depth=0` | 0 — todas as versões iguais aos pins |
| `npx vitest run --project=storybook` | **1** — bloqueado (ver secção 7.2) |

### Versões instaladas

Todas as versões pedidas pela spec foram instaladas **exactamente** na versão
pinada — sem `latest` e sem faixas. A única ausente é
`@storybook/addon-vitest@10.6.0`, por incompatibilidade de peer (secção 7.2).

Três pacotes foram Acrescentados por necessidade técnica, não por escolha:
`@storybook/react-vite@10.6.0` e `@storybook/react@10.6.0` (exigidos por
`.storybook/main.ts` / `preview.ts` — não constavam da lista),
`class-variance-authority@0.7.1` (pedido na spec como "versão estável actual"),
e `@types/react@19.3.0` / `@types/react-dom@19.3.0` / `@types/node@22.20.2`
(sem eles o TypeScript não compila JSX react nem ficheiros de config).
