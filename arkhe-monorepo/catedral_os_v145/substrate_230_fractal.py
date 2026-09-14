#!/usr/bin/env python3
"""
Substrato 230 — Fractal Renderer (Mandelbrot) + Entropia de Imagem

Renderiza fractais de Mandelbrot/Julia em numpy puro (sem dependência de numba,
mantendo o mesmo resultado) e calcula a entropia de Shannon da imagem resultante.

Orquestrador usa: render_fractal(...) e image_entropy(...).
"""

import numpy as np
from typing import Dict, Optional


def render_fractal(width: int = 800, height: int = 600,
                   xmin: float = -2.0, xmax: float = 1.0,
                   ymin: float = -1.5, ymax: float = 1.5,
                   max_iter: int = 256, julia_c: Optional[complex] = None,
                   center: Optional[complex] = None,
                   zoom: float = 1.0, output_file: Optional[str] = None) -> np.ndarray:
    """
    Renderiza o conjunto de Mandelbrot (ou Julia) em um array 2D de iterações.

    Se julia_c for fornecido, renderiza o conjunto de Julia com constante c;
    caso contrário, o Mandelbrot. Suporta centralização/zoom.
    """
    if center is not None:
        xmin = center.real - (xmax - xmin) / (2 * zoom)
        xmax = center.real + (xmax - xmin) / (2 * zoom)
        ymin = center.imag - (ymax - ymin) / (2 * zoom)
        ymax = center.imag + (ymax - ymin) / (2 * zoom)

    x = np.linspace(xmin, xmax, width)
    y = np.linspace(ymin, ymax, height)
    X, Y = np.meshgrid(x, y)
    C = X + 1j * Y

    if julia_c is None:
        Z = np.zeros_like(C, dtype=complex)
    else:
        Z = C.copy()
        C = np.full_like(C, julia_c)

    img = np.zeros(C.shape, dtype=np.float64)
    escape = 2.0 ** 2  # critério clássico de divergência |Z|² > 4
    for i in range(max_iter):
        mask_in = np.abs(Z) ** 2 <= escape  # pontos ainda não divergidos
        Zi = Z[mask_in]
        Zi = Zi * Zi + C[mask_in]           # só evolui pontos não divergidos
        Z[mask_in] = Zi
        # marca iteração de fuga dos que passaram a divergir neste passo
        fled = mask_in & (np.abs(Z) * np.abs(Z) > escape)
        img[fled] = i
        if not mask_in.any():
            break
    # pontos que nunca divergiram recebem o número máximo de iterações
    img[img == 0] = max_iter

    if output_file:
        # Salva apenas se matplotlib estiver disponível; caso contrário, no-op.
        try:
            import matplotlib.pyplot as plt
            plt.imshow(img, cmap='inferno', extent=[xmin, xmax, ymin, ymax])
            plt.title('Mandelbrot' if julia_c is None else f'Julia c={julia_c}')
            plt.savefig(output_file, dpi=120, bbox_inches='tight')
            plt.close()
        except ImportError:
            pass
    return img


def image_entropy(image: np.ndarray, bins: int = 256) -> float:
    """
    Entropia de Shannon normalizada de uma imagem (array 2D).
    Alta entropia → alta complexidade/aleatoriedade visual.
    """
    if image.ndim == 3:
        image = image.mean(axis=2)
    image = np.asarray(image, dtype=np.float64)
    hist, _ = np.histogram(image, bins=bins)
    hist = hist.astype(np.float64)
    total = hist.sum()
    if total <= 0:
        return 0.0
    prob = hist / total
    prob = prob[prob > 0]
    ent = -np.sum(prob * np.log(prob))
    # normaliza pelo log(bins) máximo possível
    return float(ent / np.log(bins))


def fractal_summary(width: int = 400, height: int = 300) -> Dict:
    """Produz um resumo do fractal Mandelbrot (usado pelo orquestrador)."""
    img = render_fractal(width, height, max_iter=128)
    return {
        "width": width,
        "height": height,
        "entropy": image_entropy(img),
        "mean_iter": float(np.mean(img)),
        "max_iter": float(np.max(img)),
    }


if __name__ == "__main__":
    summary = fractal_summary()
    print(f"Fractal Mandelbrot {summary['width']}x{summary['height']}: "
          f"entropia={summary['entropy']:.3f}")
