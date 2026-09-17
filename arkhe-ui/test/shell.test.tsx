import { describe, expect, it, vi } from 'vitest';

import { StatusBadge } from '../src/components/primitives/StatusBadge';
import { ModeToggle } from '../src/components/shell/ModeToggle';
import { NAV_TABS, SideNav } from '../src/components/shell/SideNav';
import { TopNav } from '../src/components/shell/TopNav';
import { click, render } from './render';

/**
 * Testes da fundação do blueprint: os componentes que aparecem em todos os
 * ecrãs. Cada teste verifica o que o componente **afirma**, não a sua forma —
 * as classes mudam, o estado não.
 *
 * Sem `@testing-library/react`, como o resto do repositório: `container` e
 * `querySelector` sobre o DOM real do jsdom.
 */

/** Atalho de consulta: o teste falha com uma mensagem clara se não encontrar. */
function q(container: HTMLElement, selector: string): HTMLElement {
  const el = container.querySelector(selector);
  if (!el) throw new Error(`não encontrei ${selector} no DOM`);
  return el as HTMLElement;
}

function texto(container: HTMLElement): string {
  return container.textContent ?? '';
}

describe('StatusBadge', () => {
  it('renderiza o texto entre parênteses rectos, como o blueprint', () => {
    const { container } = render(<StatusBadge status="verified">[ED25519: ROOT VALIDATED]</StatusBadge>);
    expect(texto(container)).toContain('[ED25519: ROOT VALIDATED]');
  });

  it('expõe o estado em data-status, para o poder isolar', () => {
    const { container } = render(<StatusBadge status="error">falha</StatusBadge>);
    expect(q(container, '[data-slot="status-badge"]').getAttribute('data-status')).toBe('error');
  });

  it('desenha o ponto luminoso nos estados não-terminais, e não no error', () => {
    const pendente = render(<StatusBadge status="pending">pendente</StatusBadge>);
    expect(
      pendente.container.querySelector('[data-slot="status-badge-dot"]'),
    ).not.toBeNull();

    // Um estado final que pulsa sugere que ainda está a acontecer.
    const erro = render(<StatusBadge status="error">erro</StatusBadge>);
    expect(erro.container.querySelector('[data-slot="status-badge-dot"]')).toBeNull();
  });

  it('presume pending quando não lhe dão estado', () => {
    const { container } = render(<StatusBadge>sem estado</StatusBadge>);
    expect(q(container, '[data-slot="status-badge"]').getAttribute('data-status')).toBe('pending');
  });
});

describe('ModeToggle', () => {
  it('começa no modo básico e marca-o como activo', () => {
    const { container } = render(<ModeToggle />);
    expect(q(container, '[data-testid="mode-basic"]').getAttribute('aria-pressed')).toBe('true');
    expect(q(container, '[data-testid="mode-expert"]').getAttribute('aria-pressed')).toBe('false');
  });

  it('notifica a mudança de modo quando se clica no Expert', () => {
    const onModeChange = vi.fn();
    const { container } = render(<ModeToggle onModeChange={onModeChange} />);
    click(q(container, '[data-testid="mode-expert"]'));
    expect(onModeChange).toHaveBeenCalledWith('expert');
  });

  it('respeita o modo que lhe passam', () => {
    const { container } = render(<ModeToggle mode="expert" />);
    expect(q(container, '[data-testid="mode-expert"]').getAttribute('aria-pressed')).toBe('true');
  });
});

describe('TopNav', () => {
  it('mostra a identidade do sistema', () => {
    const { container } = render(<TopNav />);
    expect(texto(container)).toContain('ARKHE // OS');
  });

  it('desenha sempre os dois badges de integridade, mesmo sem estado', () => {
    // Um espaço vazio onde devia haver prova de integridade é a informação errada.
    const { container } = render(<TopNav />);
    expect(q(container, '[data-testid="topnav-root-status"]').getAttribute('data-status')).toBe(
      'pending',
    );
    expect(q(container, '[data-testid="topnav-hitl"]').getAttribute('data-status')).toBe('pending');
  });

  it('mostra o estado da raiz quando lhe é dado', () => {
    const { container } = render(<TopNav rootStatus="verified" />);
    expect(q(container, '[data-testid="topnav-root-status"]').getAttribute('data-status')).toBe(
      'verified',
    );
  });

  it('entrega a consulta de pesquisa a quem a pediu', () => {
    const onSearch = vi.fn();
    const { container } = render(<TopNav onSearch={onSearch} />);
    const campo = q(container, '[data-testid="topnav-search"]') as HTMLInputElement;
    campo.value = 'leaf_hash=abc';
    campo.closest('form')!.dispatchEvent(new Event('submit', { cancelable: true, bubbles: true }));
    expect(onSearch).toHaveBeenCalledWith('leaf_hash=abc');
  });
});

describe('SideNav', () => {
  it('tem os dez separadores do blueprint', () => {
    expect(NAV_TABS).toHaveLength(10);
    const { container } = render(<SideNav />);
    for (const tab of NAV_TABS) {
      expect(q(container, `[data-testid="nav-${tab.id}"]`).textContent).toBe(tab.label);
    }
  });

  it('marca um só separador como activo', () => {
    const { container } = render(<SideNav active="ledger" />);
    expect(q(container, '[data-testid="nav-ledger"]').getAttribute('aria-current')).toBe('page');
    expect(q(container, '[data-testid="nav-overview"]').getAttribute('aria-current')).toBeNull();
  });

  it('notifica a escolha de um separador', () => {
    const onSelect = vi.fn();
    const { container } = render(<SideNav onSelect={onSelect} />);
    click(q(container, '[data-testid="nav-forensics"]'));
    expect(onSelect).toHaveBeenCalledWith('forensics');
  });
});
