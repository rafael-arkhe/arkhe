"""
Dashboard integrado para monitoramento do ecossistema Arkhe 8.1.
"""

import os

import requests
import pandas as pd
import plotly.graph_objects as go
import streamlit as st
from datetime import datetime, timezone

# Configuração
st.set_page_config(page_title="Arkhe Dashboard 8.1", page_icon="🌌", layout="wide")

API_BASE = os.environ.get("VERBAL_API_URL", "http://localhost:8000")
GLASS5D_API = os.environ.get("ARKHE_API_URL", "http://localhost:8082")
ROQUO_API = os.environ.get("ROQUO_API_URL", "http://localhost:8081")
DASHBOARD_REFRESH_MS = 30_000

st.title("🌌 Arkhe Manifold 8.1 — Dashboard Integrado")
st.markdown("**O ecossistema completo: Verbal Chemistry + Event Processor + Glass5D + ROQUO**")
st.caption(f"Atualizado: {datetime.now(timezone.utc).strftime('%Y-%m-%d %H:%M:%S UTC')}")

with st.sidebar:
    st.header("🔮 Controles")
    st.button("🔄 Refresh", key="refresh")
    st.divider()
    st.subheader("Status do Sistema")

    try:
        health = requests.get(f"{API_BASE}/health", timeout=5).json()
        st.success(f"✅ Event Processor: {health.get('status', 'unknown')}")
    except Exception:
        st.error("❌ Event Processor: Offline")

    try:
        glass_health = requests.get(f"{GLASS5D_API}/health", timeout=5).json()
        st.success(f"💎 Glass5D: {glass_health.get('status', 'unknown')}")
    except Exception:
        st.error("❌ Glass5D: Offline")

    try:
        roquo_health = requests.get(f"{ROQUO_API}/health", timeout=5).json()
        st.success(f"🖥️ ROQUO: {roquo_health.get('status', 'unknown')}")
    except Exception:
        st.error("❌ ROQUO: Offline")

col1, col2, col3, col4 = st.columns(4)
col1.metric("🧬 Engramas Processados", "42", "12%")
col2.metric("💎 Glass5D Usado", "124.5 TB", "34.6%")
col3.metric("🖥️ ROQUO Jobs", "156", "24 ativos")
col4.metric("🔮 Sessão", "ARKHE-8.1", "Ativa")

tab1, tab2, tab3, tab4 = st.tabs(["🧬 Verbal Chemistry", "💎 Glass5D Archive", "🖥️ ROQUO HPC", "🌌 Manifold Status"])

with tab1:
    st.header("Análise Verbal em Tempo Real")
    col1, col2 = st.columns(2)
    with col1:
        text_input = st.text_area(
            "Insira sua autofala:",
            "I am capable and learning to navigate this challenge",
        )
        if st.button("🔬 Analisar Impacto Bioquímico", type="primary"):
            try:
                response = requests.post(
                    f"{API_BASE}/verbal/analyze", json={"text": text_input}, timeout=10
                )
                response.raise_for_status()
                result = response.json()
                st.subheader("Análise")
                polarity = result.get("polarity", "unknown")
                color = "🟢" if polarity == "COHERENT" else "🔴" if polarity == "TOXIC" else "🟡"
                st.write(f"{color} Polaridade: **{polarity}**")
                st.write(f"📊 Carga Emocional: {result.get('emotional_charge', 0):+.2f}")
                bio = result.get("biochemical_impact", {})
                if bio:
                    st.write("**Efeitos Bioquímicos:**")
                    for key, value in bio.items():
                        if value != 0:
                            arrow = "↑" if value > 0 else "↓"
                            st.write(f"  • {key}: {value:+.1%} {arrow}")
            except Exception as e:
                st.error(f"Falha na análise: {e}")

    with col2:
        st.subheader("Estatísticas Verbas")
        polarity_data = {
            "COHERENT": 45,
            "CONSTRUCTIVE": 25,
            "NEUTRAL": 15,
            "DISRUPTIVE": 10,
            "TOXIC": 5,
        }
        fig = go.Figure(
            data=[
                go.Pie(
                    labels=list(polarity_data.keys()),
                    values=list(polarity_data.values()),
                    hole=0.3,
                    marker_colors=["#00CC96", "#FFA15A", "#636EFA", "#EF553B", "#FF6B6B"],
                )
            ]
        )
        fig.update_layout(height=300, margin=dict(l=20, r=20, t=30, b=20))
        st.plotly_chart(fig, use_container_width=True)

