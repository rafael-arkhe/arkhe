import type * as React from 'react';
import { cva, type VariantProps } from 'class-variance-authority';

import { cn } from '../../lib/cn';

export const badgeVariants = cva(
  [
    'inline-flex items-center gap-1.5 rounded-full border',
    'px-2 py-0.5 text-xs font-medium whitespace-nowrap',
  ],
  {
    variants: {
      /**
       * O texto do estado carrega o significado (nunca só a cor) —
       * WCAG 2.2 SC 1.4.1 "Use of Color".
       */
      status: {
        verified: 'border-status-verified-bg bg-status-verified-bg text-status-verified-fg',
        partial: 'border-status-partial-bg bg-status-partial-bg text-status-partial-fg',
        pending: 'border-status-pending-bg bg-status-pending-bg text-status-pending-fg',
        error: 'border-status-error-bg bg-status-error-bg text-status-error-fg',
      },
    },
    defaultVariants: {
      status: 'pending',
    },
  },
);

export type BadgeProps = React.ComponentPropsWithRef<'span'> &
  VariantProps<typeof badgeVariants>;

export function Badge({ className, status, children, ...props }: BadgeProps) {
  return (
    <span
      data-slot="badge"
      data-status={status ?? 'pending'}
      className={cn(badgeVariants({ status }), className)}
      {...props}
    >
      {children}
    </span>
  );
}
