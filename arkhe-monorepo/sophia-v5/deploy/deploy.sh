#!/usr/bin/env bash
# SOPHIA V5.0 — deploy auto-hospedado (homelab)
# Uso: sudo ./deploy.sh --all --environment=homelab [--verbose]
set -euo pipefail

ENV="homelab"
VERBOSE=0
ALL=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --all) ALL=1 ;;
    --environment=*) ENV="${1#*=}" ;;
    --verbose) VERBOSE=1 ;;
    --help) echo "uso: $0 [--all] [--environment=homelab] [--verbose]"; exit 0 ;;
    *) echo "opção desconhecida: $1"; exit 1 ;;
  esac
  shift
done

log() { [[ $VERBOSE -eq 1 ]] && echo "[deploy] $*"; }

if [[ "$ENV" != "homelab" ]]; then
  echo "ERROR: apenas environment=homelab é suportado (self-hosted)."
  exit 1
fi

BASE="$(cd "$(dirname "$0")/.." && pwd)"
DEST="/opt/sophia"
BIN="$DEST/bin"
PY="$DEST/agents/python"

log "País base: $BASE"

# 1. Preparar layout local
install -d -m 0755 "$DEST" "$BIN" "$DEST/dashboard" "$DEST/config" \
   "$DEST/edge_vision" "$DEST/security"

# 2. Copiar fontes (auditável: mesmo workspace vira a fonte da verdade)
cp -r "$BASE/agents/python" "$PY"
cp -r "$BASE/dashboard"/* "$DEST/dashboard/"
cp "$BASE/config/sophia.yaml" "$DEST/config/sophia.yaml"
cp -r "$BASE/security" "$DEST/security"
cp -r "$BASE"/*.md "$DEST/" 2>/dev/null || true

# 3. Compilar agentes C++ (requer cmake/g++; falha sem eles)
if command -v cmake >/dev/null && command -v make >/dev/null; then
  log "Compilando agentes C++..."
  build_dir="$(mktemp -d)"
  cmake -S "$BASE/agents/cpp" -B "$build_dir" -DCMAKE_BUILD_TYPE=Release -DSOPHIA_USE_OPENMP=ON
  make -C "$build_dir" -j"$(nproc)"
  install -m 0755 "$build_dir/sophia_orchestrator" "$BIN/sophia_orchestrator"
  install -m 0755 "$build_dir/sophia_vision" "$BIN/sophia_vision"
  rm -rf "$build_dir"
else
  log "cmake/make ausentes — binários C++ não compilados (modo Python-only)."
fi

# 4. Instalar serviços systemd
for unit in sophia-orchestrator sophia-edge-vision sophia-dashboard sophia-security; do
  install -m 0644 "$BASE/deploy/systemd/$unit.service" "/etc/systemd/system/"
done
systemctl daemon-reload

# 5. Criar caminhos de runtime
install -d -m 0750 /var/lib/sophia /etc/sophia
touch /etc/sophia/secrets.conf

# 6. Aplicar hardening
python3 "$PY/sophia_local/security/hardening.py" --apply

echo "✅ SOPHIA V5.0 instalado em $DEST"
echo "Inicie com: systemctl enable --now sophia-orchestrator sophia-dashboard sophia-security"
if [[ $ALL -eq 1 ]]; then
  systemctl enable --now sophia-orchestrator sophia-dashboard sophia-security
  echo "✅ Serviços ativos."
fi