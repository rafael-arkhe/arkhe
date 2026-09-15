import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';

// O design system, pelo caminho que o `package.json` expõe (`exports`):
// `./styles.css` → `dist/arkhe-ui.css`. A app consome o pacote construído em
// vez de reconstruir a biblioteca — e é por isso que `npm run build` (da
// biblioteca) tem de correr antes de `npm run build:app`.
import '@arkhe/ui/styles.css';

// Estilos próprios da app. Sem Tailwind de propósito: ver o cabeçalho de
// `app.css` (utilitários aqui vazariam para o `dist/arkhe-ui.css`).
import './app.css';

import { App } from './App';

const container = document.getElementById('root');
if (container === null) {
  throw new Error('index.html não tem <div id="root"> — a app não tem onde montar.');
}

createRoot(container).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
