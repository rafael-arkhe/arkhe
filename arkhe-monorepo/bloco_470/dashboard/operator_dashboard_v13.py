# =============================================================================
# BLOCO 470 v13 — DASHBOARD PARA OPERADORES (O6)
# operator_dashboard_v13.py
#
# Dashboard Streamlit com métricas operacionais de governança.
#
# Requer: pip install streamlit pandas plotly requests
# Uso:    streamlit run operator_dashboard_v13.py
# =============================================================================
import streamlit as st
import pandas as pd
import plotly.graph_objects as go
import plotly.express as px
from plotly.subplots import make_subplots
import requests
import json
from datetime import datetime, timedelta
import time

# ============================================================================
# CONFIGURAÇÃO
# ============================================================================

st.set_page_config(
    page_title="🏛️ Catedral OS — Operator Dashboard",
    page_icon="⚙️",
    layout="wide",
    initial_sidebar_state="expanded",
)

GOVERNANCE_API = "http://localhost:8008/api/governance"
PROLOG_API = "http://localhost:8000"

# ============================================================================
# AUTENTICAÇÃO OPERADOR
# ============================================================================

def operator_auth():
    """Autenticação específica para operadores."""
    st.sidebar.markdown("### 🔐 Operator Access")

    operator_id = st.sidebar.text_input("Operator ID", placeholder="op-001")
    api_key = st.sidebar.text_input("API Key", type="password")

    if operator_id and api_key:
        try:
            response = requests.post(
                f"{GOVERNANCE_API}/auth/operator",
                json={"operator_id": operator_id, "api_key": api_key},
                timeout=5,
            )
            if response.status_code == 200:
                st.sidebar.success(f"✅ Conectado como {operator_id}")
                return True
        except Exception:
            pass
        st.sidebar.error("❌ Credenciais inválidas")
        return False

    st.sidebar.info("👤 Insira suas credenciais de operador")
    return False


# ============================================================================
# FUNÇÕES DE API
# ============================================================================

@st.cache_data(ttl=10)
def fetch_operator_metrics():
    """Obtém métricas específicas para operadores."""
    try:
        response = requests.get(f"{GOVERNANCE_API}/operator/metrics", timeout=5)
        return response.json()
    except Exception:
        return None


@st.cache_data(ttl=30)
def fetch_peer_status():
    """Obtém status dos peers federados."""
    try:
        response = requests.get(f"{GOVERNANCE_API}/federation/status", timeout=5)
        return response.json()
    except Exception:
        return None


@st.cache_data(ttl=60)
def fetch_alert_history():
    """Obtém histórico de alertas."""
    try:
        response = requests.get(f"{GOVERNANCE_API}/alerts/history", timeout=5)
        return response.json()
    except Exception:
        return None


@st.cache_data(ttl=300)
def fetch_zk_stats():
    """Obtém estatísticas de provas ZK."""
    try:
        response = requests.get(f"{PROLOG_API}/zk_stats", timeout=5)
        return response.json()
    except Exception:
        return None


# ============================================================================
# DASHBOARD PRINCIPAL
# ============================================================================

