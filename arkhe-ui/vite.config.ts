import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import tailwindcss from '@tailwindcss/vite';

/**
 * Build de biblioteca (não de aplicação): o `dist/` é consumido por
 * outras apps do monorepo. `react`/`react-dom` são externos — duplicá-los
 * quebraria os hooks no consumidor.
 *
 * Nota: `entry` é relativo à raiz do projecto, para não depender de
 * `__dirname`/`import.meta.url` (que divergem entre Windows e POSIX).
 */
export default defineConfig({
  plugins: [react(), tailwindcss()],
  build: {
    lib: {
      entry: 'src/index.ts',
      formats: ['es'],
      fileName: () => 'index.js',
      // Por omissão o Vite derivaria o nome do pacote (`ui.css`) a partir de
      // `@arkhe/ui`; fixamos um nome estável para o `exports` público.
      cssFileName: 'arkhe-ui',
    },
    rollupOptions: {
      external: ['react', 'react-dom', 'react/jsx-runtime', 'react/jsx-dev-runtime'],
    },
    sourcemap: true,
    emptyOutDir: true,
  },
});
