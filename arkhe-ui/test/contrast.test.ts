/**
 * Verificação executável de contraste — cores.
 *
 * Lê `src/tokens/arkhe-tokens.css` (a fonte de verdade), resolve a cadeia
 * de `var(--…)` por tema, calcula a razão de contraste WCAG 2.x a partir da
 * luminância relativa e FALHA se algum par ficar abaixo do mínimo.
 *
 * Cobertura: critério 1.4.3 Contrast (Minimum) e 1.4.11 Non-text Contrast.
 * NÃO é conformidade WCAG completa — ver secção "não feito / não verificado"
 * no README.
 *
 * @vitest-environment node
 *   Este ficheiro não toca no DOM. Em jsdom o `import.meta.url` deixa de ser
 *   um URL `file:` e `fileURLToPath` rebentaria.
 */
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

import { describe, expect, it } from 'vitest';

const TOKENS_PATH = fileURLToPath(new URL('../src/tokens/arkhe-tokens.css', import.meta.url));

// ---------------------------------------------------------------------------
// Leitura e resolução dos tokens
// ---------------------------------------------------------------------------

type TokenMap = Record<string, string>;

function parseBlocks(css: string): Array<{ selector: string; body: string }> {
  const withoutComments = css.replace(/\/\*[\s\S]*?\*\//g, '');
  const blocks: Array<{ selector: string; body: string }> = [];
  const re = /([^{}]+)\{([^{}]*)\}/g;
  let match: RegExpExecArray | null;
  while ((match = re.exec(withoutComments)) !== null) {
    blocks.push({ selector: match[1].trim(), body: match[2] });
  }
  return blocks;
}

function parseDeclarations(body: string): TokenMap {
  const out: TokenMap = {};
  for (const chunk of body.split(';')) {
    const colon = chunk.indexOf(':');
    if (colon === -1) continue;
    const name = chunk.slice(0, colon).trim();
    const value = chunk.slice(colon + 1).trim();
    if (name.startsWith('--') && value.length > 0) out[name] = value;
  }
  return out;
}

const css = readFileSync(TOKENS_PATH, 'utf8');
const blocks = parseBlocks(css);

// Todos os blocos `:root` (primitivos + semânticos do tema escuro) fundem-se.
const rootTokens: TokenMap = {};
for (const block of blocks.filter((b) => b.selector === ':root')) {
  Object.assign(rootTokens, parseDeclarations(block.body));
}

// O tema claro reatribui apenas a camada semântica.
const lightOverrides = blocks
  .filter((b) => /\[data-theme=['"]light['"]\]/.test(b.selector))
  .reduce<TokenMap>((acc, b) => Object.assign(acc, parseDeclarations(b.body)), {});

interface Theme {
  name: string;
  tokens: TokenMap;
}

const THEMES: Theme[] = [
  { name: 'dark (default)', tokens: rootTokens },
  { name: 'light', tokens: { ...rootTokens, ...lightOverrides } },
];

function resolveValue(name: string, tokens: TokenMap, depth = 0): string {
  if (depth > 16) throw new Error(`Ciclo de var() ao resolver ${name}`);
  const raw = tokens[name];
  if (raw === undefined) throw new Error(`Token inexistente: ${name}`);

  const varMatch = /^var\(\s*(--[\w-]+)\s*(?:,([\s\S]+))?\)$/.exec(raw);
  if (!varMatch) return raw;

  const referenced = varMatch[1];
  if (tokens[referenced] !== undefined) return resolveValue(referenced, tokens, depth + 1);
  if (varMatch[2]) return varMatch[2].trim();
  throw new Error(`var(${referenced}) sem fallback e sem definição`);
}

// ---------------------------------------------------------------------------
// Matemática WCAG
// ---------------------------------------------------------------------------

interface Rgb {
  r: number;
  g: number;
  b: number;
}

function parseColor(value: string): Rgb | null {
  const hex = /^#([0-9a-f]{3}|[0-9a-f]{6})$/i.exec(value);
  if (hex) {
    const h = hex[1];
    const full =
      h.length === 3
        ? h
            .split('')
            .map((c) => c + c)
            .join('')
        : h;
    return {
      r: parseInt(full.slice(0, 2), 16),
      g: parseInt(full.slice(2, 4), 16),
      b: parseInt(full.slice(4, 6), 16),
    };
  }

  const fn = /^rgba?\(\s*([\d.]+)[\s,]+([\d.]+)[\s,]+([\d.]+)(?:[\s,/]+([\d.]+))?\s*\)$/i.exec(
    value,
  );
  if (fn) {
    const alpha = fn[4] === undefined ? 1 : Number(fn[4]);
    // Cores com alfa são decorativas (contornos): não entram no cálculo.
    if (alpha < 1) return null;
    return { r: Number(fn[1]), g: Number(fn[2]), b: Number(fn[3]) };
  }

  return null;
}

function channelToLinear(channel8bit: number): number {
  const c = channel8bit / 255;
  return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
}

/** Luminância relativa — WCAG 2.x, definição de `relative luminance`. */
function relativeLuminance({ r, g, b }: Rgb): number {
  return (
    0.2126 * channelToLinear(r) + 0.7152 * channelToLinear(g) + 0.0722 * channelToLinear(b)
  );
}

/** Razão de contraste — (L_claro + 0.05) / (L_escuro + 0.05), de 1:1 a 21:1. */
function contrastRatio(a: Rgb, b: Rgb): number {
  const la = relativeLuminance(a);
  const lb = relativeLuminance(b);
  const lighter = Math.max(la, lb);
  const darker = Math.min(la, lb);
  return (lighter + 0.05) / (darker + 0.05);
}

function rgbFor(theme: Theme, token: string): Rgb {
  const resolved = resolveValue(token, theme.tokens);
  const rgb = parseColor(resolved);
  if (!rgb) throw new Error(`Token ${token} não resolveu para cor opaca (obtido: ${resolved})`);
  return rgb;
}

// ---------------------------------------------------------------------------
// Pares exigidos
// ---------------------------------------------------------------------------

const TEXT_MIN = 4.5; // 1.4.3 — texto normal
const UI_MIN = 3.0; // 1.4.11 / texto grande

interface Pair {
  label: string;
  fg: string;
  bg: string;
  min: number;
}

const TEXT_PAIRS: Pair[] = [
  { label: 'texto primário / canvas', fg: '--text-primary', bg: '--surface', min: TEXT_MIN },
  { label: 'texto primário / painel', fg: '--text-primary', bg: '--panel', min: TEXT_MIN },
  { label: 'texto primário / controlo', fg: '--text-primary', bg: '--control', min: TEXT_MIN },
  { label: 'texto secundário / painel', fg: '--text-secondary', bg: '--panel', min: TEXT_MIN },
  { label: 'texto secundário / canvas', fg: '--text-secondary', bg: '--surface', min: TEXT_MIN },
  { label: 'texto discreto / painel', fg: '--text-muted', bg: '--panel', min: TEXT_MIN },
  { label: 'texto discreto / controlo', fg: '--text-muted', bg: '--control', min: TEXT_MIN },
  // rótulo sobre o fundo do botão
  { label: 'rótulo primário / fundo da marca', fg: '--on-brand', bg: '--brand', min: TEXT_MIN },
  { label: 'rótulo primário / marca (hover)', fg: '--on-brand', bg: '--brand-strong', min: TEXT_MIN },
  { label: 'rótulo destrutivo / fundo', fg: '--on-danger', bg: '--danger', min: TEXT_MIN },
  { label: 'rótulo destrutivo / fundo (hover)', fg: '--on-danger', bg: '--danger-strong', min: TEXT_MIN },
];

// Acentos semânticos usados como TEXTO (navegação, skills, sub-agentes, aviso, erro)
const ACCENT_TEXT_PAIRS: Pair[] = [
  { label: 'marca (cyan) / canvas', fg: '--brand', bg: '--surface', min: TEXT_MIN },
  { label: 'marca (cyan) / painel', fg: '--brand', bg: '--panel', min: TEXT_MIN },
  { label: 'skills (teal) / painel', fg: '--skill', bg: '--panel', min: TEXT_MIN },
  { label: 'sub-agentes (violet) / painel', fg: '--subagent', bg: '--panel', min: TEXT_MIN },
  { label: 'aviso (amber) / painel', fg: '--warning', bg: '--panel', min: TEXT_MIN },
  { label: 'erro (rose) / painel', fg: '--danger', bg: '--panel', min: TEXT_MIN },
];

// Badges de estado: fundo tinta sólida + texto do estado
const STATUS_PAIRS: Pair[] = [
  { label: 'badge verified', fg: '--status-verified-fg', bg: '--status-verified-bg', min: TEXT_MIN },
  { label: 'badge partial', fg: '--status-partial-fg', bg: '--status-partial-bg', min: TEXT_MIN },
  { label: 'badge pending', fg: '--status-pending-fg', bg: '--status-pending-bg', min: TEXT_MIN },
  { label: 'badge error', fg: '--status-error-fg', bg: '--status-error-bg', min: TEXT_MIN },
];

// Ícones/indicadores de estado (elementos não textuais)
const UI_PAIRS: Pair[] = [
  { label: 'indicador de marca / canvas', fg: '--brand', bg: '--surface', min: UI_MIN },
  { label: 'indicador de skills / painel', fg: '--skill', bg: '--panel', min: UI_MIN },
  { label: 'indicador de sub-agentes / painel', fg: '--subagent', bg: '--panel', min: UI_MIN },
  { label: 'indicador de aviso / painel', fg: '--warning', bg: '--panel', min: UI_MIN },
  { label: 'indicador de erro / painel', fg: '--danger', bg: '--panel', min: UI_MIN },
  { label: 'anel de foco / canvas', fg: '--focus-ring', bg: '--surface', min: UI_MIN },
  { label: 'anel de foco / painel', fg: '--focus-ring', bg: '--panel', min: UI_MIN },
];

const ALL_PAIRS = [...TEXT_PAIRS, ...ACCENT_TEXT_PAIRS, ...STATUS_PAIRS, ...UI_PAIRS];

function formatReport(failures: string[]): string {
  const lines: string[] = ['', 'Falhas de contraste:', ''];
  for (const f of failures) lines.push(`  - ${f}`);
  lines.push('');
  return lines.join('\n');
}

// ---------------------------------------------------------------------------
// Testes
// ---------------------------------------------------------------------------

describe('tokens: arquitectura em duas camadas', () => {
  it('a cor/texto resolve para hex opaco em ambos os temas (sem depender do Tailwind)', () => {
    for (const theme of THEMES) {
      expect(() => rgbFor(theme, '--surface')).not.toThrow();
      expect(() => rgbFor(theme, '--panel')).not.toThrow();
      expect(() => rgbFor(theme, '--text-primary')).not.toThrow();
    }
  });

  it('a paleta Arkhe é própria: não referencia escalas do Tailwind', () => {
    const paletteValues = Object.entries(rootTokens)
      .filter(([name]) => name.startsWith('--palette-'))
      .map(([, value]) => value);
    expect(paletteValues.length).toBeGreaterThan(0);
    for (const value of paletteValues) {
      expect(value).not.toMatch(/\bzinc\b|\bvar\(--color-/);
    }
  });

  it('os primitivos são partilhados: o tema claro só reatribui a camada semântica', () => {
    // Nenhum override do tema claro pode tocar em `--palette-*`.
    for (const name of Object.keys(lightOverrides)) {
      expect(name.startsWith('--palette-')).toBe(false);
    }
    // E os primitivos presentes nos dois temas têm de ser idênticos.
    const darkPrimitives = Object.entries(rootTokens).filter(([n]) => n.startsWith('--palette-'));
    expect(darkPrimitives.length).toBeGreaterThan(0);
    for (const [name, value] of darkPrimitives) {
      expect(THEMES[1].tokens[name]).toBe(value);
    }
  });

  it('dark-first: o tema escuro é o default em :root', () => {
    // A declaração em :root aponta para o primitivo…
    expect(rootTokens['--surface']).toBe('var(--palette-arkhe-canvas)');
    // …e resolve para o valor Arkhe escuro, não para um valor claro.
    expect(resolveValue('--surface', rootTokens)).toBe('#09090b');
  });

  it('o tema claro produz valores semânticos diferentes do escuro', () => {
    for (const token of ['--surface', '--panel', '--text-primary', '--brand']) {
      expect(resolveValue(token, THEMES[1].tokens)).not.toBe(resolveValue(token, THEMES[0].tokens));
    }
  });
});

describe('contraste WCAG', () => {
  for (const theme of THEMES) {
    it(`todos os pares passam os mínimos no tema ${theme.name}`, () => {
      const failures: string[] = [];
      const measured: string[] = [];

      for (const pair of ALL_PAIRS) {
        const ratio = contrastRatio(rgbFor(theme, pair.fg), rgbFor(theme, pair.bg));
        const ok = ratio >= pair.min;
        measured.push(
          `    ${ok ? 'PASS' : 'FAIL'}  ${ratio.toFixed(2).padStart(6)}:1 ` +
            `(min ${pair.min.toFixed(1)})  ${pair.label}`,
        );
        if (!ok) {
          failures.push(
            `${pair.label}: ${ratio.toFixed(2)}:1 < ${pair.min}:1 ` +
              `(${pair.fg} sobre ${pair.bg})`,
          );
        }
      }

      // Diagnóstico completo quando falha.
      if (failures.length > 0) {
        console.log(`\n[${theme.name}]\n${measured.join('\n')}`);
        throw new Error(formatReport(failures));
      }

      // Auditoria sob pedido: ARKHE_CONTRAST_REPORT=1 npx vitest run test/contrast.test.ts
      if (process.env['ARKHE_CONTRAST_REPORT']) {
        console.log(`\n[${theme.name}] ${ALL_PAIRS.length} pares medidos\n${measured.join('\n')}`);
      }

      expect(failures, formatReport(failures)).toHaveLength(0);
    });
  }

  it('calcula a razão de contraste segundo a definição WCAG', () => {
    // Âncoras conhecidas: preto/branco = 21:1, e uma igualdade = 1:1.
    expect(contrastRatio({ r: 0, g: 0, b: 0 }, { r: 255, g: 255, b: 255 })).toBeCloseTo(21, 1);
    expect(contrastRatio({ r: 9, g: 9, b: 11 }, { r: 9, g: 9, b: 11 })).toBeCloseTo(1, 5);
  });
});

describe('lacuna conhecida: contornos de controlo (SC 1.4.11)', () => {
  /**
   * A paleta dark aprovada na spec fixa `arkhe-control: #27272a` sobre
   * `arkhe-canvas: #09090b` e contornos com alfa 0.06/0.12. Medido, isto
   * fica muito abaixo de 3:1 — ou seja, a *fronteira* de um Input em
   * repouso não cumpre 1.4.11 Non-text Contrast (o estado de foco cumpre,
   * via `--focus-ring`, verificado acima).
   *
   * Isto NÃO é um teste a fingir que passa: fixa o valor medido para que
   * qualquer alteração da paleta volte a expor o problema, e documenta a
   * lacuna no README. Requer decisão de design sobre a paleta.
   */
  const surfacePairs: Array<{ fg: string; bg: string }> = [
    { fg: '--control', bg: '--surface' },
    { fg: '--control', bg: '--panel' },
  ];

  for (const theme of THEMES) {
    it(`registo da separação de superfícies no tema ${theme.name}`, () => {
      for (const entry of surfacePairs) {
        const ratio = contrastRatio(rgbFor(theme, entry.fg), rgbFor(theme, entry.bg));
        // Se a paleta mudar (a separação deixar de ser baixa), este assert
        // obriga a revisitar a lacuna em vez de a deixar passar em silêncio.
        expect(ratio).toBeGreaterThan(1);
        expect(ratio).toBeLessThan(UI_MIN);
      }
    });
  }

  it('o estado de foco, esse, cumpre 3:1 (o estado é distinguível)', () => {
    for (const theme of THEMES) {
      expect(contrastRatio(rgbFor(theme, '--focus-ring'), rgbFor(theme, '--surface'))).toBeGreaterThanOrEqual(
        UI_MIN,
      );
    }
  });
});
