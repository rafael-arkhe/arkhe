#!/bin/bash
# clone_high_priority.sh — Clonagem dos repositórios prioritários
# Catedral OS — Substrato 210 (Registro de Pacotes e Repositórios)

set -e

ROOT="$(cd "$(dirname "$0")" && pwd)"
DEPS="$ROOT/../catedral_deps"

mkdir -p "$DEPS"
cd "$DEPS"

clone() {
    local name="$1"
    local url="$2"
    if [ -d "$name" ]; then
        echo "✅ $name já existe."
    else
        echo "📦 Clonando $name..."
        if git clone --depth 1 "$url" "$name"; then
            echo "   ✅ $name concluído."
        else
            echo "   ⚠️  Falha ao clonar $name (repositório pode não existir)."
        fi
    fi
}

echo ""
echo "═════════════════════════════════════════════════════════════════"
echo "  📦 CATEDRAL OS — CLONAGEM DE ALTA PRIORIDADE (SUBSTRATO 210)"
echo "═════════════════════════════════════════════════════════════════"

# 1. Núcleo Prolog
clone swipl-devel          "https://github.com/SWI-Prolog/swipl-devel.git"
clone prolog-mcp           "https://github.com/umuro/prolog-mcp.git"

# 2. Robótica
clone knowrob              "https://github.com/knowrob/knowrob.git"
clone rosprolog            "https://github.com/knowrob/rosprolog.git"

# 3. Nanofotônica
clone Inverse-metasurface-design-CGAN "https://github.com/metaphotonics/Inverse-metasurface-design-CGAN.git"
clone autophotonicdesign   "https://github.com/flexcompute/autophotonicdesign.git"

# 4. Redes 6G
clone 6GDetCom_MKFirm      "https://github.com/ustutt-ipvs-vs/6GDetCom_MKFirm.git"

# 5. QKD
clone qosst                "https://github.com/QOSST/qosst.git"

# 6. Verificação Formal
clone SymbiYosys           "https://github.com/YosysHQ/SymbiYosys.git"
clone yosys                "https://github.com/YosysHQ/yosys.git"
clone lean4                "https://github.com/leanprover/lean4.git"

echo ""
echo "✅ Clonagem de alta prioridade concluída."
echo "   Repositórios em: $DEPS"
