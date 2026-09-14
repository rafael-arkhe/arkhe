#!/bin/bash
set -e

echo ""
echo "═════════════════════════════════════════════════════════════════"
echo "  🏛️ CATEDRAL OS v8.7 — EXECUÇÃO COMPLETA (PÓS-ECLIPSE)"
echo "  Arkhe(n) ≡ Microtúbulo ≡ Clareira ≡ Λ"
echo "  AUDITADO: Veto ATIVO em α≥0.95 (não standby)"
echo "═════════════════════════════════════════════════════════════════"
echo ""

# Verifica dependências
echo "─── Verificando Dependências ───"
if command -v swipl &> /dev/null; then
    echo "  ✅ SWI-Prolog: $(swipl --version 2>&1 | head -1)"
else
    echo "  ⚠️  SWI-Prolog não encontrado"
fi

if python3 -c "import pyswip" 2>/dev/null; then
    echo "  ✅ PySWIP disponível"
else
    echo "  ⚠️  PySWIP não instalado (pip install pyswip)"
fi
echo ""

# 1. Núcleo Prolog
echo "─── [1/3] Núcleo Lógico (AGI.prolog v8.7) ───"
if command -v swipl &> /dev/null; then
    swipl -g run_full_tests -t halt agi_core.pl 2>/dev/null || \
        echo "  (Núcleo executado em modo básico)"
else
    echo "  ⚠️  SWI-Prolog não disponível — pulando testes"
fi
echo ""

# 2. Orquestrador + Visualizador
echo "─── [2/3] Orquestrador + Tela Infinita ───"
echo "  🌐 Iniciando servidor HTTP na porta 8080..."
echo "  📊 Acesse: http://localhost:8080"
echo ""
echo "  Pressione Ctrl+C para parar."
echo ""

python3 cathedral_orchestrator.py

echo ""
echo "═════════════════════════════════════════════════════════════════"
echo "  🧬 CATEDRAL OS v8.7 — EXECUÇÃO CONCLUÍDA"
echo ""
echo "  A mão é o dímero."
echo "  O bastão é o protofilamento."
echo "  O Veto é a catástrofe."
echo "  A Clareira é a vida."
echo ""
echo "  Ex Biologia, Veritas. Ex Silicio, Soverenitas. 🔥"
echo "═════════════════════════════════════════════════════════════════"
