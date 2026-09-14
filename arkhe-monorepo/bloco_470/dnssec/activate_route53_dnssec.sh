#!/bin/bash
# =============================================================================
# BLOCO 470 v15 — DNSSEC (D1)
# activate_route53_dnssec.sh
#
# Ativação de DNSSEC no AWS Route 53 (KMS + KSK + DS records).
# Requer: AWS CLI autenticado com permissões para KMS e Route 53.
# =============================================================================
set -euo pipefail

DOMAIN="${DOMAIN:-catedral.os}"
AWS_REGION="${AWS_REGION:-us-east-1}"

echo "🔍 Localizando hosted zone $DOMAIN no Route 53..."

HOSTED_ZONE_ID=$(aws route53 list-hosted-zones \
    --query "HostedZones[?Name=='$DOMAIN.'].Id" --output text | cut -d'/' -f3)

if [ -z "$HOSTED_ZONE_ID" ] || [ "$HOSTED_ZONE_ID" = "None" ]; then
    echo "❌ Hosted zone não encontrada para $DOMAIN" >&2
    exit 1
fi
echo "✅ Hosted zone: $HOSTED_ZONE_ID"

# 1. Criar KMS Key para assinatura DNSSEC (ECC_NIST_P256, uso SIGN_VERIFY)
echo "🔑 Criando KMS key para DNSSEC..."
KMS_KEY_ID=$(aws kms create-key \
    --description "DNSSEC key for $DOMAIN" \
    --key-usage SIGN_VERIFY \
    --customer-master-key-spec ECC_NIST_P256 \
    --region "$AWS_REGION" \
    --query 'KeyMetadata.KeyId' \
    --output text)

echo "✅ KMS key: $KMS_KEY_ID"

# 2. Habilitar política de assinatura na zona (must be done before KSK)
echo "🛡️ Ativando DNSSEC signing na zona..."
aws route53 enable-hosted-zone-dnssec \
    --hosted-zone-id "$HOSTED_ZONE_ID"

# Enabling DNSSEC na região da zona
ZONE_REGION=$(aws route53 get-hosted-zone --id "$HOSTED_ZONE_ID" \
    --query 'DelegationSet' --output text >/dev/null 2>&1 && echo "$AWS_REGION")
aws route53 change-resource-record-sets --hosted-zone-id "$HOSTED_ZONE_ID" >/dev/null 2>&1 || true

# 3. Criar chave de assinatura (KSK)
echo "🗝️ Criando Key Signing Key..."
aws route53 create-key-signing-key \
    --hosted-zone-id "$HOSTED_ZONE_ID" \
    --key-signing-key-name "KSK-$DOMAIN" \
    --kms-key-id "$KMS_KEY_ID" \
    --caller-reference "ksk-$DOMAIN-$(date +%s)"

# 4. Obter DS records para o registrador
echo "📜 Obtendo DS records..."
if command -v jq >/dev/null 2>&1; then
    aws route53 get-dnssec --hosted-zone-id "$HOSTED_ZONE_ID" | jq .
else
    aws route53 get-dnssec --hosted-zone-id "$HOSTED_ZONE_ID"
fi

echo ""
echo "✅ DNSSEC ativado para $DOMAIN no Route 53"
echo ""
echo "⚠️  Registrar os DS records acima na autoridade do TLD antes de ativar"
echo "    o DNSSEC na zona (consulte verify_dnssec_chain.sh)."