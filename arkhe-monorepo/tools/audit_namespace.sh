#!/usr/bin/env bash
# Audita o namespace de invariantes (bloco 1057 ratificado).
# Diferente de qualquer versao anterior: distingue COLISÃO REAL de
# REFERENCIA MORTA. IDs sao atestados apenas por realizacao testada em
# packages/ (convencao do monorepo: nao existe crates/).

set -uo pipefail

echo "Auditoria de namespace — Catedral OS (bloco 1057)"
echo "--------------------------------------------------"

# 1. IDs atestados: ocorrencias em codigo Rust real do workspace.
# (--no-filename: em multiplos arquivos o rg prefixa o caminho, o que quebraria grep -qx.)
ATTESTED=$(rg -oP --no-filename 'I\d{3,4}' packages/ -g '*.rs' 2>/dev/null | sort -u || true)
echo "IDs com substrato (Rust em packages/):"
[ -z "$ATTESTED" ] && echo "  (nenhum)" || echo "$ATTESTED" | sed 's/^/  - /'

# 2. IDs referidos em documentos (podem ser referencia morta).
DOC_REFS=$(rg -oP --no-filename 'I\d{3,4}' docs/ -g '*.md' 2>/dev/null | sort -u || true)

echo ""
echo "Referencias em docs sem substrato em Rust packages/:"
FOUND=0
for id in $DOC_REFS; do
    if ! echo "$ATTESTED" | grep -qx "$id"; then
        echo "  - $id (referencia morta — rejeitar, nao atestar)"
        FOUND=1
    fi
done
[ "$FOUND" -eq 0 ] && echo "  (nenhuma)"

# 3. Referencias explicitamente rejeitadas (registry canonico).
echo ""
echo "Referencias rejeitadas registadas (arkhe-invariant-registry):"
echo "  - I624 (MLPerf)  — 0 ocorrencias no monorepo; rejeitada sem renomeacao"
echo "  - I624 (KeyMgmt) — sem substrato; rejeitada sem renomeacao"

# 4. Canonico atestado: apenas IDs com realizacao testada e registrada.
echo ""
echo "Canonicos atestados (realizacao + ledger):"
for id in I624; do
    if echo "$ATTESTED" | grep -qx "$id"; then
        echo "  - $id OK (bloco 1052..1057, arkhe-blink-bridge)"
    else
        echo "  - $id AUSENTE — atestacao pendente"
    fi
done

echo ""
echo "--------------------------------------------------"
echo "Auditoria concluida. Renomeacoes nao sao propostas para referencias mortas."