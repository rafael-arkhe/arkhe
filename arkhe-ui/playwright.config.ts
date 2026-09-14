import { defineConfig, devices } from '@playwright/test';

/**
 * Config padrão do Playwright (chromium / firefox / webkit).
 *
 * ATENÇÃO — os browsers NÃO estão instalados neste workspace (são centenas
 * de MB). Nada nesta fase foi executado com este config. Para os instalar:
 *
 *     npx playwright install
 *
 * A suite de e2e aponta para o build estático do Storybook, portanto
 * requer `npm run build-storybook` primeiro.
 */
export default defineConfig({
  testDir: './e2e',
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: process.env.CI ? 1 : undefined,
  reporter: process.env.CI ? [['github'], ['html', { open: 'never' }]] : [['list']],
  use: {
    baseURL: 'http://127.0.0.1:6006',
    trace: 'on-first-retry',
  },
  projects: [
    { name: 'chromium', use: { ...devices['Desktop Chrome'] } },
    { name: 'firefox', use: { ...devices['Desktop Firefox'] } },
    { name: 'webkit', use: { ...devices['Desktop Safari'] } },
  ],
  webServer: {
    // `http-server` não é dependência do projecto: o npx resolve-o on-demand.
    command: 'npx --yes http-server storybook-static -p 6006 -s',
    url: 'http://127.0.0.1:6006',
    reuseExistingServer: !process.env.CI,
    timeout: 60_000,
  },
});
