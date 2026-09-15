# Arkhe OS

Infraestrutura de verificação para artefactos e agentes de IA — kernel de
contenção, atestação criptográfica, grafo causal e governança por invariantes.

O que este repositório faz é **verificar**: hashes, assinaturas, provas de
inclusão e quóruns de testemunhas. O produto é a evidência, e cada afirmação
aponta, quando solicitada, para o mecanismo que a verifica.

## Estado

- **2.174 ficheiros rastreados**, **151 `Cargo.toml`**
- **35 membros** no workspace canónico (`safe-core-monorepo`); `cargo check --workspace` em verde
- **Aplicação de ambiente de trabalho** (`arkhe-ui`, React + Tauri) com instaladores MSI e NSIS produzidos
- **Updater** publicado no hashtree, versão 0.2.3
- **Licença:** `MIT OR Apache-2.0` — textos em `LICENSE` e `LICENSE-APACHE`

## Estrutura

| Caminho | O que é |
|:--|:--|
| `safe-core-monorepo/` | workspace canónico — 35 membros |
| `arkhe-monorepo/` | história do projecto: blocos, catedrais, substratos |
| `arkhe-ui/` | interface (React) e casco Tauri |
| `docs/` | documentação de projecto |

## Verificação em quatro gates

1. **Hash** — SHA-256 / BLAKE3
2. **Assinatura** — Ed25519 / ML-DSA
3. **Inclusão** — árvore de Merkle (RFC 6962)
4. **Quórum** — testemunhas independentes

A política do projecto é falhar fechada: na dúvida, rejeitar.

## História

Este repositório reúne trabalho de vários períodos. O `README` anterior, sobre
o projecto Ω-TEMP, está preservado em [`README-omega-temp.md`](README-omega-temp.md).
