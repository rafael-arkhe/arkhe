import { afterEach } from 'vitest';

import { cleanupAll } from './render';

/**
 * Setup global do Vitest (ver `setupFiles` em vitest.config.ts).
 *
 * Sem `@testing-library/jest-dom` nesta fase: as asserções usam o `expect`
 * nativo do Vitest, para não alargar as dependências além da spec.
 */

// O React 19 exige esta flag para não emitir avisos de `act` em ambiente de teste.
(globalThis as unknown as Record<string, unknown>)['IS_REACT_ACT_ENVIRONMENT'] = true;

// jsdom não implementa matchMedia; componentes de tema podem vir a usá-lo.
if (typeof window !== 'undefined' && !window.matchMedia) {
  window.matchMedia = ((query: string) => ({
    matches: false,
    media: query,
    onchange: null,
    addListener: () => {},
    removeListener: () => {},
    addEventListener: () => {},
    removeEventListener: () => {},
    dispatchEvent: () => false,
  })) as unknown as typeof window.matchMedia;
}

// Este setup é global, mas corre também em ficheiros com
// `@vitest-environment node` (ex.: o teste de contraste). Nada aqui pode
// assumir que existe DOM.
const hasDom = typeof document !== 'undefined';

afterEach(() => {
  cleanupAll();
  if (hasDom) {
    document.body.innerHTML = '';
    document.documentElement.removeAttribute('data-theme');
  }
});
