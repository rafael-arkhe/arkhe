#!/usr/bin/env bash
# Catedral OS v304.0 — Deploy Script
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

echo "=== Catedral OS v304.0 — Deploy ==="
echo "Project: $PROJECT_DIR"

# Build Rust binaries
echo "[1/4] Building Rust binaries..."
cd "$PROJECT_DIR/rust"
cargo build --release 2>&1 | tail -5

# Install Python dependencies
echo "[2/4] Installing Python dependencies..."
cd "$PROJECT_DIR/python"
pip install -r requirements.txt 2>&1 | tail -3

# Copy binaries to install dir
echo "[3/4] Installing binaries..."
sudo mkdir -p /opt/catedral-os/bin
sudo cp "$PROJECT_DIR/rust/target/release/catedral-raft" /opt/catedral-os/bin/
sudo chmod +x /opt/catedral-os/bin/catedral-raft

# Deploy systemd units
echo "[4/4] Deploying systemd units..."
sudo cp "$PROJECT_DIR/systemd/"*.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl restart catedral-orquestrador catedral-decisor catedral-zeno

echo "=== Deploy complete ==="
echo "Status: systemctl status catedral-*"
