#!/usr/bin/env bash
set -euo pipefail

echo "════════════════════════════════════════════════════════════"
echo "  ARKHE-SPECTRAL v4.2.1 — Execução completa"
echo "════════════════════════════════════════════════════════════"

cd "$(dirname "$0")"

echo ""
echo "▶ [1/2] Rust: cargo test --release"
echo "────────────────────────────────────────────────────────────"
cargo test --release --workspace -- --nocapture

echo ""
echo "▶ [2/2] Python: zeitgeist v4.2.1"
echo "────────────────────────────────────────────────────────────"
PYTHON_CMD=""
for py in python3 python; do
    if command -v "$py" >/dev/null 2>&1 && "$py" -c "import scipy" 2>/dev/null; then
        PYTHON_CMD="$py"
        break
    fi
done
if [ -z "$PYTHON_CMD" ]; then
    echo "ERRO: nenhum python3/python com scipy disponível" >&2
    exit 1
fi
echo "Usando: $PYTHON_CMD ($(command -v "$PYTHON_CMD"))"
"$PYTHON_CMD" python/zeitgeist_v4_2_1.py

echo ""
echo "════════════════════════════════════════════════════════════"
echo "  ✅ Execução completa"
echo "════════════════════════════════════════════════════════════"