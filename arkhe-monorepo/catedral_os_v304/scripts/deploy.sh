#!/usr/bin/env bash
set -euo pipefail

CATEDRAL_VERSION="${1:-v304.0}"
INVENTORY="${2:-ansible/inventory.ini}"
PLAYBOOK="${3:-ansible/playbook.yml}"

echo "=== Catedral OS ${CATEDRAL_VERSION} Deployment ==="
echo "Inventory: ${INVENTORY}"
echo "Playbook: ${PLAYBOOK}"

# Pre-flight checks
echo "[1/5] Pre-flight checks..."
command -v ansible-playbook >/dev/null 2>&1 || { echo "ERROR: ansible-playbook not found"; exit 1; }

# Syntax check
echo "[2/5] Syntax check..."
ansible-playbook --syntax-check -i "${INVENTORY}" "${PLAYBOOK}"

# Dry run
echo "[3/5] Dry run..."
ansible-playbook --check -i "${INVENTORY}" "${PLAYBOOK}" || echo "Dry run had issues (expected for first deploy)"

# Deploy
echo "[4/5] Deploying..."
ansible-playbook -i "${INVENTORY}" "${PLAYBOOK}"

# Verify
echo "[5/5] Verification..."
for host in node-alfa node-beta node-gama; do
    ansible "${host}" -i "${INVENTORY}" -m shell -a "systemctl is-active catedral-orquestrador.service" 2>/dev/null && \
        echo "  ${host}: orchestrator running" || \
        echo "  ${host}: orchestrator NOT running"
done

echo "=== Deployment complete ==="
