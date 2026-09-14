# Análise do Repositório — 2026-07-04

Análise factual, baseada apenas no que existe em disco. Sem selos, sem scores inflados.

## 1. Visão geral

O repositório tem cerca de 90 diretórios de nível superior e **986 arquivos soltos na raiz** (278 .py, 178 .md, 81 .rs, 36 PDFs, modelos .gguf, zips, imagens). Há inclusive uma cópia aninhada do próprio repositório (`sasc-v34.8-ω-__-real-implementation-engine/` dentro dele mesmo) e o llama.cpp vendorizado inteiro. Isso não é um projeto de software — é um arquivo de trabalho acumulado. Nenhuma ferramenta (cargo, pytest, CI) consegue operar de forma confiável sobre essa estrutura.

## 2. Git

O histórico tem **11 commits no total**, todos de um único autor, com o último em **2026-05-03** — dois meses atrás. Existem **2.640 mudanças não commitadas** no working tree, e o branch atual é `QC-0892` (não `master`). Na prática, o git não está rastreando o trabalho: quase tudo que foi produzido desde maio vive fora do controle de versão.

## 3. Rust — o que existe de verdade

O workspace Cargo real tem 6 members, e todos os Cargo.toml existem:

O `kernel` (arkhe-kernel) tem **921 linhas de Rust em 6 arquivos** (temporal_chain, model_loader, inference_loop, qip_engine, qart_engine, main). Sem `todo!()` ou `unimplemented!()` — é pequeno, mas parece coeso. Os bindings (cli, python, wasm) e o sagemaker-proxy têm 1 arquivo .rs cada. Em `crates/` existem apenas `safe-core-crypto` e `safe-core-policy`.

**Não foi possível rodar `cargo check`**: o sandbox de análise não tem Rust e a instalação foi bloqueada pela rede. Atenção: o workspace declara `edition = "2024"`, que exige Rust ≥ 1.85. Compilar localmente é o teste que falta.

## 4. Python — o que roda e o que não roda

O pytest coleta **1.508 testes**, mas **25 módulos de teste nem importam** (dependências ausentes, ex.: `polynomial_arkhe`). Numa amostra executada (test_arklib, test_agentfield_bridge, arkhe_os_integral_test): **32 passaram, 27 falharam, 1 erro** — aproximadamente metade quebrada, incluindo testes async mal configurados.

## 5. O que os documentos descrevem mas NÃO existe no repo

Isto é o ponto mais importante. Os documentos colados na conversa (selos "ARKHE-SDK-COMPLETE", "INTEGRACOES-PRIORITARIAS v1.0/v2.0") descrevem código que **não está em disco**:

- O "ARKHE SDK" Rust com 9 crates (arkhe-sdk-kernel, arkhe-sdk-boundary, arkhe-sdk-provenance, arkhe-sdk-mcp...) **não existe**. O diretório `arkhe-sdk/` real é um pacote **Python** com 14 arquivos.
- `crates/arkhe-isolation-kernel` com `i1_no_cross_session_memory.rs` (citado no memo de purge) **não existe**.
- O diretório `specs/tla/` com I9_AcyclicAuthority.tla e I3_CacheOwnership.tla **não existe**. Nenhuma validação TLC foi rodada.
- BoundaryG, ProvenanceEngine, Evidence Bus, MCP servers — existem apenas como texto em documentos, não como código.

Os selos "✅ Concluído" nesses documentos descrevem intenções, não entregas. O próprio memorando de verificação (score 58-72/100) já tinha identificado esse padrão — e depois foi ele mesmo contaminado por um "parecer executivo" de 88/100.

## 6. Estado real, em uma frase

Existe um kernel Rust pequeno (~1k linhas) possivelmente compilável, um conjunto de testes Python meio quebrado, e uma quantidade enorme de documentos gerados por IA descrevendo sistemas que nunca foram escritos.

## 7. Próximos passos recomendados (ordem estrita)

1. **Commitar ou descartar** as 2.640 mudanças pendentes. Decidir o que é código e o que é arquivo morto.
2. **Quarentena**: mover os 986 arquivos soltos da raiz para `archive/` (a pasta já existe). Manter na raiz apenas o workspace Cargo, `tests/`, `src/` e docs essenciais.
3. **Rodar `cargo check --workspace`** na sua máquina (Rust ≥ 1.85). Esse é o único "score" que importa agora: compila ou não compila.
4. **Consertar a coleta do pytest**: criar um `requirements.txt` real, fazer os 25 módulos importarem ou movê-los para quarentena, e chegar a uma suíte onde 100% dos testes coletados executam (mesmo que alguns falhem).
5. Só depois disso: decidir **um** módulo para evoluir (sugestão: `kernel/` com temporal_chain) e escrever testes Rust para ele.
6. Não escrever mais documentos de arquitetura até que os passos 3 e 4 estejam verdes.

## 8. Fatos verificados nesta análise

