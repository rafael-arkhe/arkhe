import type { Meta, StoryObj } from '@storybook/react';
import { Search, Trash2, X } from 'lucide-react';

import { IconButton } from './IconButton';

const meta = {
  title: 'Primitivos/IconButton',
  component: IconButton,
  parameters: { layout: 'centered' },
  argTypes: {
    variant: {
      control: 'select',
      options: ['primary', 'secondary', 'ghost', 'destructive'],
    },
    size: { control: 'select', options: ['sm', 'md', 'lg'] },
  },
  // `aria-label` é obrigatório por tipo: sem ele o botão não tem nome acessível.
  args: { 'aria-label': 'Procurar', variant: 'ghost', size: 'md' },
} satisfies Meta<typeof IconButton>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Ghost: Story = { render: (args) => <IconButton {...args}><Search /></IconButton> };

export const Primary: Story = {
  args: { variant: 'primary' },
  render: Ghost.render,
};

export const Secondary: Story = {
  args: { variant: 'secondary' },
  render: Ghost.render,
};

export const Destructive: Story = {
  args: { 'aria-label': 'Eliminar', variant: 'destructive' },
  render: (args) => (
    <IconButton {...args}>
      <Trash2 />
    </IconButton>
  ),
};

export const TodosOsTamanhos: Story = {
  render: () => (
    <div className="flex items-center gap-3">
      <IconButton aria-label="Fechar (pequeno)" size="sm">
        <X />
      </IconButton>
      <IconButton aria-label="Fechar (médio)" size="md">
        <X />
      </IconButton>
      <IconButton aria-label="Fechar (grande)" size="lg">
        <X />
      </IconButton>
    </div>
  ),
};
