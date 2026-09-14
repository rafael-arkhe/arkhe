import type { StorybookConfig } from '@storybook/react-vite';

const config: StorybookConfig = {
  stories: ['../src/**/*.stories.@(ts|tsx)'],
  /**
   * `@storybook/addon-vitest` está AUSENTE de propósito nesta fase.
   * Motivo: 10.6.0 declara `vitest: ^3.0.0 || ^4.0.0` como peer, e a spec
   * fixa `vitest@5.0.0` → `npm install` rebenta com ERESOLVE. Além disso o
   * addon exige `@vitest/browser` + browsers do Playwright, que esta fase
   * não instala. Ver README, secção "não feito / não verificado".
   */
  addons: ['@storybook/addon-a11y'],
  framework: {
    name: '@storybook/react-vite',
    options: {},
  },
};

export default config;
