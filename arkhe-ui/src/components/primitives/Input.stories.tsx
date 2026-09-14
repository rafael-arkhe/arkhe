import type { Meta, StoryObj } from '@storybook/react';

import { Input } from './Input';

const meta = {
  title: 'Primitivos/Input',
  component: Input,
  parameters: { layout: 'centered' },
  argTypes: {
    variant: { control: 'select', options: ['default', 'strong'] },
    size: { control: 'select', options: ['sm', 'md', 'lg'] },
  },
  // `aria-label` garante nome acessível nas stories (o Input não o inventa).
  args: { 'aria-label': 'Identificador Arkhe', placeholder: 'arkhe://' },
} satisfies Meta<typeof Input>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

export const Strong: Story = { args: { variant: 'strong' } };

export const Invalid: Story = {
  args: { 'aria-invalid': 'true', defaultValue: 'valor inválido' },
};

export const Disabled: Story = { args: { disabled: true, defaultValue: 'bloqueado' } };

export const ComRotuloVisivel: Story = {
  render: () => (
    <div className="flex w-72 flex-col gap-1.5">
      <label htmlFor="arkhe-node" className="text-sm font-medium text-fg">
        Nó de destino
      </label>
      <Input id="arkhe-node" placeholder="arkhe://" defaultValue="" />
    </div>
  ),
};

export const TodosOsTamanhos: Story = {
  render: () => (
    <div className="flex w-72 flex-col gap-3">
      <Input size="sm" aria-label="Pequeno" placeholder="pequeno" />
      <Input size="md" aria-label="Médio" placeholder="médio" />
      <Input size="lg" aria-label="Grande" placeholder="grande" />
    </div>
  ),
};