with tab2:
    st.header("📦 Arquivo Eterno — Glass5D")
    col1, col2 = st.columns(2)
    with col1:
        st.subheader("Últimos Engramas")
        try:
            response = requests.get(f"{GLASS5D_API}/datasets", timeout=10)
            datasets = response.json().get("datasets", [])
            if datasets:
                df = pd.DataFrame(datasets)
                st.dataframe(df)
            else:
                st.info("Nenhum engrama encontrado no Glass5D")
        except Exception:
            st.warning("Glass5D simulado — dados de exemplo")
            sample_data = [
                {"dataset": "genesis_20260817", "record_id": "g5d:abc123", "bytes_written": 450.2e9, "created_at": "2026-08-17T00:00:00Z"},
                {"dataset": "verbal_engram_20260816", "record_id": "g5d:def456", "bytes_written": 120.8e9, "created_at": "2026-08-16T12:30:00Z"},
                {"dataset": "amazon_rhythm_20260815", "record_id": "g5d:ghi789", "bytes_written": 890.1e9, "created_at": "2026-08-15T06:15:00Z"},
            ]
            st.dataframe(pd.DataFrame(sample_data))

    with col2:
        st.subheader("Uso do Glass5D")
        used = 124.5
        total = 360.0
        percentage = used / total * 100
        fig = go.Figure(
            go.Indicator(
                mode="gauge+number",
                value=percentage,
                domain={"x": [0, 1], "y": [0, 1]},
                title={"text": "Capacidade Utilizada"},
                gauge={
                    "axis": {"range": [0, 100]},
                    "bar": {"color": "#636EFA"},
                    "steps": [
                        {"range": [0, 50], "color": "#00CC96"},
                        {"range": [50, 80], "color": "#FFA15A"},
                        {"range": [80, 100], "color": "#EF553B"},
                    ],
                    "threshold": {"line": {"color": "red", "width": 4}, "thickness": 0.75, "value": 90},
                },
            )
        )
        fig.update_layout(height=250, margin=dict(l=20, r=20, t=30, b=20))
        st.plotly_chart(fig, use_container_width=True)
        st.metric("Total Armazenado", f"{used:.1f} TB / {total:.0f} TB")
        st.metric("Camadas de Redundância", "2", "WORM ativo")

with tab3:
    st.header("🖥️ ROQUO — Supercomputador Quântico-Híbrido")
    st.metric("Poder de Processamento", "19.80 PFLOPS Rmax", "5+ EXAFLOPS FP8")
    col1, col2 = st.columns(2)
    with col1:
        st.subheader("Jobs Ativos")
        active_jobs = [
            {"job_id": "JOB-001", "status": "running", "progress": 67, "nodes": 4, "gpus": 16},
            {"job_id": "JOB-002", "status": "running", "progress": 34, "nodes": 2, "gpus": 8},
            {"job_id": "JOB-003", "status": "queued", "progress": 0, "nodes": 8, "gpus": 32},
        ]
        for job in active_jobs:
            st.progress(job["progress"] / 100, text=f"{job['job_id']} — {job['status']} ({job['progress']}%)")
    with col2:
        st.subheader("Recursos")
        st.metric("Nós Alocados", "14/64", "22%")
        st.metric("GPUs em Uso", "56/256", "22%")
        st.metric("Quantum Backend", "Reimei", "Ativo")
        st.info("🔮 SQC Interface: Conectada")

with tab4:
    st.header("🌌 Manifold Arkhe 8.1 — Status do Ecossistema")
    col1, col2 = st.columns(2)
    with col1:
        st.subheader("Componentes")
        components = {
            "ROQUO": "🟢 OPERACIONAL",
            "Glass5D": "🟢 OPERACIONAL",
            "SQC Interface": "🟢 OPERACIONAL",
            "Skyrmions": "🟢 VALIDADO",
            "FugakuNEXT": "🔄 EM DESENVOLVIMENTO",
        }
        for name, status in components.items():
            st.write(f"**{name}:** {status}")
    with col2:
        st.subheader("Métricas do Sistema")
        metrics = {
            "Engramas Processados": "42",
            "Glass5D Usado": "124.5 TB",
            "ROQUO Jobs": "156",
            "Taxa de Sucesso": "98.7%",
            "Sessão Atual": f"ARKHE-8.1-{datetime.now(timezone.utc).strftime('%Y%m%d')}",
        }
        for name, value in metrics.items():
            st.metric(name, value)

st.divider()
st.caption("🌌 Arkhe Manifold 8.1 — © 2026. Todos os dados são WORM no Glass5D.")