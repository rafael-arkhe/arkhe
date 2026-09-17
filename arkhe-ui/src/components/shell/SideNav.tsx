import type * as React from 'react';

import { cn } from '../../lib/cn';
import { TEXT } from './text';

/**
 * Side nav rail — a barra lateral persistente, o segundo componente comum.
 *
 * O blueprint chama-lhe `SIDE NAV BAR (Persistent Rail Shared Component)` e dá
 * dez separadores. A ordem abaixo é a do blueprint, lida do ecrã de entrada e
 * dos restantes; não é uma invenção de arrumação.
 *
 * Cada entrada leva um `id` estável para os testes e para a navegação. O
 * separador activo é o único com o traço de 2px à esquerda — no `DESIGN.md`
 * "every indicator represents an explicit system state", portanto a marca é
 * binária, não uma gradação.
 */
export const NAV_TABS = [
  { id: 'overview', label: 'Visão Geral' },
  { id: 'artifacts', label: 'Artefatos' },
  { id: 'models', label: 'Modelos' },
  { id: 'ledger', label: 'Ledger' },
  { id: 'gates', label: 'Gates' },
  { id: 'forensics', label: 'Forense' },
  { id: 'wormgraph', label: 'WormGraph' },
  { id: 'network', label: 'Rede' },
  { id: 'keys', label: 'Chaves' },
  { id: 'settings', label: 'Configuração' },
] as const;

export type NavTabId = (typeof NAV_TABS)[number]['id'];

export type SideNavProps = Omit<React.ComponentPropsWithRef<'nav'>, 'onSelect'> & {
  /** Separador activo. */
  active?: NavTabId;
  /** Chamado com o separador escolhido. */
  onSelect?: (id: NavTabId) => void;
  /** Selo do nó, mostrado por cima dos separadores. */
  nodeSeal?: React.ReactNode;
};

export function SideNav({
  className,
  active = 'overview',
  onSelect,
  nodeSeal,
  ...props
}: SideNavProps) {
  return (
    <nav
      data-slot="side-nav"
      aria-label="Navegação do sistema"
      className={cn(
        'flex w-52 shrink-0 flex-col gap-1 border-r border-border-default bg-panel p-3',
        className,
      )}
      {...props}
    >
      {nodeSeal !== undefined && (
        <div
          data-slot="side-nav-seal"
          className="mb-2 border-b border-border-default pb-3 text-fg-muted"
        >
          {nodeSeal}
        </div>
      )}

      {NAV_TABS.map(({ id, label }) => {
        const activo = id === active;
        return (
          <button
            key={id}
            type="button"
            data-testid={`nav-${id}`}
            aria-current={activo ? 'page' : undefined}
            onClick={() => onSelect?.(id)}
            className={cn(
              'flex items-center gap-2 rounded-control px-2 py-1.5 text-left outline-none',
              'transition-colors duration-150',
              'focus-visible:ring-2 focus-visible:ring-focus-ring',
              TEXT.labelSm,
              activo
                ? 'border-l-2 border-brand bg-control text-brand'
                : 'border-l-2 border-transparent text-fg-muted hover:bg-control hover:text-fg',
            )}
          >
            {label}
          </button>
        );
      })}
    </nav>
  );
}
