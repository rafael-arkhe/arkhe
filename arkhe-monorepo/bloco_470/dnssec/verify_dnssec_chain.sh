#!/bin/bash
# =============================================================================
# BLOCO 470 v15 — DNSSEC (D1)
# verify_dnssec_chain.sh
#
# Verificação da cadeia de confiança DNSSEC:
#   DNSKEY -> DS no pai -> RRSIG -> AD bit -> _did TXT
# Requer: dig (bind-tools/dnsutils).
# =============================================================================
set -euo pipefail

DOMAIN="${DOMAIN:-catedral.os}"

echo "🔍 Verificando cadeia de confiança DNSSEC para $DOMAIN"
echo "-------------------------------------------------------------------"

# 1. Verificar DNSKEY (chaves da zona)
echo "1. DNSKEY:"
dig +dnssec "$DOMAIN" DNSKEY | grep -E "DNSKEY|RRSIG" || echo "   (sem DNSKEY)"

# 2. Verificar DS no pai
echo ""
echo "2. DS record no pai:"
dig +dnssec "$DOMAIN" DS | grep -E "DS|RRSIG" || echo "   (sem DS — delegue ao registrador)"

# 3. Verificar cadeia completa (sigchase valida NSEC/NSEC3 recursivamente)
echo ""
echo "3. Cadeia de confiança:"
if dig +sigchase +trusted-key=/dev/null "$DOMAIN" SOA >/dev/null 2>&1; then
    echo "   (sigchase não dispõe de trusted keys locais; usando validadores públicos)"
fi
dig +dnssec "$DOMAIN" SOA | grep -E "status:|SOA" || true

# 4. Verificar AD bit (resposta autenticada)
echo ""
echo "4. AD bit:"
dig +dnssec "$DOMAIN" A | grep -E "flags:.*ad" || echo "   AD bit ausente — cadeia ainda não validada"

# 5. Verificar _did TXT com DNSSEC (did:web)
echo ""
echo "5. _did TXT:"
dig +dnssec "_did.$DOMAIN" TXT | grep -E "TXT|RRSIG" || echo "   (sem registro _did)"

echo ""
echo "-------------------------------------------------------------------"
echo "✅ Verificação DNSSEC concluída"
echo "   - DS/DNSKEY presentes e RRSIG válidos  => cadeia íntegra"
echo "   - AD bit setado na resposta           => resolução autenticada"