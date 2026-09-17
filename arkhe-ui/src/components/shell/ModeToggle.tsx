import type * as React from 'react';

import { cn } from '../../lib/cn';
import { TEXT } from './text';

/**
 * Alternador de modo — Básico / Expert.
 *
 * O `DESIGN.md` descreve-o como uma das peças centrais do produto: o mesmo
 * sistema, dois níveis de exposição. O modo Básico mostra o painel executivo;
 * o Expert expõe os protocolos, os hashes e as camadas Raw.
 *
 * É um `<nav>` com dois `<a>` no blueprint. Aqui é um grupo de botões com
 * `aria-pressed`, porque o que faz é **mudar um estado**, não navegar para um
 * recurso — e a diferença importa para quem usa leitor de ecrã.
 *
 * A `children` é a barra direita do par (o blueprint mostra dois `<a>` lado a
 * lado); este componente desenha só os dois botões.
 */
export type Mode = 'basic' | 'expert';

const MODO: { id: Mode; rotulo: string }[] = [
  { id: 'basic', rotulo: 'Modo Básico' },
  { id: 'expert', rotulo: 'Modo Expert' },
];

export type ModeToggleProps = Omit<React.ComponentPropsWithRef<'div'>, 'onChange'> & {
  /** Modo activo. */
  mode?: Mode;
  /** Chamado com o modo escolhido. Sem ele o alternador é só apresentação. */
  onModeChange?: (mode: Mode) => void;
};

export function ModeToggle({
  className,
  mode = 'basic',
  onModeChange,
  ...props
}: ModeToggleProps) {
  return (
    <div
      data-slot="mode-toggle"
      data-mode={mode}
      role="group"
      aria-label="Nível de exposição do sistema"
      className={cn('flex items-center gap-2', className)}
      {...props}
    >
      {MODO.map(({ id, rotulo }) => {
        const activo = id === mode;
        return (
          <button
            key={id}
            type="button"
            data-testid={`mode-${id}`}
            aria-pressed={activo}
            onClick={() => onModeChange?.(id)}
            className={cn(
              'px-2 py-0.5 outline-none transition-colors duration-150',
              'focus-visible:ring-2 focus-visible:ring-focus-ring',
              TEXT.labelXs,
              activo
                ? 'border-b border-brand font-bold text-brand'
                : 'text-fg-subtle hover:text-fg',
            )}
          >
            {rotulo}
          </button>
        );
      })}
    </div>
  );
}
