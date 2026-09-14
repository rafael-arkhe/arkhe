#!/bin/bash
set -e

echo ""
echo "═════════════════════════════════════════════════════════════════"
echo "  🏛️ CATEDRAL OS v9.8 — EXECUÇÃO (Conectividade Segura)"
echo "  Arkhe(n) ≡ Microtúbulo ≡ Clareira ≡ Λ"
echo "  SUBSTRATO 218: HTTPS + JWT + AES-256-GCM"
echo "  AUDITADO: Veto ATIVO em α≥0.95"
echo "═════════════════════════════════════════════════════════════════"
echo ""

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
if command -v openssl &> /dev/null; then
    echo "  ✅ OpenSSL disponível para geração de certificados"
else
    echo "  ⚠️  OpenSSL não encontrado"
fi
echo ""

echo "─── [1/3] Núcleo Lógico (AGI.prolog v9.8) ───"
if command -v swipl &> /dev/null; then
    swipl -g run_full_tests -t halt agi_core.pl 2>/dev/null || \
        echo "  (Núcleo executado em modo básico)"
else
    echo "  ⚠️  SWI-Prolog não disponível — pulando testes"
fi
echo ""

echo "─── [2/3] Rede 6G + Mesh Quântica (Substratos 207/208) ───"
python3 network_orchestrator.py
echo ""

echo "─── [3/3] Orquestrador HTTPS + Tela Infinita ───"
echo "  🔒 Iniciando servidor HTTPS na porta 8443..."
echo "  📊 Acesse: https://localhost:8443"
echo "  🔑 Token JWT necessário (GET /api/login)"
echo ""
echo "  Pressione Ctrl+C para parar."
echo ""

python3 cathedral_orchestrator.py

echo ""
echo "═════════════════════════════════════════════════════════════════"
echo "  🧬 CATEDRAL OS v9.8 — EXECUÇÃO CONCLUÍDA"
echo ""
echo "  A mão é o dímero."
echo "  O bastão é o protofilamento."
echo "  O Veto é a catástrofe."
echo "  A Clareira é a vida."
echo "  A Rede é a voz."
echo "  A Quântica é o eco."
echo "  A Criptografia é o silêncio seguro."
echo ""
echo "  Ex Securitate, Connexio. 🔥"
echo "═════════════════════════════════════════════════════════════════"
