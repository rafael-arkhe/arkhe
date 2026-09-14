# catedral_os_v280/tests/test_dashboard.py
"""Testes do dashboard (L5): figuras Plotly + fallback estatico."""

import numpy as np
import plotly.graph_objs as go

from dashboard.dash_app import (DASH_AVAILABLE, CURRENT_STATE,
                                create_tunnel_figure, export_static_html,
                                get_metrics, update_state)


def test_metrics_defaults():
    CURRENT_STATE["phi_total"] = None
    m = get_metrics()
    assert m["phi_star"] == 0.85
    assert m["chain_ok"] is True or m["chain_ok"] is False


def test_update_state_mirror():
    update_state({"phi_total": 0.9, "tunnel_grid": np.ones((16, 16))})
    assert get_metrics()["phi_total"] == 0.9


def test_create_tunnel_figure():
    fig = create_tunnel_figure()
    assert isinstance(fig, go.Figure)
    assert len(fig.data) == 1


def test_dash_available_constant():
    # dash é opcional; a constante espelha o ambiente real.
    assert isinstance(DASH_AVAILABLE, bool)


def test_export_static_html(tmp_path):
    update_state({"phi_total": 0.7, "tunnel_grid": np.zeros((16, 16))})
    path = str(tmp_path / "dash.html")
    result = export_static_html(path)
    assert result == path
    with open(path, "r", encoding="utf-8") as f:
        html = f.read()
    assert "plotly" in html.lower()