| Item | Valor |
|---|---|
| Commits no git | 11 (último: 2026-05-03) |
| Mudanças não commitadas | 2.640 |
| Arquivos soltos na raiz | 986 |
| Linhas de Rust no kernel | 921 (6 arquivos, 0 stubs) |
| Members do workspace Cargo | 6 (todos com Cargo.toml presente) |
| Testes Python coletados | 1.508 |
| Módulos de teste que não importam | 25 |
| Amostra executada | 32 pass / 27 fail / 1 erro |
| SDK Rust de 9 crates dos documentos | não existe em disco |
| specs/tla/ (I9, I3) | não existe em disco |

## Adendo — Correção da suíte de testes (mesmo dia)

A coleta do pytest foi consertada: **1.762 testes coletados, zero erros de coleta** (antes: 1.508 coletados com 25 módulos quebrados).

O que foi feito: criado o diretório `lib/` com 36 módulos (27 recuperados do archive por análise de imports dos testes, 5 recuperados de versões numeradas tipo `polynomial_arkhe_960.py`, 4 aliases); conftest.py atualizado para incluir `lib/` no sys.path; 5 testes que dependem de torch protegidos com `pytest.importorskip("torch")`; 11 testes movidos para `tests/quarantine/` porque importam módulos que não existem em lugar nenhum do repo (bindu, tanmatra, clarity_gate, arkhe_global, etc. — ver tests/quarantine/README.md); corrigidos erros de sintaxe reais em 6 módulos (f-strings com aspas aninhadas incompatíveis com Python < 3.12, strings com quebra de linha literal, BOM); test_post_cathedral_substrates.py estava truncado no meio de uma linha e foi fechado minimamente; pytest, aiohttp e scipy adicionados ao requirements.txt.

Execução completa (em blocos, timeout de 5s por teste): aproximadamente **1.344 passaram, 171 falharam, ~102 erros de setup, 5 pulados**. Ou seja: a suíte agora coleta e roda, com ~76% de aprovação. Os erros de setup concentram-se em testes com dependência de ordem de import (ex.: test_orcid_onchain espera `substrate_251` no sys.path via efeito colateral de outro teste) — isso é a próxima dívida a atacar.

Avisos: os arquivos `__pycache__/*.pyc` antigos dentro do repo não puderam ser removidos pelo sandbox e podem conter bytecode obsoleto — rode com `PYTHONPYCACHEPREFIX` apontando para fora do repo ou apague-os manualmente. Um teste (`test_cross_substrate.py::test_substrate_570_importable`) trava indefinidamente sem timeout — use `pytest-timeout`.

## Adendo 2 — cargo check --workspace (mesmo dia)

Executado com rustc/cargo 1.91.1 (pacotes Ubuntu extraídos localmente, já que rustup está bloqueado no sandbox), em cópia do workspace em /tmp, resolvendo dependências frescas do crates.io (sem Cargo.lock). Edition 2024 real, sem adaptações no código.

**Resultado: 6 de 7 crates passam. O `arkhe-kernel` falha com 20 erros.**

| Crate | Resultado |
|---|---|
| arkhe-safe-core-sdk (raiz) | ✅ passa |
| arkhe-cli | ✅ passa |
| arkhe-cmd (kernel/cmd/arkhe) | ✅ passa |
| arkhe-python-bindings | ✅ passa |
| arkhe-wasm-bindings | ✅ passa |
| arkhe-sagemaker-proxy | ✅ passa |
| **arkhe-kernel** | ❌ **20 erros** |

Erros do arkhe-kernel, agrupados:

1. **Módulos fantasma** (main.rs:27-28): `mod orbital_mesh;` e `mod watchdog;` declarados, mas os arquivos nunca foram escritos — não existem nem no repo nem no archive. Mesmo padrão dos testes Python quarentenados.
2. **no_std num binário comum** (main.rs:15-16): `#![feature]` exige nightly (E0554); `Vec` não existe sem `extern crate alloc` (6 erros E0412/E0433); "unwinding panics are not supported without std" exige `panic = "abort"`. O main.rs foi escrito como kernel bare-metal, mas o Cargo.toml o trata como binário normal.
3. **Dependência não declarada**: `use sha3` em 3 arquivos, mas `sha3` não está no `[dependencies]` do kernel/Cargo.toml.
4. **Erros de tipo reais**: `AtomicU64: Clone` (temporal_chain.rs:25,28), `Copy` inválido em `GradientEntry` (qip_engine.rs:33), move de referência compartilhada (qart_engine.rs:67), tipo incompatível (qart_engine.rs:47), atributo unsafe sem `unsafe` (main.rs:87 — exigência da edition 2024).

Correção estimada: os itens 1 e 3 são triviais (criar stubs dos módulos ou remover as declarações; adicionar sha3 ao Cargo.toml). O item 2 é uma decisão de design: ou o kernel é no_std de verdade (precisa de target e panic handler adequados) ou vira um binário std normal (remover os atributos). O item 4 é trabalho de correção normal, bem localizado.

Observação: o `[[bench]] phi_c_compute` declarado no Cargo.toml da raiz aponta para `benches/phi_c_compute.rs`, que existe — ok. O Cargo.lock atual do repo não foi validado (a verificação regenerou o lock); rode `cargo check --locked` na sua máquina para validá-lo.
