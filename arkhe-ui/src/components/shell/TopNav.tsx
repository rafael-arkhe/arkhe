import type * as React from 'react';

import { cn } from '../../lib/cn';
import { StatusBadge } from '../primitives/StatusBadge';
import { IconButton } from '../primitives/IconButton';
import { TEXT } from './text';
import { ModeToggle, type Mode } from './ModeToggle';

/**
 * Top nav bar — o componente mais reutilizado do blueprint.
 *
 * Aparece comentado como `SHARED COMPONENT EXECUTION` em todos os 20 ecrãs.
 * Estrutura, da esquerda para a direita:
 *
 *   identidade    → `ARKHE // OS` + alternador de modo
 *   pesquisa      → consola com o atalho `^K` (simulação de paleta de comandos)
 *   integridade   → os dois badges: `[ED25519: ROOT VALIDATED]` e `HITL OVERRIDE`
 *   acções        → terminal, shield, lock
 *
 * Os dois badges de integridade **não** são decorativos e não são opcionais no
 * desenho: o `DESIGN.md` põe a autoridade criptográfica à vista, não escondida
 * em metadados. Por isso o componente recebe os seus estados como props e
 * desenha-os sempre — se não houver estado, o badge diz `pending`, em vez de
 * desaparecer. Um espaço vazio onde devia haver prova de integridade é a
 * informação errada.
 */
export type TopNavProps = Omit<React.ComponentPropsWithRef<'header'>, 'onChange'> & {
  /** Modo activo (o alternador Básico/Expert). */
  mode?: Mode;
  /** Notificado quando o modo muda. */
  onModeChange?: (mode: Mode) => void;
  /** Estado da raiz de confiança. Por omissão `pending` — nunca se presume. */
  rootStatus?: 'verified' | 'partial' | 'pending' | 'error';
  /** Texto do badge da raiz, se quiser sobrepor o texto por omissão. */
  rootLabel?: string;
  /** Estado do override humano. */
  hitlStatus?: 'pending' | 'error';
  /** Acções de ícone à direita. Sem elas, desenham-se as três do blueprint. */
  actions?: React.ReactNode;
  /** Chamado quando a pesquisa é submetida. */
  onSearch?: (query: string) => void;
};

export function TopNav({
  className,
  mode = 'basic',
  onModeChange,
  rootStatus = 'pending',
  rootLabel,
  hitlStatus = 'pending',
  actions,
  onSearch,
  ...props
}: TopNavProps) {
  const rotuloRaiz = rootLabel ?? '[ED25519: ROOT VALIDATED]';

  return (
    <header
      data-slot="top-nav"
      className={cn(
        'sticky top-0 z-40 flex h-12 w-full items-center justify-between gap-4',
        'border-b border-border-default bg-surface px-5',
        className,
      )}
      {...props}
    >
      <div className="flex min-w-0 items-center gap-5">
        <div className="flex items-center gap-1">
          <span className={cn(TEXT.label, 'text-brand')} aria-hidden="true">
            ◈
          </span>
          <span className={cn(TEXT.label, 'font-bold tracking-[0.2em] text-brand')}>
            ARKHE // OS
          </span>
        </div>
        <div className="h-4 w-px bg-border-default" aria-hidden="true" />
        <ModeToggle mode={mode} onModeChange={onModeChange} />
      </div>

      <form
        role="search"
        className="hidden min-w-0 flex-1 items-center gap-2 lg:flex"
        onSubmit={(event) => {
          event.preventDefault();
          const campo = new FormData(event.currentTarget).get('query');
          onSearch?.(typeof campo === 'string' ? campo : '');
        }}
      >
        <div className="flex w-full max-w-md items-center gap-2 rounded-control border border-border-default bg-panel px-2 py-1">
          <span className="text-fg-subtle" aria-hidden="true">
            ⌕
          </span>
          <input
            name="query"
            type="search"
            data-testid="topnav-search"
            aria-label="Pesquisar no sistema"
            placeholder="QUERY_MERKLE_LOG: index=[1..142] || leaf_hash…"
            className={cn(
              TEXT.codeSm,
              'w-full border-none bg-transparent p-0 text-fg outline-none',
              'placeholder:text-fg-subtle focus:ring-0',
            )}
          />
          <kbd
            className={cn(
              TEXT.labelXs,
              'rounded border border-border-default px-1 text-fg-subtle',
            )}
          >
            ^K
          </kbd>
        </div>
      </form>

      <div className="flex shrink-0 items-center gap-3">
        <StatusBadge status={rootStatus} data-testid="topnav-root-status">
          {rotuloRaiz}
        </StatusBadge>
        <StatusBadge status={hitlStatus === 'error' ? 'error' : 'pending'} data-testid="topnav-hitl">
          HITL OVERRIDE [FAIL-CLOSED]
        </StatusBadge>

        {actions ?? (
          <div className="flex items-center gap-1">
            <IconButton aria-label="Terminal Root" title="Terminal Root">
              ▤
            </IconButton>
            <IconButton aria-label="Kernel Shield" title="Kernel Shield">
              ◈
            </IconButton>
            <IconButton aria-label="Vault Locked" title="Vault Locked">
              ▣
            </IconButton>
          </div>
        )}
      </div>
    </header>
  );
}
