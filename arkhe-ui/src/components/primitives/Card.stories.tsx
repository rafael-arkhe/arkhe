import type { Meta, StoryObj } from '@storybook/react';

import { Button } from './Button';
import {
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from './Card';
import { Badge } from './Badge';

const meta = {
  title: 'Primitivos/Card',
  component: Card,
  parameters: { layout: 'centered' },
  argTypes: {
    variant: { control: 'select', options: ['panel', 'surface', 'interactive'] },
    padding: { control: 'select', options: ['none', 'sm', 'md', 'lg'] },
  },
  args: { variant: 'panel', padding: 'md' },
} satisfies Meta<typeof Card>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Panel: Story = {
  render: (args) => (
    <Card {...args} className="w-80">
      <CardHeader>
        <CardTitle>arkhe-verify</CardTitle>
        <CardDescription>Verificação formal do artefacto</CardDescription>
      </CardHeader>
      <CardContent>3 de 4 invariantes provadas.</CardContent>
      <CardFooter>
        <Badge status="partial">parcial</Badge>
      </CardFooter>
    </Card>
  ),
};

export const Surface: Story = {
  args: { variant: 'surface' },
  render: (args) => (
    <Card {...args} className="w-80">
      <CardHeader>
        <CardTitle>arkhe-verify</CardTitle>
        <CardDescription>Verificação formal do artefacto</CardDescription>
      </CardHeader>
      <CardContent>3 de 4 invariantes provadas.</CardContent>
      <CardFooter>
        <Badge status="partial">parcial</Badge>
      </CardFooter>
    </Card>
  ),
};

export const Interactive: Story = {
  args: { variant: 'interactive' },
  render: (args) => (
    <Card {...args} className="w-80">
      <CardHeader>
        <CardTitle>Skill: sum-check</CardTitle>
        <CardDescription>Inspectável por teclado</CardDescription>
      </CardHeader>
      <CardContent className="flex items-center justify-between">
        <span className="text-fg-muted">estado</span>
        <Badge status="verified">verificado</Badge>
      </CardContent>
      <CardFooter>
        <Button size="sm" variant="secondary">
          Abrir
        </Button>
      </CardFooter>
    </Card>
  ),
};
