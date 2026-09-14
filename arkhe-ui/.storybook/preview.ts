import type { Preview } from '@storybook/react';

// Carrega os tokens + utilitários para as stories renderizarem com o tema.
// (Única linha acrescentada ao preview da spec; o bloco `a11y` abaixo é
//  idêntico, byte a byte, ao aprovado.)
import '../src/styles/globals.css';

const preview: Preview = {
  parameters: {
    a11y: {
      config: { rules: [
        { id: 'color-contrast',        enabled: true },
        { id: 'focus-order-semantics', enabled: true },
        { id: 'tabindex',              enabled: true },
      ]},
      test: 'error',
    },
  },
};

export default preview;
