#!/usr/bin/env bash
# Auditoria de namespace de invariantes (bloco 1057 ratificado) — versao CI.
# Substrato = Rust em packages/ (src/ e tests/) + provas Lean em src/lean/.
# Referencias em docs sem substrato e que NAO estao na allowlist de rejeitadas
# fazem o check falhar (portao honesto). IDs rejeitados por pareceres (v500,
# v510, v510.1, v511) sao permitidos como referencia documental, nunca atestados.
# Monorepo: usa packages/ (NAO existe crates/, invariants/ nem ledger/).
# Pre-requisito: ripgrep (rg) no PATH.

set -uo pipefail

# self-locating: roda a partir do diretorio do script, independente do cwd
# (WSL/Windows path-mangling: nunca confiar em `cd` relativo ao invocador).
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR/../.." || exit 1

# ripgrep e pre-requisito obrigatorio: sem rg a auditoria nao pode atestar nada.
if ! command -v rg >/dev/null 2>&1; then
    echo "ERRO: ripgrep (rg) nao instalado no PATH — instalado nas CI images (ubuntu: apt-get install ripgrep)."
    exit 2
fi

echo "Auditoria de namespace — Catedral OS (bloco 1057)"
echo "--------------------------------------------------"

# --- raizes reais do workspace (convencao do monorepo) ---
ROOTS_RUST=(packages)
ROOTS_LEAN=(src/lean)
ROOT_DOCS=(docs)

# --- 1. IDs atestados: Rust + Lean (substrato real, --no-filename p/ grep -qx) ---
RUST_IDS=$(rg -oP --no-filename 'I\d{3,4}' "${ROOTS_RUST[@]}" -g '*.rs' 2>/dev/null | sort -u || true)
LEAN_IDS=$(rg -oP --no-filename 'I\d{3,4}' "${ROOTS_LEAN[@]}" -g '*.lean' 2>/dev/null | sort -u || true)
ATTESTED=$(printf '%s\n%s\n' "$RUST_IDS" "$LEAN_IDS" | sort -u)
echo "IDs com substrato (Rust packages/ + Lean src/lean/):"
[ -z "$ATTESTED" ] && echo "  (nenhum)" || echo "$ATTESTED" | sed 's/^/  - /'

# --- 2. IDs rejeitados (allowlist honesta, pareceres v500/v510/v510.1/v511) ---
REJECTED="I461
I462
I627
I628
I629
I630
I632
I633
I635
I636
I641
I642
I643
I644
I645
I646"

# --- 3. IDs referidos em docs sem substrato e fora da allowlist => falha ---
DOC_REFS=$(rg -oP --no-filename 'I\d{3,4}' "${ROOT_DOCS[@]}" -g '*.md' 2>/dev/null | sort -u || true)
echo ""
echo "Referencias em docs sem substrato (fora da allowlist de rejeitadas):"
VIOLATED=0
for id in $DOC_REFS; do
    if ! echo "$ATTESTED" | grep -qx "$id"; then
        if echo "$REJECTED" | grep -qx "$id"; then
            echo "  - $id (referencia morta — rejeitada por parecer, nao atestada)"
        else
            echo "  - $id (SEM SUBSTRATO E SEM PARECER — violacao de namespace)"
            VIOLATED=1
        fi
    fi
done
[ "$VIOLATED" -eq 0 ] && echo "  (nenhuma) — docs so referenciam IDs atestados ou parecer-rejeitados"

# --- 4. Canonico obrigatorio: I624 (ARKHE Chaves) deve ter substrato ---
echo ""
echo "Canonico I624 (ARKHE Chaves, blocos 1052..1057):"
if echo "$ATTESTED" | grep -qx 'I624'; then
    echo "  - I624 OK (arkhe-blink-bridge, 35/35 testes; vinculo P2P arkhe-p2p)"
else
    echo "  - I624 AUSENTE — atestacao pendente"
    VIOLATED=1
fi

echo ""
echo "--------------------------------------------------"
if [ "$VIOLATED" -eq 0 ]; then
    echo "Auditoria CONCLUIDA OK. Nenhum ID novo sem substrato; I624 atestado."
    exit 0
else
    echo "Auditoria FALHOU: novo(s) ID(s) sem substrato nao podem ser atestados (regra: realizacao testada antes do ID)."
    exit 1
fi