import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

/**
 * Config da **aplicação** (Fase 4) — deliberadamente separada de
 * `vite.config.ts`, que continua a ser o build da **biblioteca**.
 *
 * A separação é o ponto: `vite.config.ts` produz `dist/` (o pacote que o
 * `main`/`exports` do `package.json` vendem e que outras apps do monorepo
 * consomem, com `react` externalizado). Este produz um bundle de aplicação
 * normal — `react` incluído, entrada `index.html` — em `dist-app/`.
 *
 * `dist-app/` e não `dist/`: partilhar directoria faria com que um
 * `build:app` apagasse o pacote da biblioteca.
 *
 * Sem `@tailwindcss/vite` de propósito: a app não escreve utilidades do
 * Tailwind (ver o cabeçalho de `src/app/app.css`). Os estilos vêm do pacote
 * construído (`@arkhe/ui/styles.css`) mais o CSS semântico da app.
 *
 * **Pré-requisito:** `dist/` tem de existir — a app importa
 * `@arkhe/ui/styles.css`, que o `exports` mapeia para `dist/arkhe-ui.css`. Por
 * isso os scripts `dev:app`/`build:app` do `package.json` correm `npm run
 * build` (a biblioteca) antes deste config. Sem isso, o erro é uma pilha de
 * resolução em vez de uma mensagem.
 */
export default defineConfig({
  plugins: [react()],
  build: {
    outDir: 'dist-app',
    emptyOutDir: true,
  },
  server: {
    // O adaptador importa o `pkg-web` gerado a partir do crate Rust, que vive
    // fora desta directoria (`../safe-core-monorepo/...`). Sem isto o servidor
    // de desenvolvimento recusa servir o módulo por estar fora da raiz.
    fs: { allow: ['..'] },
  },
});
