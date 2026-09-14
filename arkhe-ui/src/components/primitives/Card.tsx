import type * as React from 'react';
import { cva, type VariantProps } from 'class-variance-authority';

import { cn } from '../../lib/cn';

export const cardVariants = cva('rounded-panel border', {
  variants: {
    variant: {
      panel: 'border-border-default bg-panel text-fg',
      surface: 'border-border-default bg-surface text-fg',
      interactive: [
        'border-border-default bg-panel text-fg',
        'transition-colors hover:border-border-strong hover:bg-control',
        // Cartão clicável tem de mostrar foco — senão é inalcançável por teclado.
        'focus-within:ring-2 focus-within:ring-focus-ring',
      ],
    },
    padding: {
      none: 'p-0',
      sm: 'p-3',
      md: 'p-4',
      lg: 'p-6',
    },
  },
  defaultVariants: {
    variant: 'panel',
    padding: 'md',
  },
});

export type CardProps = React.ComponentPropsWithRef<'div'> &
  VariantProps<typeof cardVariants>;

export function Card({ className, variant, padding, ...props }: CardProps) {
  return (
    <div
      data-slot="card"
      data-variant={variant ?? 'panel'}
      className={cn(cardVariants({ variant, padding }), className)}
      {...props}
    />
  );
}

export function CardHeader({ className, ...props }: React.ComponentPropsWithRef<'div'>) {
  return <div data-slot="card-header" className={cn('mb-3 space-y-1', className)} {...props} />;
}

export function CardTitle({ className, ...props }: React.ComponentPropsWithRef<'h3'>) {
  return (
    <h3
      data-slot="card-title"
      className={cn('text-base leading-tight font-semibold text-fg', className)}
      {...props}
    />
  );
}

export function CardDescription({ className, ...props }: React.ComponentPropsWithRef<'p'>) {
  return (
    <p
      data-slot="card-description"
      className={cn('text-sm text-fg-muted', className)}
      {...props}
    />
  );
}

export function CardContent({ className, ...props }: React.ComponentPropsWithRef<'div'>) {
  return <div data-slot="card-content" className={cn('text-sm text-fg', className)} {...props} />;
}

export function CardFooter({ className, ...props }: React.ComponentPropsWithRef<'div'>) {
  return (
    <div
      data-slot="card-footer"
      className={cn('mt-4 flex items-center gap-2', className)}
      {...props}
    />
  );
}
