#!/usr/bin/env bash
set -euo pipefail

# Verifica se o sistema operacional é Linux
if [[ "$(uname -s)" != "Linux" ]]; then
    echo "ERROR: This is a systemd/Linux-only deployment (systemd Type=notify)."
    echo "Current OS: $(uname -s)"
    exit 1
fi

# Verifica Python
command -v python3 >/dev/null 2>&1 || { echo "ERROR: python3 not found"; exit 1; }

# Verifica Rust toolchain
command -v cargo >/dev/null 2>&1 || { echo "ERROR: cargo (Rust) not found"; exit 1; }

echo "=== Pre-flight checks OK ==="