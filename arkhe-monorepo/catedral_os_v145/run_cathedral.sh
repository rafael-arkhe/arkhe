#!/usr/bin/env bash
# Catedral OS v14.5 — script de execução/verificação
set -e
cd "$(dirname "$0")"

echo "==> Verificando dependências..."
python -c "import numpy, scipy" 2>/dev/null || {
    echo "Faltando numpy/scipy. Instale: pip install -r requirements.txt"; exit 1
}

echo "==> Rodando testes unitários..."
python -m pytest tests -q

echo "==> Rodando teste end-to-end..."
python test_e2e.py

echo ""
echo "==> Pronto. Para iniciar o servidor:"
echo "    python cathedral_orchestrator.py"
