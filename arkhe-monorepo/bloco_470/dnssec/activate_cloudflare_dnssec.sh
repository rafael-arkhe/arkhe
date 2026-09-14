#!/bin/bash
# =============================================================================
# BLOCO 470 v15 — DNSSEC (D1)
# activate_cloudflare_dnssec.sh
#
# Ativação de DNSSEC no Cloudflare e extração dos DS records para o registrador.
# Requer: CLOUDFLARE_API_KEY, CLOUDFLARE_EMAIL e CLI `jq`.
# =============================================================================
set -euo pipefail

DOMAIN="${DOMAIN:-catedral.os}"
CLOUDFLARE_API_KEY="${CLOUDFLARE_API_KEY:?CLOUDFLARE_API_KEY é obrigatória}"
CLOUDFLARE_EMAIL="${CLOUDFLARE_EMAIL:?CLOUDFLARE_EMAIL é obrigatório}"

echo "🔍 Localizando zona $DOMAIN no Cloudflare..."

ZONE_ID=$(curl -s -X GET "https://api.cloudflare.com/client/v4/zones?name=$DOMAIN" \
    -H "X-Auth-Email: $CLOUDFLARE_EMAIL" \
    -H "X-Auth-Key: $CLOUDFLARE_API_KEY" \
    -H "Content-Type: application/json" | jq -r '.result[0].id')

if [ -z "$ZONE_ID" ] || [ "$ZONE_ID" = "null" ]; then
    echo "❌ Zona não encontrada para $DOMAIN" >&2
    exit 1
fi
echo "✅ Zona encontrada: $ZONE_ID"

# 1. Ativar DNSSEC
echo "🛡️ Ativando DNSSEC para $DOMAIN..."
curl -s -X PATCH "https://api.cloudflare.com/client/v4/zones/$ZONE_ID/dnssec" \
    -H "X-Auth-Email: $CLOUDFLARE_EMAIL" \
    -H "X-Auth-Key: $CLOUDFLARE_API_KEY" \
    -H "Content-Type: application/json" \
    --data '{"status": "active"}' | jq .

# 2. Obter os DS records
echo "📜 Obtendo DS records..."
DS_RECORDS=$(curl -s -X GET "https://api.cloudflare.com/client/v4/zones/$ZONE_ID/dnssec" \
    -H "X-Auth-Email: $CLOUDFLARE_EMAIL" \
    -H "X-Auth-Key: $CLOUDFLARE_API_KEY" \
    -H "Content-Type: application/json" | jq '.result')

# 3. Exibir DS records e cadeia de confiança
echo ""
echo "✅ DNSSEC ativado para $DOMAIN"
echo ""
echo "DS Records a serem adicionados ao registrador:"
echo "-------------------------------------------------------------------"
echo "$DS_RECORDS" | jq .
echo "-------------------------------------------------------------------"

# 4. Persistir DS records para o passo verify_dnssec_chain.sh
DS_FILE="$(dirname "$0")/ds_records_cloudflare.json"
echo "$DS_RECORDS" > "$DS_FILE"
echo "📄 DS records persistidos em: $DS_FILE"