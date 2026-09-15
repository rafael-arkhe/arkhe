# arkhe.gguf — Model Card

## Identificação

| Campo | Valor |
|:---|:---|
| Nome | arkhe.gguf |
| Versão | 0.1.0 |
| Data | <ISO 8601> |
| Modelo base | <Llama 3.2 1B / Qwen 2.5 1.5B / SmolLM2 1.7B> |
| Tipo | <base / instruction-tuned> |
| Quantização | <Q4_K_M / Q5_K_M / Q8_0> |
| Tamanho | <bytes> |
| SHA-256 | <hex> |
| BLAKE3 | <hex> |

## Proveniência

| Campo | Valor |
|:---|:---|
| Pipeline | safetensors → GGUF → sign → extend → anchor |
| Conversão | llama.cpp/convert_hf_to_gguf.py |
| Assinatura | OpenSSF Model Signing v1.0 |
| Log | Rekor (log_id: <id>, index: <index>) |
| Trust root | Sigstore/Fulcio |

## Capabilities Declaradas

| Capability | Status | Evidência |
|:---|:---|:---|
| (nenhuma declarada) | — | — |

## Non-Capabilities Declaradas

| Non-capability | Status |
|:---|:---|
| code-execution | not_implemented |
| network-access | not_implemented |
| file-write | not_implemented |
| shell-execution | not_implemented |

## Disclaimer

Capabilities são **declarações do signatário**, não verificações. O Arkhe OS
verifica a integridade do artefato (hash, assinatura, inclusão, quórum) — não
o comportamento do modelo.
