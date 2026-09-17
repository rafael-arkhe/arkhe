import type * as React from 'react';
import { cva, type VariantProps } from 'class-variance-authority';

import { cn } from '../../lib/cn';

/**
 * Badge de estado criptográfico — o "status pill" do blueprint.
 *
 * O blueprint desenha quatro estados de integridade (`verified`, `partial`,
 * `pending`, `error`) e o `DESIGN.md` é explícito sobre a semântica: a cor
 * **nunca** é ornamento, cada valor significa um estado definitivo do sistema.
 *
 * Em vez de escolher cores, este badge consome os tokens de estado que o
 * design system já tem (`--color-status-*-bg` / `--color-status-*-fg`). São
 * pares próprios, não derivados de `--brand` ou `--danger`: um estado pendente
 * não é uma "marca" nem um "erro", e tratá-lo como qualquer um dos dois
 * apagaria a distinção que o blueprint existe para mostrar.
 *
 * O ponto luminoso pulsante do blueprint só aparece em `verified` e `pending`
 * — estados transitórios ou activos. Um `error` não pulsa: é um estado final,
 * e animação num estado final sugere que ainda está a acontecer.
 */
export const statusBadgeVariants = cva(
  [
    'inline-flex shrink-0 items-center gap-1.5',
    'rounded-control border px-2 py-0.5',
    'font-mono text-[0.625rem] font-bold tracking-[0.1em] uppercase',
    'whitespace-nowrap',
  ],
  {
    variants: {
      status: {
        verified: 'border-status-verified-bg bg-status-verified-bg text-status-verified-fg',
        partial: 'border-status-partial-bg bg-status-partial-bg text-status-partial-fg',
        pending: 'border-status-pending-bg bg-status-pending-bg text-status-pending-fg',
        error: 'border-status-error-bg bg-status-error-bg text-status-error-fg',
      },
    },
    defaultVariants: { status: 'pending' },
  },
);

/** O estado com que um indicador luminoso é desenhado, quando o há. */
const DOT: Record<NonNullable<StatusBadgeProps['status']>, string> = {
  verified: 'bg-status-verified-fg animate-pulse',
  partial: 'bg-status-partial-fg',
  pending: 'bg-status-pending-fg animate-pulse',
  error: 'bg-status-error-fg',
};

export type StatusBadgeProps = Omit<React.ComponentPropsWithRef<'span'>, 'children'> &
  VariantProps<typeof statusBadgeVariants> & {
    /** O texto do badge. No blueprint vem entre parênteses rectos, e mantém-se. */
    children: React.ReactNode;
    /** Mostra o ponto luminoso. Por omissão, só nos estados não-terminais. */
    dot?: boolean;
  };

export function StatusBadge({ className, status, dot, children, ...props }: StatusBadgeProps) {
  const chave = status ?? 'pending';
  const mostrarPonto = dot ?? chave !== 'error';

  return (
    <span
      data-slot="status-badge"
      data-status={chave}
      className={cn(statusBadgeVariants({ status }), className)}
      {...props}
    >
      {mostrarPonto && (
        <span
          data-slot="status-badge-dot"
          aria-hidden="true"
          className={cn('h-1.5 w-1.5 rounded-full', DOT[chave])}
        />
      )}
      {children}
    </span>
  );
}
