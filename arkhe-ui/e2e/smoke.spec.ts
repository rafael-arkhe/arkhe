import { expect, test } from '@playwright/test';

/**
 * Smoke e2e sobre o build estático do Storybook.
 *
 * Pré-requisitos (NÃO satisfeitos neste workspace — ver README):
 *   1. `npm run build-storybook`   → produz `storybook-static/`
 *   2. `npx playwright install`    → descarrega os browsers
 *
 * Nada aqui foi executado na Fase 1.
 */
test.describe('Storybook estático', () => {
  test('a página do Button carrega e expõe o controlo', async ({ page }) => {
    await page.goto('/iframe.html?id=primitivos-button--primary&viewMode=story');

    const button = page.getByRole('button', { name: 'Executar' });
    await expect(button).toBeVisible();
    await expect(button).toBeEnabled();
  });

  test('o botão é alcançável e accionável por teclado', async ({ page }) => {
    await page.goto('/iframe.html?id=primitivos-button--primary&viewMode=story');

    const button = page.getByRole('button', { name: 'Executar' });
    await button.focus();
    await expect(button).toBeFocused();
  });

  test('o botão desactivado está marcado como tal', async ({ page }) => {
    await page.goto('/iframe.html?id=primitivos-button--disabled&viewMode=story');
    await expect(page.getByRole('button', { name: 'Executar' })).toBeDisabled();
  });
});
