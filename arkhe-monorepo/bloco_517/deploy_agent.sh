#!/usr/bin/env bash
# deploy_agent.sh — v77 VETADO — BLOCO 517 (G1)
# Registro e deploy do agente no watsonx Orchestrate.
#
# VETAGEM: o CLI `orchestrate` nao existe neste repositorio; o script e o
# contrato de integracao, com guardas de credencial. --dry-run lista os
# comandos sem executar. Selftest possivel apenas fora do repo (WXO).

set -euo pipefail

DRY_RUN="${DRY_RUN:-0}"
: "${WXO_URL:?variavel WXO_URL obrigatoria (ex.: https://eu-gb.ml.cloud.ibm.com)}"
: "${WXO_APIKEY:?variavel WXO_APIKEY obrigatoria}"

run_or_print() {
  if [ "$DRY_RUN" = "1" ]; then
    printf '  [dry-run] %s\n' "$*"
  else
    "$@"
  fi
}

export ORCHESTRATE_URL="$WXO_URL"
export ORCHESTRATE_APIKEY="$WXO_APIKEY"

run_or_print orchestrate env add --name production --url "$WXO_URL"
run_or_print orchestrate env activate production

for tool in tools/rpc_tools.py tools/trace_tools.py tools/simulation_tools.py tools/resend_tools.py; do
  run_or_print orchestrate tools import -k python -f "$tool"
done

run_or_print orchestrate agents import -f agent.yaml
run_or_print orchestrate agents deploy -n hyperevm_recovery_agent
run_or_print orchestrate agents list -v

echo "deploy_agent.sh: OK (dry_run=${DRY_RUN})"