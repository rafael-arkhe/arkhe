import { describe, expect, it, vi } from 'vitest';
import { createRef } from 'react';

import {
  Badge,
  Button,
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
  IconButton,
  Input,
  badgeVariants,
  buttonVariants,
  cardVariants,
  iconButtonVariants,
  inputVariants,
} from '../src/components/primitives';
import { cn } from '../src/lib/cn';
import { click, render } from './render';

describe('cn()', () => {
  it('junta classes e resolve conflitos do Tailwind (a última vence)', () => {
    expect(cn('p-2', 'p-4')).toBe('p-4');
    expect(cn('text-sm', false && 'hidden', undefined, 'font-medium')).toBe(
      'text-sm font-medium',
    );
  });
});

describe('Button', () => {
  it('renderiza com data-slot e defaults (type=button evita submit acidental)', () => {
    const { container } = render(<Button>Guardar</Button>);
    const el = container.querySelector('[data-slot="button"]')!;
    expect(el.tagName).toBe('BUTTON');
    expect(el.getAttribute('type')).toBe('button');
    expect(el.getAttribute('data-variant')).toBe('primary');
    expect(el.getAttribute('data-size')).toBe('md');
  });

  it('expõe as quatro variantes com fundos/rotulos distintos', () => {
    const seen = new Set<string>();
    for (const variant of ['primary', 'secondary', 'ghost', 'destructive'] as const) {
      const { container } = render(<Button variant={variant}>x</Button>);
      const cls = container.querySelector('[data-slot="button"]')!.className;
      seen.add(cls);
      expect(cls).toBe(buttonVariants({ variant }));
    }
    expect(seen.size).toBe(4);
  });

  it('expõe os quatro tamanhos', () => {
    for (const size of ['sm', 'md', 'lg', 'icon'] as const) {
      const { container } = render(<Button size={size}>x</Button>);
      expect(container.querySelector('[data-slot="button"]')!.getAttribute('data-size')).toBe(size);
    }
  });

  it('inclui anel de foco visível e offset', () => {
    const cls = buttonVariants({ variant: 'primary' });
    expect(cls).toContain('focus-visible:ring-2');
    expect(cls).toContain('focus-visible:ring-focus-ring');
    expect(cls).toContain('focus-visible:ring-offset-2');
  });

  it('permite override de className (tailwind-merge) sem perder o anel de foco', () => {
    const { container } = render(<Button className="px-8">x</Button>);
    const cls = container.querySelector('[data-slot="button"]')!.className;
    expect(cls).toContain('px-8');
    expect(cls).not.toContain('px-4');
    expect(cls).toContain('focus-visible:ring-2');
  });

  it('encaminha o ref para o elemento nativo', () => {
    const ref = createRef<HTMLButtonElement>();
    render(<Button ref={ref}>x</Button>);
    expect(ref.current).toBeInstanceOf(HTMLButtonElement);
  });

  it('dispara onClick quando activo e NÃO dispara quando disabled', () => {
    const onClick = vi.fn();
    const first = render(<Button onClick={onClick}>x</Button>);
    click(first.container.querySelector('[data-slot="button"]')!);
    expect(onClick).toHaveBeenCalledTimes(1);

    const onClickDisabled = vi.fn();
    const second = render(
      <Button onClick={onClickDisabled} disabled>
        x
      </Button>,
    );
    const el = second.container.querySelector('[data-slot="button"]') as HTMLButtonElement;
    expect(el.disabled).toBe(true);
    click(el);
    expect(onClickDisabled).not.toHaveBeenCalled();
  });

  it('respeita type explícito', () => {
    const { container } = render(<Button type="submit">x</Button>);
    expect(container.querySelector('[data-slot="button"]')!.getAttribute('type')).toBe('submit');
  });
});

