import type * as React from 'react';
import { cva, type VariantProps } from 'class-variance-authority';

import { cn } from '../../lib/cn';
import { TEXT } from './text';

/**
 * Chamber — o módulo estrutural do Arkhe OS.
 *
 * É o "cartão" do blueprint (`bg-surface-container-low border
 * border-outline-variant`) traduzido para os tokens semânticos do repositório:
 *
 *   canvas → `--surface`   (Level 0, canvas base)
 *   panel  → `--panel`     (Level 1, módulo em repouso)
 *   active → `--control`   (Level 2, workstation activa)
 *
 * O `accent` acrescenta a borda de 1px colorida que o `DESIGN.md` usa para
 * sinalizar estado (Level 3). O halo luminoso do blueprint não é reproduzido:
 * não há token para o halo e a regra é não inventar cor — a borda de 1px é o
 * sinal que fica. Ver o relatório.
 */
export const chamberVariants = cva('rounded-panel border', {
  variants: {
    level: {
      canvas: 'border-border-default bg-surface',
      panel: 'border-border-default bg-panel',
      active: 'border-border-strong bg-control',
    },
    /**
     * `danger` é o único que acrescenta contenção extra (anel), porque no
     * fail-closed a borda sozinha não distingue "aviso" de "fronteira de
     * contenção" (`DESIGN.md`, "Fail-Closed State Isolation").
     */
    accent: {
      none: '',
      brand: 'border-brand/60',
      skill: 'border-skill/60',
      subagent: 'border-subagent/60',
      warning: 'border-warning/60',
      danger: 'border-danger ring-1 ring-danger/25',
    },
    padding: {
      none: 'p-0',
      sm: 'p-3',
      md: 'p-4',
      lg: 'p-6',
    },
  },
  defaultVariants: {
    level: 'panel',
    accent: 'none',
    padding: 'md',
  },
});

/** Tom do cabeçalho do módulo — o `text-primary`/`text-secondary` do blueprint. */
export const headingTones = {
  default: 'text-fg',
  brand: 'text-brand',
  skill: 'text-skill',
  subagent: 'text-subagent',
  warning: 'text-warning',
  danger: 'text-danger',
  muted: 'text-fg-muted',
} as const;

export type HeadingTone = keyof typeof headingTones;

export type ChamberProps = Omit<React.ComponentPropsWithRef<'section'>, 'title'> &
  VariantProps<typeof chamberVariants> & {
    /** Título do módulo. Sem ele não há cabeçalho nenhum. */
    heading?: React.ReactNode;
    /** Linha secundária do cabeçalho (o `text-code-sm text-outline` do blueprint). */
    subheading?: React.ReactNode;
    /** Canto direito do cabeçalho: badges, contadores, acções. */
    actions?: React.ReactNode;
    /** Rodapé separado por 1px, como nas tabelas. */
    footer?: React.ReactNode;
    headingTone?: HeadingTone;
    /** `data-testid` do módulo, para os testes o poderem isolar. */
    testId?: string;
  };

export function Chamber({
  className,
  level,
  accent,
  padding = 'none',
  heading,
  subheading,
  actions,
  footer,
  headingTone = 'default',
  testId,
  children,
  ...props
}: ChamberProps) {
  const hasHeader = heading !== undefined || actions !== undefined || subheading !== undefined;
  // O padding do módulo é `none` por omissão: o cabeçalho e o corpo gerem o
  // seu próprio espaçamento, senão o separador do cabeçalho não toca nas
  // bordas — que é exactamente o desenho do blueprint.
  const bodyPadding = padding === 'none' ? 'p-4' : '';

  return (
    <section
      data-slot="chamber"
      data-level={level ?? 'panel'}
      data-accent={accent ?? 'none'}
      data-testid={testId}
      className={cn(chamberVariants({ level, accent, padding }), className)}
      {...props}
    >
      {hasHeader && (
        <header
          data-slot="chamber-header"
          className={cn(
            'flex flex-wrap items-center justify-between gap-2 border-b border-border-default px-4 py-3',
          )}
        >
          <div className="min-w-0">
            {heading !== undefined && (
              <div className={cn(TEXT.label, headingTones[headingTone], 'truncate')}>{heading}</div>
            )}
            {subheading !== undefined && (
              <p className={cn(TEXT.codeSm, 'mt-1 text-fg-subtle')}>{subheading}</p>
            )}
          </div>
          {actions !== undefined && (
            <div className="flex shrink-0 flex-wrap items-center gap-2">{actions}</div>
          )}
        </header>
      )}
      <div data-slot="chamber-body" className={cn(bodyPadding)}>
        {children}
      </div>
      {footer !== undefined && (
        <div
          data-slot="chamber-footer"
          className="flex flex-wrap items-center justify-between gap-2 border-t border-border-default px-4 py-3"
        >
          {footer}
        </div>
      )}
    </section>
  );
}
