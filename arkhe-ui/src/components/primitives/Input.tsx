import type * as React from 'react';
import { cva, type VariantProps } from 'class-variance-authority';

import { cn } from '../../lib/cn';

export const inputVariants = cva(
  [
    'w-full rounded-control border bg-control text-fg',
    'placeholder:text-fg-subtle',
    'transition-colors',
    'outline-none focus-visible:ring-2 focus-visible:ring-focus-ring',
    'focus-visible:ring-offset-2 focus-visible:ring-offset-surface',
    // Estado inválido: a cor reforça, mas `aria-invalid` é que o comunica.
    'aria-invalid:border-danger aria-invalid:ring-danger',
    'disabled:cursor-not-allowed disabled:opacity-50',
  ],
  {
    variants: {
      variant: {
        default: 'border-border-default',
        strong: 'border-border-strong',
      },
      // A padding vive SÓ aqui (não também na base): caso contrário o
      // tailwind-merge teria de escolher e a intenção ficava ambígua.
      size: {
        sm: 'h-8 px-2 text-sm',
        md: 'h-9 px-3 text-sm',
        lg: 'h-11 px-4 text-base',
      },
    },
    defaultVariants: {
      variant: 'default',
      size: 'md',
    },
  },
);

export type InputProps = Omit<React.ComponentPropsWithRef<'input'>, 'size'> &
  VariantProps<typeof inputVariants>;

/**
 * `Input` é o controlo nu. O rótulo acessível tem de vir do consumidor:
 * ou um `<label htmlFor>` visível, ou `aria-label`. O componente não
 * inventa um nome acessível — inventá-lo produziria um nome enganador.
 */
export function Input({ className, variant, size, type = 'text', ...props }: InputProps) {
  return (
    <input
      data-slot="input"
      data-variant={variant ?? 'default'}
      data-size={size ?? 'md'}
      type={type}
      className={cn(inputVariants({ variant, size }), className)}
      {...props}
    />
  );
}
