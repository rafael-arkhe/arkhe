#!/usr/bin/env bash
set -euo pipefail

NODE_ID="${1:-1}"
METRICS_PORT="${2:-9090}"
API_BASE="http://localhost:${METRICS_PORT}"

echo "=== Health Check — Catedral OS v304.0 Node ${NODE_ID} ==="

# Check systemd services
echo "[1/4] Service status..."
for svc in catedral-orquestrador catedral-decisor catedral-zeno; do
    if systemctl is-active --quiet "${svc}.service" 2>/dev/null; then
        echo "  ${svc}: active"
    else
        echo "  ${svc}: INACTIVE"
        exit 1
    fi
done

# Check metrics endpoint
echo "[2/4] Metrics endpoint..."
if curl -sf "${API_BASE}/metrics" > /dev/null 2>&1; then
    echo "  Metrics: available"
else
    echo "  Metrics: UNAVAILABLE"
fi

# Check Raft storage
echo "[3/4] Raft storage..."
RAFT_DIR="/var/lib/catedral/raft/${NODE_ID}"
if [ -d "${RAFT_DIR}" ]; then
    DB_SIZE=$(du -sh "${RAFT_DIR}" 2>/dev/null | cut -f1)
    echo "  Raft DB: ${DB_SIZE}"
else
    echo "  Raft DB: directory not found"
fi

# Check phi bounds
echo "[4/4] Phi bounds..."
PHI=$(curl -sf "${API_BASE}/metrics" 2>/dev/null | grep "catedral_phi " | awk '{print $2}' || echo "N/A")
if [ "${PHI}" != "N/A" ] && [ -n "${PHI}" ]; then
    echo "  Phi: ${PHI}"
else
    echo "  Phi: reading from metrics"
fi

echo "=== Health check complete ==="
