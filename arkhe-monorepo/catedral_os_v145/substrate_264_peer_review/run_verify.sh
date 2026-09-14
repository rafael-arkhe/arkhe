#!/usr/bin/env bash
# Substrato 264 v2 — verificação
set -e
cd "$(dirname "$0")"

echo "==> Testes do port Python..."
python -m pytest tests -q

echo "==> Demo do fluxo editorial..."
python substrate_264_peer_review.py >/dev/null && echo "Demo OK"

echo ""
echo "==> Pronto. O peer_review_v2.pl requer swipl (não verificado aqui)."
