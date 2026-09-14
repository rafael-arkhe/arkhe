import type * as React from 'react';
import { cva, type VariantProps } from 'class-variance-authority';

import { cn } from '../../lib/cn';

export const iconButtonVariants = cva(
  [
    'inline-flex shrink-0 items-center justify-center',
    'rounded-control',
    'transition-colors select-none',
    'outline-none focus-visible:ring-2 focus-visible:ring-focus-ring',
    'focus-visible:ring-offset-2 focus-visible:ring-offset-surface',
    'disabled:pointer-events-none disabled:opacity-50',
    // O alvo mínimo de 24x24 CSS px (WCAG 2.2 SC 2.5.8) é garantido pelo size.
    '[&>svg]:shrink-0',
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
        // 24px, 32px, 40px — todos >= 24px (SC 2.5.8)
        sm: 'size-6 [&>svg]:size-3.5',
        md: 'size-8 [&>svg]:size-4',
        lg: 'size-10 [&>svg]:size-5',
      },
    },
    defaultVariants: {
      variant: 'ghost',
      size: 'md',
    },
  },
);

export type IconButtonProps = Omit<React.ComponentPropsWithRef<'button'>, 'aria-label'> &
  VariantProps<typeof iconButtonVariants> & {
    /**
     * Obrigatório. Um botão só-ícone não tem conteúdo textual, por isso o
     * nome acessível TEM de ser fornecido — não há forma de o inferir.
     * Sem isto o controlo é inutilizável por leitor de ecrã (SC 4.1.2).
     */
    'aria-label': string;
  };

export function IconButton({
  className,
  variant,
  size,
  type = 'button',
  ...props
}: IconButtonProps) {
  return (
    <button
      data-slot="icon-button"
      data-variant={variant ?? 'ghost'}
      data-size={size ?? 'md'}
      type={type}
      className={cn(iconButtonVariants({ variant, size }), className)}
      {...props}
    />
  );
}