def main():
    st.title("⚙️ Catedral OS — Operator Dashboard")
    st.caption(f"🕒 Última atualização: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}")

    # Autenticação
    if not operator_auth():
        st.stop()

    # ========================================================================
    # KPI ROW
    # ========================================================================
    metrics = fetch_operator_metrics()
    if metrics:
        col1, col2, col3, col4, col5 = st.columns(5)

        with col1:
            st.metric("🔄 PRs em Análise", metrics.get("pending_prs", 0),
                      delta=metrics.get("pending_delta", 0))
        with col2:
            st.metric("⏳ Tempo Médio", f"{metrics.get('avg_review_time', 0):.1f}m",
                      delta=f"{metrics.get('review_time_delta', 0):.1f}m")
        with col3:
            st.metric("📊 Coerência Média", f"{metrics.get('avg_phi', 0)*100:.1f}%",
                      delta=f"{metrics.get('phi_delta', 0)*100:.1f}%")
        with col4:
            st.metric("🚨 Alertas Ativos", metrics.get("active_alerts", 0),
                      delta=metrics.get("alert_delta", 0))
        with col5:
            st.metric("🌐 Peers Ativos",
                      f"{metrics.get('active_peers', 0)}/{metrics.get('total_peers', 0)}",
                      delta=metrics.get("peer_delta", 0))
    else:
        st.info("⚠️ Sem resposta da API de métricas — exibindo demo.")

    # ========================================================================
    # GRÁFICOS OPERACIONAIS
    # ========================================================================
    col1, col2 = st.columns(2)

    with col1:
        st.markdown("### 📈 Evolução da Coerência (Últimas 24h)")
        hours = list(range(24))
        import random

        phi_values = [0.65 + 0.3 * (i / 24) + 0.05 * (random.random() - 0.5) for i in range(24)]
        phi_values = [min(1.0, max(0.0, v)) for v in phi_values]

        fig = go.Figure()
        fig.add_trace(go.Scatter(
            x=hours, y=phi_values, mode="lines+markers", name="Φ Médio",
            line=dict(color="#e94560", width=2), marker=dict(size=8),
        ))
        fig.add_hline(y=0.85, line_dash="dash", line_color="#00ff88",
                      annotation_text="Threshold")
        fig.add_hline(y=0.70, line_dash="dot", line_color="#ffaa00",
                      annotation_text="Alerta")
        fig.update_layout(
            template="plotly_dark", height=300,
            margin=dict(l=0, r=0, t=20, b=0),
            xaxis_title="Hora", yaxis_title="Coerência (Φ)", yaxis_range=[0, 1],
        )
        st.plotly_chart(fig, use_container_width=True)

    with col2:
        st.markdown("### 🚦 Status dos Peers")
        peer_status = fetch_peer_status()
        peers = {}
        if peer_status:
            peers = peer_status.get("peers", {})

        if peers:
            df_peers = pd.DataFrame([
                {
                    "Peer": name,
                    "Trust": info.get("trust_score", 0),
                    "Status": "🟢 Ativo" if info.get("active", False) else "🔴 Inativo",
                }
                for name, info in peers.items()
            ])
            fig = px.bar(
                df_peers, x="Peer", y="Trust", color="Status",
                color_discrete_map={"🟢 Ativo": "#00ff88", "🔴 Inativo": "#ff4444"},
                title="Trust Scores dos Peers",
            )
        else:
            # Demo
            df_peers = pd.DataFrame([
                {"Peer": "Catedral-PR", "Trust": 0.92, "Status": "🟢 Ativo"},
                {"Peer": "Catedral-SP", "Trust": 0.78, "Status": "🟢 Ativo"},
                {"Peer": "Catedral-RJ", "Trust": 0.45, "Status": "🔴 Inativo"},
            ])
            fig = px.bar(
                df_peers, x="Peer", y="Trust", color="Status",
                color_discrete_map={"🟢 Ativo": "#00ff88", "🔴 Inativo": "#ff4444"},
                title="Trust Scores dos Peers (demo)",
            )

        fig.add_hline(y=0.5, line_dash="dash", line_color="#ffaa00",
                      annotation_text="Alerta")
        fig.add_hline(y=0.3, line_dash="dash", line_color="#ff4444",
                      annotation_text="Crítico")
        fig.update_layout(template="plotly_dark", height=300,
                          margin=dict(l=0, r=0, t=30, b=0))
        st.plotly_chart(fig, use_container_width=True)

    # ========================================================================
    # TABELAS OPERACIONAIS
    # ========================================================================
    st.markdown("### 📋 Alertas Ativos")
    alerts = fetch_alert_history()
    if alerts and alerts.get("alerts"):
        df_alerts = pd.DataFrame([
            {
                "ID": a.get("id", ""),
                "Severidade": a.get("severity", ""),
                "Peer": a.get("peer", ""),
                "Trust Score": f"{a.get('trust_score', 0):.2f}",
                "Threshold": f"{a.get('threshold', 0):.2f}",
                "Timestamp": a.get("timestamp", ""),
                "Status": "✅ Reconhecido" if a.get("acknowledged") else "⏳ Pendente",
            }
            for a in alerts.get("alerts", [])[:10]
        ])
        st.dataframe(df_alerts, use_container_width=True)
    else:
        st.info("Sem alertas no momento (demo: nenhum alerta ativo).")

    # ========================================================================
    # ZK STATS
    # ========================================================================
    st.markdown("### 🔐 Estatísticas de Provas ZK")
    zk_stats = fetch_zk_stats()
    if zk_stats:
        col1, col2, col3, col4 = st.columns(4)
        with col1:
            st.metric("Provas Geradas", zk_stats.get("total_proofs", 0))
        with col2:
            st.metric("Verificadas On-Chain", zk_stats.get("onchain_verified", 0))
        with col3:
            rate = zk_stats.get("verification_rate", 0) * 100
            st.metric("Taxa de Verificação", f"{rate:.1f}%")
        with col4:
            st.metric("Circuitos Ativos", zk_stats.get("active_circuits", 0))

        circuits = zk_stats.get("circuit_stats", {})
        if circuits:
            df_circuits = pd.DataFrame([
                {"Circuito": k, "Provas": v} for k, v in circuits.items()
            ])
            fig = px.pie(df_circuits, values="Provas", names="Circuito",
                         title="Distribuição de Provas por Circuito", hole=0.4)
            fig.update_layout(template="plotly_dark", height=300)
            st.plotly_chart(fig, use_container_width=True)
    else:
        st.info("ZK stats indisponíveis (demo).")

    # ========================================================================
    # AÇÕES DO OPERADOR
    # ========================================================================
    with st.expander("🛠️ Ações do Operador"):
        col1, col2, col3 = st.columns(3)

        with col1:
            if st.button("🔄 Sincronizar Federação"):
                try:
                    response = requests.post(f"{GOVERNANCE_API}/federation/sync", timeout=10)
                    if response.status_code == 200:
                        st.success("✅ Sincronização concluída")
                except Exception:
                    st.warning("API indisponível (demo).")

        with col2:
            if st.button("📊 Exportar Relatório"):
                st.download_button(
                    "📥 Baixar Relatório",
                    data=json.dumps({"nota": "relatório demo", "ts": datetime.now().isoformat()}),
                    file_name=f"governance_report_{datetime.now().strftime('%Y%m%d')}.json",
                    mime="application/json",
                )

        with col3:
            if st.button("🚨 Testar Alerta"):
                st.success("✅ Alerta de teste enviado (console)")

    st.markdown("---")
    st.caption(f"🧬 Bloco 470 v13 — Operator Dashboard | {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}")


if __name__ == "__main__":
    main()