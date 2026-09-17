/**
 * Escala tipográfica do Arkhe OS, em classes reutilizáveis.
 *
 * O `DESIGN.md` do blueprint define 14 estilos (headline-*, body-*, code-*,
 * label-*). O design system do repositório não tem tokens de tipografia além
 * de `--font-sans` / `--font-mono`, portanto a escala é escrita aqui UMA vez e
 * consumida pelos ecrãs — em vez de repetida à mão em cada componente.
 *
 * Regra do `DESIGN.md` que estes nomes codificam: tudo o que é dado numérico,
 * hash, endereço ou micro-etiqueta vai em `mono` (JetBrains Mono no blueprint,
 * `--font-mono` do repositório); prosa vai em `sans`.
 *
 * As cores NÃO estão aqui: cada sítio escolhe o token semântico que lhe cabe
 * (`text-fg`, `text-fg-muted`, `text-brand`, …).
 */
export const TEXT = {
  /** headline-xl */
  display: 'font-sans text-4xl font-bold tracking-[-0.03em]',
  /** headline-lg */
  h1: 'font-sans text-3xl font-semibold tracking-[-0.02em]',
  /** headline-md */
  h2: 'font-sans text-2xl font-semibold tracking-[-0.01em]',
  /** headline-sm */
  h3: 'font-sans text-lg font-semibold',
  /** body-lg */
  body: 'font-sans text-base',
  /** body-md */
  bodyMd: 'font-sans text-sm tracking-[0.01em]',
  /** body-sm */
  bodySm: 'font-sans text-xs tracking-[0.02em]',
  /** label-md */
  label: 'font-mono text-[0.8125rem] font-semibold tracking-[0.05em] uppercase',
  /** label-sm */
  labelSm: 'font-mono text-[0.6875rem] font-semibold tracking-[0.08em] uppercase',
  /** label-xs */
  labelXs: 'font-mono text-[0.625rem] font-bold tracking-[0.1em] uppercase',
  /** code-md */
  code: 'font-mono text-sm tracking-[-0.01em]',
  /** code-sm — a tipografia padrão das tabelas de telemetria */
  codeSm: 'font-mono text-xs',
} as const;

export type TextRole = keyof typeof TEXT;
