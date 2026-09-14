import { act, type ReactElement } from 'react';
import { createRoot, type Root } from 'react-dom/client';

/**
 * Helper de render mínimo sobre `react-dom/client`.
 *
 * A spec da Fase 1 não inclui `@testing-library/react`, por isso não a
 * introduzimos: usamos `createRoot` + `act` directamente. A asserção é
 * feita com o `expect` nativo do Vitest sobre o DOM real do jsdom.
 */

const mounted: Array<{ root: Root; container: HTMLElement }> = [];

export function render(ui: ReactElement): { container: HTMLElement } {
  const container = document.createElement('div');
  document.body.appendChild(container);
  const root = createRoot(container);

  act(() => {
    root.render(ui);
  });

  mounted.push({ root, container });
  return { container };
}

/** Desmonta tudo o que foi montado no teste corrente. */
export function cleanupAll(): void {
  while (mounted.length > 0) {
    const entry = mounted.pop();
    if (!entry) break;
    act(() => {
      entry.root.unmount();
    });
    entry.container.remove();
  }
}

/** Dispara um clique real no elemento (respeita `disabled`). */
export function click(el: Element): void {
  act(() => {
    (el as HTMLElement).click();
  });
}
