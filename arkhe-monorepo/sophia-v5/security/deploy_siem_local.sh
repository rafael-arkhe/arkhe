#!/usr/bin/env bash
# Instala e configura Wazuh SIEM local (bound to localhost apenas).
set -e

echo "[SIEM] Deploying Wazuh local..."
apt-get update
apt-get install -y wazuh-manager wazuh-indexer wazuh-dashboard

# Apenas interface local (nenhuma exposição externa)
sed -i 's/0.0.0.0/127.0.0.1/g' /etc/wazuh-indexer/opensearch.yml

# Regras específicas Sophia
cp /opt/sophia/security/wazuh_rules/sophia_rules.xml /var/ossec/etc/rules/

# Carrega novas regras sem bloquear o entrypoint constitucional
/var/ossec/bin/ossec-control restart

echo "[SIEM] Wazuh local em https://localhost:5601 (dashboard)"
echo "[SIEM] Regras Sophia instaladas em /var/ossec/etc/rules/."