describe('Input', () => {
  it('renderiza com data-slot e default type=text', () => {
    const { container } = render(<Input aria-label="nome" />);
    const el = container.querySelector('[data-slot="input"]') as HTMLInputElement;
    expect(el.tagName).toBe('INPUT');
    expect(el.getAttribute('type')).toBe('text');
    expect(el.getAttribute('aria-label')).toBe('nome');
  });

  it('aceita as variantes e tamanhos, e o tamanho vence a base', () => {
    const heightBySize = { sm: 'h-8', md: 'h-9', lg: 'h-11' } as const;
    const paddingBySize = { sm: 'px-2', md: 'px-3', lg: 'px-4' } as const;

    for (const variant of ['default', 'strong'] as const) {
      for (const size of ['sm', 'md', 'lg'] as const) {
        const { container } = render(<Input variant={variant} size={size} aria-label="x" />);
        const el = container.querySelector('[data-slot="input"]')!;

        expect(el.getAttribute('data-variant')).toBe(variant);
        expect(el.getAttribute('data-size')).toBe(size);

        // O que o componente aplica é o resultado já passado por tailwind-merge.
        expect(el.className).toBe(cn(inputVariants({ variant, size })));
        expect(el.className).toContain(heightBySize[size]);
        expect(el.className).toContain(paddingBySize[size]);
        expect(el.className).toContain(`border-border-${variant === 'default' ? 'default' : 'strong'}`);
      }
    }
  });

  it('cada tamanho produz exactamente uma padding horizontal (sem merges ambíguos)', () => {
    for (const size of ['sm', 'md', 'lg'] as const) {
      const { container } = render(<Input size={size} aria-label="x" />);
      const cls = container.querySelector('[data-slot="input"]')!.className;
      expect(cls.match(/\bpx-\d/g) ?? []).toHaveLength(1);
    }
  });

  it('comunica o estado inválido por aria-invalid (não só por cor)', () => {
    const { container } = render(<Input aria-label="x" aria-invalid="true" />);
    const el = container.querySelector('[data-slot="input"]')!;
    expect(el.getAttribute('aria-invalid')).toBe('true');
    expect(el.className).toContain('aria-invalid:border-danger');
  });

  it('aceita valor controlado', () => {
    const { container } = render(<Input aria-label="x" value="arkhe" readOnly />);
    expect((container.querySelector('[data-slot="input"]') as HTMLInputElement).value).toBe('arkhe');
  });
});

describe('IconButton', () => {
  it('exige e aplica aria-label como nome acessível', () => {
    const { container } = render(<IconButton aria-label="Fechar">×</IconButton>);
    const el = container.querySelector('[data-slot="icon-button"]')!;
    expect(el.getAttribute('aria-label')).toBe('Fechar');
    expect(el.getAttribute('type')).toBe('button');
    expect(el.getAttribute('data-variant')).toBe('ghost');
  });

  it('garante alvo mínimo de 24px em todos os tamanhos (SC 2.5.8)', () => {
    for (const size of ['sm', 'md', 'lg'] as const) {
      const { container } = render(<IconButton aria-label="x" size={size} />);
      const el = container.querySelector('[data-slot="icon-button"]')!;
      expect(el.className).toBe(iconButtonVariants({ variant: 'ghost', size }));
      expect(el.className).toMatch(/size-(6|8|10)/);
    }
  });

  it('expõe as variantes', () => {
    for (const variant of ['primary', 'secondary', 'ghost', 'destructive'] as const) {
      const { container } = render(<IconButton aria-label="x" variant={variant} />);
      expect(container.querySelector('[data-slot="icon-button"]')!.getAttribute('data-variant')).toBe(
        variant,
      );
    }
  });
});

describe('Badge', () => {
  it('renderiza os quatro estados de verificação', () => {
    for (const status of ['verified', 'partial', 'pending', 'error'] as const) {
      const { container } = render(<Badge status={status}>{status}</Badge>);
      const el = container.querySelector('[data-slot="badge"]')!;
      expect(el.tagName).toBe('SPAN');
      expect(el.getAttribute('data-status')).toBe(status);
      expect(el.textContent).toBe(status);
      expect(el.className).toBe(badgeVariants({ status }));
    }
  });

  it('default é pending', () => {
    const { container } = render(<Badge>—</Badge>);
    expect(container.querySelector('[data-slot="badge"]')!.getAttribute('data-status')).toBe(
      'pending',
    );
  });

  it('o estado é também textual, não só cromático (SC 1.4.1)', () => {
    const { container } = render(<Badge status="verified">verificado</Badge>);
    expect(container.textContent).toContain('verificado');
  });
});

describe('Card', () => {
  it('renderiza com sub-partes e data-slot próprios', () => {
    const { container } = render(
      <Card>
        <CardHeader>
          <CardTitle>Arkhe</CardTitle>
          <CardDescription>Fase 1</CardDescription>
        </CardHeader>
        <CardContent>corpo</CardContent>
        <CardFooter>rodapé</CardFooter>
      </Card>,
    );

    expect(container.querySelector('[data-slot="card"]')).not.toBeNull();
    expect(container.querySelector('[data-slot="card-header"]')).not.toBeNull();
    expect(container.querySelector('[data-slot="card-title"]')!.tagName).toBe('H3');
    expect(container.querySelector('[data-slot="card-description"]')!.tagName).toBe('P');
    expect(container.querySelector('[data-slot="card-content"]')).not.toBeNull();
    expect(container.querySelector('[data-slot="card-footer"]')).not.toBeNull();
  });

  it('expõe variantes e paddings', () => {
    for (const variant of ['panel', 'surface', 'interactive'] as const) {
      for (const padding of ['none', 'sm', 'md', 'lg'] as const) {
        const { container } = render(<Card variant={variant} padding={padding} />);
        const el = container.querySelector('[data-slot="card"]')!;
        expect(el.className).toBe(cardVariants({ variant, padding }));
      }
    }
  });

  it('a variante interactive mostra foco para uso por teclado', () => {
    expect(cardVariants({ variant: 'interactive' })).toContain('focus-within:ring-2');
  });
});
