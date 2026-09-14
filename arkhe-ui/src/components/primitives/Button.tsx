import type * as React from 'react';
import { cva, type VariantProps } from 'class-variance-authority';

import { cn } from '../../lib/cn';

/**
 * Foco visível: `focus-visible:` (não `focus:`) para não desenhar o anel
 * em cliques de rato. O anel cobre 2.4.7 e 2.4.11 (WCAG 2.2).
 * `disabled:` mantém os estados perceptíveis sem depender só da cor.
 */
export const buttonVariants = cva(
  [
    'inline-flex shrink-0 items-center justify-center gap-2',
    'rounded-control font-medium whitespace-nowrap',
    'transition-colors select-none',
    'outline-none focus-visible:ring-2 focus-visible:ring-focus-ring',
    'focus-visible:ring-offset-2 focus-visible:ring-offset-surface',
    'disabled:pointer-events-none disabled:opacity-50',
  ],
  {
    variants: {
      variant: {
        primary: 'bg-brand text-on-brand hover:bg-brand-strong',
        secondary:
          'border border-border-strong bg-control text-fg hover:bg-control-strong',
        ghost: 'bg-transparent text-fg-muted hover:bg-control hover:text-fg',
        destructive: 'bg-danger text-on-danger hover:bg-danger-strong',
      },
      size: {
        sm: 'h-8 px-3 text-sm',
        md: 'h-9 px-4 text-sm',
        lg: 'h-11 px-6 text-base',
        icon: 'size-9',
      },
    },
    defaultVariants: {
      variant: 'primary',
      size: 'md',
    },
  },
);

export type ButtonProps = React.ComponentPropsWithRef<'button'> &
  VariantProps<typeof buttonVariants>;

export function Button({
  className,
  variant,
  size,
  type = 'button',
  ...props
}: ButtonProps) {
  return (
    <button
      data-slot="button"
      data-variant={variant ?? 'primary'}
      data-size={size ?? 'md'}
      type={type}
      className={cn(buttonVariants({ variant, size }), className)}
      {...props}
    />
  );
}
