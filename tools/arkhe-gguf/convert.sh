#!/usr/bin/env bash
# convert.sh — Converte modelo HF para GGUF
set -euo pipefail

MODEL_DIR="${1:?uso: convert.sh <model_dir> [outfile] [outtype]}"
OUTFILE="${2:-arkhe-base.gguf}"
OUTTYPE="${3:-q4_k_m}"

[[ -d "$MODEL_DIR" ]] || { echo "ERRO: dir inexistente: $MODEL_DIR"; exit 1; }
command -v python3 >/dev/null || { echo "python3 em falta"; exit 1; }

[[ -f "llama.cpp/convert_hf_to_gguf.py" ]] || {
    echo "ERRO: llama.cpp ausente. Clone:"
    echo "  git clone https://github.com/ggerganov/llama.cpp"
    exit 1
}

pip install -r llama.cpp/requirements.txt

python3 llama.cpp/convert_hf_to_gguf.py \
    --outfile "$OUTFILE" \
    --outtype "$OUTTYPE" \
    "$MODEL_DIR"

echo "GGUF base: $OUTFILE"
sha256sum "$OUTFILE"
