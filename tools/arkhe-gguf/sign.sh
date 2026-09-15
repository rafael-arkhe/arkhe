#!/usr/bin/env bash
# sign.sh — Assina modelo com OpenSSF Model Signing v1.0 (keyless)
set -euo pipefail

GGUF_PATH="${1:?uso: sign.sh <model.gguf>}"

pip install model-signing

# Assinatura keyless (OIDC + Fulcio + Rekor)
model_signing sign "$GGUF_PATH"

echo "Bundle:    $GGUF_PATH.sigstore"

# Verificação (roundtrip)
model_signing verify "$GGUF_PATH" \
    --signature "$GGUF_PATH.sig" \
    --identity "$(git config user.email)" \
    --identity-provider "https://token.actions.githubusercontent.com"

echo "✅ Assinatura verificada"
