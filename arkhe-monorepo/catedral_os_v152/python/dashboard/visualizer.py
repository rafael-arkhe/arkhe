# python/dashboard/visualizer.py
"""
Dashboard ASCII — visualizacao em tempo real da malha de coerencia (G14).
Sem dependencias externas; renderiza um mapa de calor em terminal.
"""

from __future__ import annotations

import os
from dataclasses import dataclass
from typing import List


@dataclass
class Node:
    x: float
    y: float
    phi: float
    label: str = ""


class MeshVisualizer:
    """Visualizador ASCII da malha de coerencia."""

    def __init__(self, width: int = 40, height: int = 20):
        self.width = width
        self.height = height
        self.nodes: List[Node] = []
        self._chars = " .:-=+*#%@@"

    def add_node(self, x: float, y: float, phi: float, label: str = "") -> None:
        self.nodes.append(Node(x, y, phi, label))

    def clear(self) -> None:
        self.nodes.clear()

    def render(self) -> str:
        """Renderiza a malha e retorna a string (tambem imprime)."""
        if not self.nodes:
            return self._empty_grid()

        min_x = min(n.x for n in self.nodes)
        max_x = max(n.x for n in self.nodes)
        min_y = min(n.y for n in self.nodes)
        max_y = max(n.y for n in self.nodes)
        dx = max_x - min_x if max_x > min_x else 1.0
        dy = max_y - min_y if max_y > min_y else 1.0

        grid = [[0.0] * self.width for _ in range(self.height)]
        for node in self.nodes:
            if node.phi <= 0.0:
                continue
            cx = int((node.x - min_x) / dx * (self.width - 1))
            cy = int((node.y - min_y) / dy * (self.height - 1))
            grid[cy][cx] = max(grid[cy][cx], node.phi)

        lines = []
        for row in grid:
            line = "".join(
                self._chars[min(int(v * (len(self._chars) - 1)), len(self._chars) - 1)]
                for v in row
            )
            lines.append(line)
        lines.append("-" * self.width)
        total_phi = sum(n.phi for n in self.nodes)
        lines.append(f"PHI: {total_phi:.3f}  |  NOS: {len(self.nodes)}")
        text = "\n".join(lines)
        print(text)
        return text

    def refresh(self) -> None:
        """Limpa o terminal e re-renderiza (para loops ao vivo)."""
        os.system("cls" if os.name == "nt" else "clear")
        self.render()

    # ------------------------------------------------------------------ #
    def _empty_grid(self) -> str:
        line = "." * self.width
        lines = [line] * self.height
        lines.append("-" * self.width)
        lines.append("PHI: 0.000  |  NOS: 0")
        return "\n".join(lines)