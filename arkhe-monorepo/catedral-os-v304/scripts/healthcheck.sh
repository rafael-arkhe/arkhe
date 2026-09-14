#!/usr/bin/env bash
# Catedral OS v304.0 — Healthcheck
set -euo pipefail

PHI_LOWER=0.577350
PHI_UPPER=0.999900
ERRORS=0

echo "=== Catedral OS v304.0 — Healthcheck ==="

# Check systemd services
for svc in catedral-orquestrador catedral-decisor catedral-zeno; do
    if systemctl is-active --quiet "$svc"; then
        echo "[OK] $svc is active"
    else
        echo "[FAIL] $svc is not active"
        ERRORS=$((ERRORS + 1))
    fi
done

# Check metrics endpoint
if curl -sf http://localhost:9090/metrics > /dev/null 2>&1; then
    echo "[OK] Metrics endpoint responding"
else
    echo "[WARN] Metrics endpoint not reachable"
fi

# Check phi value from metrics
PHI_RAW=$(curl -sf http://localhost:9090/metrics 2>/dev/null | grep '^catedral_phi ' | awk '{print $2}' || echo "0")
if [ -n "$PHI_RAW" ] && [ "$PHI_RAW" != "0" ]; then
    PHI_OK=$(echo "$PHI_RAW $PHI_LOWER $PHI_UPPER" | awk '{if ($1 > $2 && $1 < $3) print "yes"; else print "no"}')
    if [ "$PHI_OK" = "yes" ]; then
        echo "[OK] Phi=$PHI_RAW within bounds ($PHI_LOWER, $PHI_UPPER)"
    else
        echo "[FAIL] Phi=$PHI_RAW of bounds"
        ERRORS=$((ERRORS + 1))
    fi
else
    echo "[WARN] Could not read phi from metrics"
fi

# Check data directory
if [ -d /var/lib/catedral ]; then
    echo "[OK] Data directory exists"
else
    echo "[WARN] Data directory missing"
fi

# Check log directory
if [ -d /var/log/catedral ]; then
    echo "[OK] Log directory exists"
else
    echo "[WARN] Log directory missing"
fi

echo "=== Healthcheck complete (errors: $ERRORS) ==="
exit $ERRORS
