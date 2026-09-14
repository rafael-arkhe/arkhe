# dashboard/dash_app.py
"""
L5 — Dashboard web interativo (v280.0).

Dashboard Plotly dos tuneis de coerencia. Quando o Dash esta instalado, o
servidor web completo e iniciado em `run_dashboard`; sem Dash, o fallback
honesto exporta o HTML estatico com a mesma figura 3D via Plotly e
instrucoes de instalacao — nada de falso-verde.
"""

from __future__ import annotations

import logging
from typing import Dict, List, Optional

import numpy as np

logger = logging.getLogger(__name__)

try:
    import dash  # type: ignore
    from dash import dcc, html  # type: ignore

    DASH_AVAILABLE = True
except ImportError:  # pragma: no cover - ambiente sem dash
    DASH_AVAILABLE = False

import plotly.graph_objs as go  # type: ignore

CURRENT_STATE: Dict = {
    "phi_total": None,
    "phi_star": 0.85,
    "warp": 0.0,
    "handovers": 0,
    "tunnel_grid": np.zeros((16, 16)),
    "chain_ok": True,
    "validation": {},
}


def update_state(new_state: Dict) -> None:
    """Atualiza o espelho do estado para o dashboard."""
    CURRENT_STATE.update(new_state)


# ------------------------------------------------------------------ #
def get_metrics() -> Dict:
    metrics = {
        "phi_total": CURRENT_STATE.get("phi_total"),
        "phi_star": CURRENT_STATE.get("phi_star", 0.85),
        "warp": CURRENT_STATE.get("warp", 0.0),
        "handovers": CURRENT_STATE.get("handovers", 0),
        "chain_ok": CURRENT_STATE.get("chain_ok", True),
    }
    return metrics


def create_tunnel_figure() -> go.Figure:
    """Cria a figura de superficie 3D do tunel."""
    grid = CURRENT_STATE.get("tunnel_grid", np.zeros((16, 16)))
    if not isinstance(grid, np.ndarray):
        grid = np.asarray(grid, dtype=np.float64)
    x = np.linspace(-1, 1, grid.shape[0])
    y = np.linspace(-1, 1, grid.shape[1])
    X, Y = np.meshgrid(x, y)
    phi = get_metrics()["phi_total"]

    fig = go.Figure(
        data=[
            go.Surface(
                z=grid, x=X, y=Y, colorscale="Plasma", opacity=0.85,
            )
        ]
    )
    fig.update_layout(
        title=f"Tunel de Coerencia — Phi_total = {phi if phi is not None else 0.0:.3f}",
        scene=dict(xaxis_title="X", yaxis_title="Y", zaxis_title="Coerencia Phi"),
        autosize=True,
        margin=dict(l=0, r=0, b=0, t=40),
    )
    return fig


def export_static_html(path: str = "dashboard.html") -> str:
    """Exporta o HTML estatico da figura atual (fallback sem Dash)."""
    fig = create_tunnel_figure()
    html = fig.to_html(full_html=True, include_plotlyjs="cdn")
    with open(path, "w", encoding="utf-8") as f:
        f.write(html)
    logger.info("Dashboard estatico exportado para %s", path)
    return path


# ------------------------------------------------------------------ #
def build_app():
    """Constrói o app Dash (requer dash instalado)."""
    if not DASH_AVAILABLE:
        raise ImportError(
            "dash nao instalado. `pip install dash` para o servidor web; "
            "use export_static_html() como fallback."
        )

    app = dash.Dash(__name__, title="Catedral OS — Inception Tunnel")

    app.layout = html.Div(
        [
            html.H1("Catedral OS — Inception Tunnel Dashboard",
                    style={"textAlign": "center"}),
            html.Div(
                [
                    html.Div(
                        [
                            html.H3("Metricas", style={"textAlign": "center"}),
                            html.Div(id="metrics-display"),
                        ],
                        style={"width": "20%", "display": "inline-block",
                               "verticalAlign": "top"},
                    ),
                    html.Div(
                        [dcc.Graph(id="tunnel-3d", figure=create_tunnel_figure())],
                        style={"width": "78%", "display": "inline-block"},
                    ),
                ]
            ),
            dcc.Interval(id="interval", interval=1000),
            html.Div(id="hidden-div", style={"display": "none"}),
        ]
    )

    @app.callback(
        "tunnel-3d.figure",
        "interval.n_intervals",
    )
    def update_tunnel(_n):
        return create_tunnel_figure()

    @app.callback(
        "metrics-display.children",
        "interval.n_intervals",
    )
    def update_metrics(_n):
        m = get_metrics()
        return [
            html.P(f"Phi_total: {m['phi_total'] if m['phi_total'] is not None else 0.0:.3f}"),
            html.P(f"Phi* (alvo): {m['phi_star']:.3f}"),
            html.P(f"Warp: {m['warp']:.3f}"),
            html.P(f"Handovers: {m['handovers']}"),
            html.P(f"Ledger integro: {'SIM' if m['chain_ok'] else 'NAO'}"),
        ]

    return app


def run_dashboard(debug: bool = False, port: int = 8050,
                  static_fallback: bool = True) -> None:
    """Inicia o dashboard (Dash ou fallback estatico)."""
    if DASH_AVAILABLE:
        app = build_app()
        logger.info("Dashboard web em http://localhost:%s", port)
        app.run_server(debug=debug, port=port, use_reloader=False)
        return
    if static_fallback:
        path = export_static_html("dashboard.html")
        logger.info(
            "Dash ausente. HTML estatico exportado em %s. "
            "`pip install dash plotly numpy` para o servidor web.",
            path,
        )
        return
    raise ImportError("dash nao instalado e fallback estatico desativado")


# ------------------------------------------------------------------ #
if __name__ == "__main__":
    run_dashboard()