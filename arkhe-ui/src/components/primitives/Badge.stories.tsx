import type { Meta, StoryObj } from '@storybook/react';

import { Badge } from './Badge';

const meta = {
  title: 'Primitivos/Badge',
  component: Badge,
  parameters: { layout: 'centered' },
  argTypes: {
    status: { control: 'select', options: ['verified', 'partial', 'pending', 'error'] },
  },
  args: { status: 'verified', children: 'verificado' },
} satisfies Meta<typeof Badge>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Verified: Story = {};

export const Partial: Story = { args: { status: 'partial', children: 'parcial' } };

export const Pending: Story = { args: { status: 'pending', children: 'pendente' } };

export const Error: Story = { args: { status: 'error', children: 'erro' } };

export const TodosOsEstados: Story = {
  render: () => (
    <div className="flex flex-wrap items-center gap-2">
      <Badge status="verified">verificado</Badge>
      <Badge status="partial">parcial</Badge>
      <Badge status="pending">pendente</Badge>
      <Badge status="error">erro</Badge>
    </div>
  ),
};
