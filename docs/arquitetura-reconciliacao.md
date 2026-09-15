# Reconciliação entre a arquitetura declarada e a árvore real do repositório

**Tipo de artefato:** medição. Não é design, não é plano, não é proposta.
**Data da medição:** 2026-09-15 · **branch:** `QC-0892` · **HEAD:** `b0a3940c`
**Objeto medido:** `ARKHE-Documento-Mestre.md` §5 ("O monorepo: árvore completa") e §6 ("Índice dos 12 crates")
**Regra desta página:** zero afirmação sem `arquivo:linha` ou citação literal da `description` do `Cargo.toml` / cabeçalho de `lib.rs`. Onde não há evidência, está escrito que não há.

---

## 0. Método, definições e os números medidos

### 0.1 O que conta como "crate"

Adoto a definição verificável: **um diretório que contém um `Cargo.toml` rastreado pelo git**. Diretórios que são apenas contêineres (sem manifesto) são contados à parte e assinalados.

```bash
git ls-files '*Cargo.toml' | wc -l          # → 151
git ls-files '*Cargo.toml' | sed 's#/[^/]*$##' | sort -u | wc -l   # → 151
```

> `find` não é utilizável neste checkout (devolve 0 resultados para padrões que existem, sem erro). Todas as contagens usam `git ls-files`, que é a visão rastreada — e a única que sobrevive a um clone.

### 0.2 As três raízes e o que há fora delas

| Raiz | Entradas de diretório | Com `Cargo.toml` | Sem manifesto |
|-|-:|-:|-:|
| `crates/` (raiz do repo) | 3 | 2 | 1 (`crates/crates/`) |
| `crates/crates/` (aninhado) | 2 | 2 | 0 |
| `safe-core-monorepo/crates/` | 62 | 60 | 2 |
| `arkhe-monorepo/packages/` | 64 | 56 | 8 |
| **Total das três raízes** | **131** | **120** | **11** |

Fora dessas três raízes existem ainda **31 diretórios com `Cargo.toml` rastreado** (detalhados no Anexo A). Total repo-wide: **120 + 31 = 151**.

Os 11 diretórios sem manifesto, por raiz:

- `crates/crates/` — contêiner puro (os dois crates estão um nível abaixo)
- `safe-core-monorepo/crates/arkhe-buzz` — **zero arquivos rastreados**; `safe-core-monorepo/crates/arkhe-input-validation` — 1 arquivo rastreado (`src/lib.rs`), sem manifesto
- `arkhe-monorepo/packages/arkhe-grover`, `arkhe-photonics`, `arkhe-quantum-validator`, `arkhe-reveng`, `arkhe-sft-orchestrator`, `arkhe-sft`, `arkhe-smeasure`, `arkhe-web3-bounty` — **estes 8 não estão vazios**: contêm projetos Lean (`lakefile.toml`, `lean-toolchain`, `*.lean`) e/ou Python rastreados. São diretórios de *pacote* que não são crates Rust, não diretórios mortos.

Nenhum dos 11 aparece em `members` de nenhum workspace — logo não participam de `cargo check`.

### 0.3 Os 14 declarados: quais existem como caminho

```bash
for n in arkhe-core arkhe-spec arkhe-omni arkhe-pqc arkhe-p2p arkhe-zk arkhe-blockchain \
         arkhe-ledger arkhe-rsi arkhe-evm arkhe-hathor-notary arkhe-cognition \
         arkhe-governance arkhe-lean; do
  ls -d safe-core-monorepo/crates/$n arkhe-monorepo/packages/$n crates/$n 2>/dev/null
done
```

**5 nomes existem como diretório** (`arkhe-core`, `arkhe-pqc`, `arkhe-p2p`, `arkhe-rsi`, `arkhe-governance`), ocupando **6 diretórios** — porque `arkhe-core` existe em duas árvores.
**9 nomes não existem como caminho em lugar nenhum**.
**6 diretórios** das três raízes correspondem a nomes declarados → **131 − 6 = 125** fora do documento. É essa a origem do número 125.

Contagem de menções, com fronteira estrita (padrão `nome\([^a-z0-9_-]\|$\)`, para não inflar com `arkhe-lean-bridge`, `arkhe-spectral`, `arkhe-pqc-core`, `arkhe-rsi-core`, `arkhe-governance-bridge`):

| Nome | Arquivos que mencionam (estrito) | Fora do próprio Documento Mestre |
|-|-:|-:|
| `arkhe-core` | 97 | 96 |
| `arkhe-governance` | 29 | 28 |
| `arkhe-p2p` | 17 | 16 |
| `arkhe-rsi` | 13 | 12 |
| `arkhe-pqc` | 11 | 10 |
| `arkhe-ledger` | 3 | 2 |
| `arkhe-zk` | 2 | 1 |
| `arkhe-spec` | 1 | 0 |
| `arkhe-omni` | 1 | 0 |
| `arkhe-blockchain` | 1 | 0 |
| `arkhe-evm` | 1 | 0 |
| `arkhe-hathor-notary` | 1 | 0 |
| `arkhe-cognition` | 1 | 0 |
| `arkhe-lean` | 1 | 0 |

Os 9 inexistentes aparecem em **7 casos apenas no próprio Documento Mestre**. As duas exceções:

- `arkhe-zk` → `arkhe-monorepo/etherscan_verified_signatures_export.json` (uma string dentro de um dump de contratos verificados)
- `arkhe-ledger` → `arkhe-monorepo/docs/parecer_v510_substrato_fotonico.md`, `arkhe-monorepo/docs/parecer_v514_orquestracao_agentica.md` (pareceres que discutem o ledger em prosa)

### 0.4 Uma inconsistência interna do documento, medida antes de tudo

§6 tem o título **"Índice dos 12 crates"**, mas a tabela de §6 tem **13 linhas** e a de §5 lista **14 crates**. O documento não concorda consigo mesmo sobre quantos crates declara. Isto é relevante para a Parte 2: o número "12" de §6 não é um inventário medido, é um rótulo.

---

## 1. Tabela (a) — Onde cada um dos 14 declarados vive hoje

Veredictos possíveis: **EXISTE COM ESSE NOME** · **candidato: X (não confirmado)** · **NÃO REALIZADO**.

### 1.1 Os cinco nomes que existem

| # | Declarado (§) | Caminho real | Evidência | Veredicto |
|-:|-|-|-|-|
| 1 | `arkhe-core` (§7) | `safe-core-monorepo/crates/arkhe-core` **e** `arkhe-monorepo/packages/arkhe-core` | `safe-core-monorepo/crates/arkhe-core/Cargo.toml:6` → `description = "Foundation types, traits, and error handling for Arkhe OS"`; `arkhe-monorepo/packages/arkhe-core/Cargo.toml:5` → `description = "arkhe-core — pure transition (State/Event/apply) + append-only hash-chained ledger (Ghost-1)..."` | **EXISTE COM ESSE NOME — em dois lugares, com funções diferentes.** Nenhuma das duas faz o que §7 descreve (binário do pipeline de 10 passos). Ver §3.3. |
| 2 | `arkhe-pqc` (§10) | `arkhe-monorepo/packages/arkhe-pqc` | `Cargo.toml:5` → `description = "Quantum-safe cryptography layer: FIPS 203 ML-KEM + FIPS 204 ML-DSA in pure Rust"` | **EXISTE COM ESSE NOME.** O conteúdo declarado não bate: §5 diz `src/{lib,chk,ml_dsa,ml_kem}.rs`; o real é `src/{lib,kem,sign,handshake,auth_kem,wallet,tests}.rs`. **CHK não está aqui** — está em `safe-core-monorepo/crates/arkhe-crypto-pqc/src/chk.rs`. |
| 3 | `arkhe-p2p` (§11) | `arkhe-monorepo/packages/arkhe-p2p` | `Cargo.toml:6` → `description = "Camada P2P da Catedral (bloco 1055): identidade Ed25519, mensagens assinadas e rede libp2p 0.56.0 real (Kademlia + gossipsub + identify). Núcleo testável sem malha multi-nó."` | **EXISTE COM ESSE NOME, com stack diferente da declarada.** §11 declara "Nostr + WebRTC real"; as deps reais são `libp2p = { version = "=0.57.0", features = ["ed25519","noise","tcp","dns","tokio","yamux","kad","gossipsub","identify",...] }`. Não há Nostr nem WebRTC neste crate. (Nota: a `description` diz 0.56.0, a dep diz 0.57.0 — divergência interna do próprio crate.) |
| 4 | `arkhe-rsi` (§14) | `safe-core-monorepo/crates/arkhe-rsi` | `Cargo.toml:6` → `description = "FI-050 — self-limited RSI: quorum approval, hash-chain/score-drop verification, and automatic rollback gate a recursive self-improvement loop"`; `src/lib.rs:1` → `"Verificador de invariantes do loop RSI."` | **EXISTE COM ESSE NOME, com função diferente da declarada.** §14 diz "Otimizador de janela deslizante"; o real é um *verificador de invariantes* do loop RSI (13 arquivos em `src/`). Existe também `safe-core-monorepo/crates/arkhe-rsi-core` (tipos base do mesmo loop). |
| 5 | `arkhe-governance` (§18) | `safe-core-monorepo/crates/arkhe-governance` | `src/lib.rs:1` → `"Governance of the Arkhe OS."`; `Cargo.toml` sem `description` | **EXISTE COM ESSE NOME — e não é stub.** §6/§18 marcam `arkhe-governance` como 🟡 stub. O diretório real tem 7 módulos: `amendment.rs`, `audit_policy.rs`, `capability.rs`, `constitution.rs`, `gdpr.rs`, `lib.rs`, `paper_defaults.rs`. `src/amendment.rs:1` → `//! The amendment flow: **proposal → quorum → timelock**.` |

### 1.2 Os nove nomes que não existem — e o que foi verificado sobre cada candidato

| # | Declarado (§) | Veredicto | Candidato proposto → **verificado ou refutado** |
|-:|-|-|-|
| 6 | `arkhe-spec` (§8) | **NÃO REALIZADO** | **`arkhe-lean-bridge` REFUTADO** como "o `arkhe-spec`": é uma ponte FFI Lean↔Rust (`arkhe-monorepo/packages/arkhe-lean-bridge/Cargo.toml` → `"Ponte Lean↔Rust (v494.1 real): verificação centralizada no kernel Lean 4.33.1"`), não uma DSL com compilador. **`arkhe-spectral` REFUTADO**: `arkhe-monorepo/arkhe-spectral/crates/spectral-core/Cargo.toml` → `"Análise espectral de DAGs de tarefas: Laplaciano, Fiedler confiável/único, condicionamento e particionamento"` — análise espectral, não DSL. **Prova negativa:** zero arquivos `*.arkhe` rastreados (`git ls-files '*.arkhe'` → vazio) e zero arquivos `ast.rs`/`parser.rs`/`rust_gen.rs`/`lean_gen.rs`/`zk_gen.rs`. `specs/` (declarado em §5) **não existe**. <br>**O que faltaria para confirmar um candidato:** um crate que parseie uma gramática própria e emita código para ≥2 alvos. Há um único artefato na direção certa, e é uma *proc-macro*: `safe-core-monorepo/crates/arkhe-lean-spec-derive/src/lib.rs:7` → `#[proc_macro_derive(LeanSpec)]` — gera spec Lean a partir de structs Rust. Cobre **um** alvo, não três. |
| 7 | `arkhe-omni` (§9) | **NÃO REALIZADO** | **`arkhe-inference` — candidato plausível, NÃO confirmado.** `safe-core-monorepo/crates/arkhe-inference/Cargo.toml:5` → `description = "Unified inference engine with multiple backends"`; tem `candle-core`/`candle-nn`/`candle-transformers`/`mistralrs`/`llama-cpp-2` como **features opcionais** (`default = []`). <br>**Prova negativa:** §5/§9 declaram `src/{lib,compressed_data,aqua_kv,matryoshka,policy,verified_cache}.rs`. Nenhum desses arquivos existe (`git ls-files \| grep -i 'aqua_kv\|matryoshka\|paged_attention\|verified_cache\|compressed_data'` → vazio). `hf-hub` aparece em **exatamente um arquivo: o próprio Documento Mestre**. <br>**O que faltaria para confirmar:** qualquer arquivo de KV-cache, compressão ou registry de modelos no `arkhe-inference`. Não há. O candidato cobre "inferência" em geral; não cobre o que §9 descreve. |
| 8 | `arkhe-zk` (§12) | **NÃO REALIZADO** | **`agentkit-zkproof` REFUTADO:** zero arquivos com "agentkit" no repositório. **`arkhe-stark` REFUTADO como "o mesmo crate":** existe e é real (`arkhe-monorepo/packages/arkhe-stark/Cargo.toml` → `"STARKs pos-quanticos condicionais a hash function ... SHA-256 (Grover: 128 bits PQ) vs SHAKE256 (FIPS 202: 256 bits PQ)"`), mas é **STARK**, e §12 declara **Groth16 / arkworks BN254** — esquemas diferentes, sem sobreposição de código. <br>**O que existe de facto em Groth16:** `arkhe-monorepo/bloco_470/onchain/contracts/verifiers/Groth16Verifier.sol` — um verificador **Solidity**, não um crate Rust. `arkworks`/`bn254` aparecem em `bloco_470` (Solidity/Python/deploy) e no `etherscan_verified_signatures_export.json`, nunca em Rust. |
| 9 | `arkhe-blockchain` (§13) | **NÃO REALIZADO** | **`arkhe-block-registry` / `arkhe-timechain` — candidatos por domínio, NÃO confirmados.** Não existe crate com `ZkBlock` nem `ConsensusEngine` (`git grep -il 'ConsensusEngine\|ZkBlock'` devolve o Documento Mestre, um JSON de etherscan e dois arquivos não relacionados). <br>Substratos reais mais próximos, todos com mecanismo diferente do declarado: `arkhe-monorepo/packages/arkhe-timechain/Cargo.toml` → `"...a block becomes final when a majority of those echoes interfere constructively (consensus — 'loop closure'); balances live as flux-tube UTXOs..."` (finalidade por Chern–Simons, não ZK-proof de bloco); `arkhe-monorepo/catedral-os-v304/rust/raft/Cargo.toml` → `"Catedral OS v304.0 — Núcleo de consenso Raft com persistência e snapshots"` (Raft, fora das três raízes). <br>**O quórum 2/3 declarado existe, mas noutro sítio:** `arkhe-monorepo/packages/arkhe-field-stability/src/validators.rs:24` → `pub const VALIDATOR_QUORUM: f64 = 2.0 / 3.0;` e `src/validators.rs:14` → `"...aprovação de mais de 2/3 dos validadores"`. É um eixo de validadores semânticos, não um motor de consenso de blocos. |
| 10 | `arkhe-ledger` (§13) | **NÃO REALIZADO** | **`arkhe-block-registry` / `arkhe-timechain` — candidatos, NÃO confirmados.** §5/§13 declaram `arkhe-ledger` = "Sled + BlockStore + VoteStore + PaymentStore". Nenhum dos quatro nomes existe (`git grep -il 'BlockStore\|VoteStore\|PaymentStore'` → vazio). `sled` aparece apenas em `safe-core-monorepo/crates/arkhe-rsi/src/sled_backend.rs` (backend do RSI, não um ledger). <br>`arkhe-monorepo/packages/arkhe-block-registry/src/lib.rs:1` → `"arkhe-block-registry — Registro canonico de blocos (bloco 1075, v582.0-exec)"` com hash SHA3-256 em hex de 64 caracteres — é um *registro canónico de blocos*, sem store nem votos. <br>**O que faltaria para confirmar:** um crate com um store chave-valor persistente + votos + pagamentos. Não existe. |
| 11 | `arkhe-evm` (§16) | **NÃO REALIZADO** | **`arkhe-allo` REFUTADO** como correspondência de crate: `arkhe-allo/` é um projeto **Foundry/Solidity** (20 arquivos rastreados: `foundry.toml`, `src/AlloPool.sol`, `script/DeployAll.s.sol`, `.gas-snapshot`), **não um crate Rust**. O domínio coincide (EVM), o tipo de artefacto e a função não: §16 declara "USDC ERC-20 + Escrow + Settlement + Event Listener" em Rust, com `Cargo.toml`. <br>Zero ocorrências de `arkhe-evm` fora do Documento Mestre. |
| 12 | `arkhe-hathor-notary` (§17) | **NÃO REALIZADO** | **`arkhe-nostr-anchor` REFUTADO como identidade.** O candidato existe e é real: `safe-core-monorepo/crates/arkhe-nostr-anchor/Cargo.toml` → `description = "FI-032 — real NIP-01 event IDs and BIP-340 Schnorr signing (k256), anchoring an arkhe-identity Gdid to a Nostr identity-root event. No live relay/WebSocket client."` Mas (a) ancora **identidade**, não hashes arbitrários, e (b) ancora na **Nostr**, não na Hathor Network. <br>**Prova negativa decisiva:** `git grep -il 'hathor'` devolve **exatamente um arquivo — o próprio Documento Mestre**. Zero substrato Hathor no repositório. A analogia funcional (âncora externa) existe; a correspondência declarada não. |
| 13 | `arkhe-cognition` (§18) | **NÃO REALIZADO** | Sem candidato proposto no pedido; **não proponho um**. Os crates que tocam cognição são `arkhe-agi` (`"AGI coordinator with safety, inference, memory, and reflection"`), `arkhe-reasoning` (`"FI-031 — acyclic execution plans, detected via Kahn's algorithm"`), `arkhe-agi-coordinator` e `arkhe-reflector-agent`. Nenhum se autodescreve como camada cognitiva. <br>**O que faltaria para confirmar:** qualquer `description` ou cabeçalho que reivindique "camada cognitiva". Não existe nenhum. `arkhe-cognition` aparece em 1 arquivo: o Documento Mestre. |
| 14 | `arkhe-lean` (§15) | **NÃO REALIZADO como crate** | **Trabalho Lean real existe — mas não neste crate nem nestes caminhos.** Prova negativa dupla: (a) `arkhe-lean` (estrito) aparece em **1 arquivo: o Documento Mestre**; (b) os dois ficheiros de prova nomeados em §5/§15, `proofs/compose_lipschitz.lean` e `proofs/precision_stability.lean`, **não existem** — `git grep -ln 'compose_lipschitz\|precision_stability'` → zero; `git ls-files \| grep -i 'lipschitz\|precision_stab'` → zero. <br>O que **existe**: **51 arquivos `.lean` rastreados**, incluindo `src/Governance/**` (6: `External/ClusterProtocol.lean`, `Integrity/ToonLedgerTheorems.lean`, `Metatron/CubeTheorems.lean`, `Randomness/ARPAVerifiable.lean`, `Statistics/LikelihoodTheorems.lean`, `Thermodynamics/ExactInequalities.lean`), `lean-llm-agi/` (7 arquivos: `AGI.lean`, `Boundary.lean`, `LLM.lean`, `Percolation.lean`, `lakefile.toml`, `lake-manifest.json`, `lean-toolchain`), `lean-ethereum/` (4: `Ethereum.lean`, `lakefile.toml`, ...), e a ponte `arkhe-monorepo/packages/arkhe-lean-bridge`. <br>**Conclusão:** a *função* (provas formais) está realizada, dispersa em ≥8 diretórios; o *crate* `arkhe-lean` e as *duas provas nomeadas* não existem. |

**Resumo de (a):** 5 resolvidos com evidência (existem com esse nome) · 0 "candidatos não confirmados" puros · **9 NÃO REALIZADOS** — dos quais 7 não aparecem em nenhum arquivo além do próprio documento. Nenhum candidato proposto no pedido sobreviveu como correspondência confirmada.

---

## 2. Tabela (b) — Os 125 diretórios das três raízes fora do documento

**Como ler.** Cada linha traz o diretório, a função que serve **segundo o Documento Mestre** (com a parte do documento que a declara) e a evidência literal do próprio crate. Quando o documento não declara função correspondente, a linha diz `FORA DO ESCOPO DA ARQUITETURA`.

**Vocabulário de mapeamento** (só os eixos que o documento declara):

- **P1** — Pilar 1, Identidade e Comunicação / P2P (§3 Pilar 1, §24)
- **P2** — Pilar 2, Computação e Inteligência / Edge (§3 Pilar 2, §9)
- **P3** — Pilar 3, Verificação e Consenso (§3 Pilar 3, §21–§25)
- **P4** — Pilar 4, Económico (§3 Pilar 4, §27–§28, §30–§31)
- **§n** — a secção concreta que declara aquela função
- **FDS** — `FORA DO ESCOPO DA ARQUITETURA`: o documento não declara esta função em parte alguma
- **SEM MANIFESTO** — diretório sem `Cargo.toml` rastreado (não participa de nenhum workspace)

### 2.1 `safe-core-monorepo/crates/` — 59 dos 62 (excluídos os declarados `arkhe-core`, `arkhe-governance`, `arkhe-rsi`)

| Diretório | Função no documento | Evidência (citação literal) |
|-|-|-|
| `arkhe-agent-vm` | P1 — identidade de agentes | `Cargo.toml:6` → `"Arkhe Agent VM (AAVM) — lifecycle-managed, policy-constrained agent identities backed by hybrid PQC signatures and GDID"` |
| `arkhe-agi-coordinator` | P2 / §4 — orquestração do pipeline | `Cargo.toml` → `"AGI Coordinator — orquestra SafetyEnforcer + InferenceEngine + EvolutionaryMemoryV2"` |
| `arkhe-agi` | P2 / §18 — camada de agente | `Cargo.toml:5` → `"AGI coordinator with safety, inference, memory, and reflection"` (8 arquivos em `src/`) |
| `arkhe-artifact-signing` | §23 — selos e assinatura de artefactos | `Cargo.toml` → `"Cryptographic signing of artifacts using ed25519-dalek v2.2"` |
| `arkhe-blossom` | P1 — camada Nostr / armazenamento endereçado | `Cargo.toml` → `"Blossom protocol (BUD-01) support: SHA-256-addressed blob storage and Nostr-event authorization, layered on arkhe-storage and arkhe-nostr-anchor"` |
| `arkhe-bom` | **FDS** | `Cargo.toml` → `"AI Bill of Materials — CycloneDX ML-BOM generation"`. O documento não declara ML-BOM em nenhuma secção. |
| `arkhe-buzz` | **SEM MANIFESTO** | Zero arquivos rastreados no diretório. O próprio workspace o documenta: `safe-core-monorepo/Cargo.toml:31-38` → `"TEMPORARILY DISABLED 2026-09-14 ... exists as a directory but has no Cargo.toml -- its only file is an untracked src/bin/arkhe-buzz-server.rs"` |
| `arkhe-cli` | §7 — a CLI do `arkhe-core` | `Cargo.toml` → `"Command-line interface for Arkhe OS"`; `src/lib.rs` → `"CLI — re-export."` |
| `arkhe-cloud-provider` | **FDS** | `Cargo.toml` → `"Generic cloud provider trait with OpenNebula and EGI/OpenStack implementations"`. O tópico mais próximo é §29 ("Nillion e mercados de GPU 🔭"), que é aspiracional e não declara nenhum provedor de nuvem. |
| `arkhe-configuration` | **FDS** | `Cargo.toml` → `"Hierarchical configuration with env vars, files, and feature flags"`. Infraestrutura transversal; nenhuma secção do documento a declara. |
| `arkhe-crypto-pqc` | §22 — criptografia pós-quântica (**e o CHK**) | `Cargo.toml` → `"Hybrid post-quantum cryptography (ML-DSA-65 + Ed25519, ML-KEM-1024) for Arkhe Web3 security (FI-W01, FI-W02)"`; `src/chk.rs` é aqui que o CHK declarado em §10 existe. |
| `arkhe-deploy` | **FDS** | `Cargo.toml` → `"Declarative deployment and rollback for Arkhe OS — NixOS-inspired"` |
| `arkhe-document-loaders` | **FDS** | `src/lib.rs:1` → `"Carregadores de documentos com detecção de PII e políticas."` — o mais próximo é §34 (regulação), que não declara este componente. |
| `arkhe-event-bus` | P1 — comunicação tipada entre componentes | `Cargo.toml:6` → `"...typed publish/subscribe bus for the Arkhe Trust Infrastructure. Implements the §5.2 EventBus / Publisher / Subscriber contract of the canonical Arkhe paper (DOI 10.5281/zenodo.21383201)..."` ⚠️ **Atenção:** o "§5.2" citado aqui é de um *paper*, **não** do §5 deste Documento Mestre. Não é evidência de reconciliação. |
| `arkhe-evidence` | P3 / §25 — registo de evidência encadeado | `Cargo.toml` → `"FI-011/FI-017 — hash-chained, tamper-evident evidence log (prev_hash-linked BLAKE3 records)"` |
| `arkhe-geometric-verifier` | P3 — verificação de turnos de agente | `Cargo.toml` → `"FI-120–FI-125 — canonicalize -> BLAKE3 -> invariant checks over a turn, plus the typed provenance and memory graphs built from it"` |
| `arkhe-governance-bridge` | P1 — ponte de identidade DID↔SID/UID | `Cargo.toml` → `"Cross-platform identity bridge — DID↔SID/UID mapping + unified policy evaluation"` |
| `arkhe-hallucination` | **FDS** | `Cargo.toml` → `"Hallucination detection — confidence estimation and semantic consistency"`, **mas** `src/lib.rs:1` → `"⚠️ STUB ARQUITETURAL — Não é detecção real de alucinações."` O próprio crate desmente a sua `description`. |
| `arkhe-health-check` | **FDS** | `Cargo.toml` → `"Health check framework with dependency status and readiness probes"` — infraestrutura transversal. |
| `arkhe-identity` | P1 — identidade de dispositivo | `src/lib.rs:1` → `"Identidade de dispositivo (GDID) para o ARKHE OS."` |
| `arkhe-inference` | P2 / §9 — motor de inferência (candidato a `arkhe-omni`, não confirmado) | `Cargo.toml:5` → `"Unified inference engine with multiple backends"`; 9 arquivos em `src/`; backends candle/mistralrs/llama-cpp **opcionais** |
| `arkhe-input-validation` | **SEM MANIFESTO** | Sem `Cargo.toml`. Tem **1 arquivo rastreado**: `src/lib.rs`, cujo cabeçalho é `"Validação de entrada para sistemas de IA."` Não é compilado por nenhum workspace. Função não declarada no documento → FDS. |
| `arkhe-langchain-bridge` | P2 — integração com agentes externos | `Cargo.toml` → `"Bridge between Arkhe policy gateway and LangChain Rust models"` |
| `arkhe-langgraph` | P2 / §18 — orquestração de estados | `src/lib.rs:1` → `"LangGraph – orquestração de estados com auditoria."` |
| `arkhe-lean-spec-derive` | §19/§20 — geração de spec Lean a partir de Rust | `src/lib.rs:7` → `#[proc_macro_derive(LeanSpec)]`; `Cargo.toml` → `[lib] proc-macro = true` |
| `arkhe-llm-adapter` | P2 — adaptador de LLM | `Cargo.toml` sem `description`; `src/lib.rs` sem doc-header; 3 arquivos em `src/`. **Evidência de função limitada ao nome do diretório.** |
| `arkhe-mcp-bridge` | P2 — integração MCP | `Cargo.toml` → `"MCP protocol bridge"`; `src/lib.rs:1` → `"MCP Bridge — stub."` |
| `arkhe-mcp` | P2 / §4 — servidor MCP que expõe verificação | `Cargo.toml` → `"...servidor MCP (stdio) que expõe a verificação do Arkhe a agentes de IA. As ferramentas delegam às funções reais de arkhe-verify..."` |
| `arkhe-memory-v2` | P2 — memória evolutiva | `src/lib.rs:1` → `"Evolutionary Memory V2 — stub."` |
| `arkhe-memory` | P2 — memória de agente | `Cargo.toml` e `src/lib.rs` sem `description`/doc-header; 1 arquivo em `src/`. Evidência apenas nominal. |
| `arkhe-mobile-bridge` | **FDS** | `Cargo.toml` → `"Mobile FFI bridge — exposes policy check + audit to Android/iOS via uniffi"`. "Edge-First" é princípio declarado (§ cabeçalho), mas nenhuma secção declara um bridge móvel. |
| `arkhe-network` | P1 / §24 — mensagens autenticadas, nonces | `Cargo.toml` → `"FI-071/075/077/078 — authenticated messages, per-sender nonces, panic-free parsing, TLS 1.3 config. Not a full P2P/transport stack — see README."` |
| `arkhe-nostr-anchor` | P1 / §24 — raízes de identidade Nostr | `Cargo.toml` → `"FI-032 — real NIP-01 event IDs and BIP-340 Schnorr signing (k256), anchoring an arkhe-identity Gdid to a Nostr identity-root event. No live relay/WebSocket client."` |
| `arkhe-orcid` | P1 — identidade científica | `Cargo.toml` → `"ORCID iD validation (ISO 7064 mod 11-2) and verification against the ORCID public API"` |
| `arkhe-output-filter` | **FDS** | `Cargo.toml` → `"Output filtering — schema validation, harmful content, injection blocking"`. `src/` tem **0 arquivos**. |
| `arkhe-pea` | **FDS** | **Sem `description`, sem doc-header.** `src/lib.rs` tem **2 linhas**: `// crates/arkhe-pea/src/lib.rs` e `#![warn(missing_docs)]`. Crate declarado no workspace (`safe-core-monorepo/Cargo.toml:6`) e efetivamente vazio. Nenhuma evidência de função existe. |
| `arkhe-policy-gateway` | §32 — execução de política com auditoria | `Cargo.toml` → `"Unified policy enforcement gateway with Rego + audit trail"` |
| `arkhe-policy-regorus` | §32 — motor Rego/OPA | `Cargo.toml` → `"Rego/OPA policy engine integration for Arkhe OS"` |
| `arkhe-pqc-core` | §22 — criptografia pós-quântica (implementação paralela) | `Cargo.toml` → `"Hybrid PQC (Ed25519 + ML-DSA-65 via pqcrypto-dilithium, ML-KEM-1024 via pqcrypto-mlkem) — parallel implementation to arkhe-crypto-pqc, kept as a separate crate by explicit choice rather than a replacement"` |
| `arkhe-prompt-detector` | **FDS** | `Cargo.toml` → `"Prompt injection detection — AISVS C2.1.3, C2.1.8"`. O documento não menciona AISVS. |
| `arkhe-rate-limit` | **FDS** | `Cargo.toml` → `"Token and request rate limiting for AI systems — AISVS C9.4"` |
| `arkhe-reasoning` | P2 / §18 — planos de execução | `Cargo.toml` → `"FI-031 — acyclic execution plans, detected via Kahn's algorithm (topological sort)"` |
| `arkhe-reflector-agent` | P2 / §14 — reflexão e aprendizagem (candidato a `arkhe-cognition`) | `Cargo.toml` → `"Reflects on sessions and extracts learning patterns"` |
| `arkhe-rsi-core` | §14 — tipos base do loop RSI | `Cargo.toml` → `"FI-050 — core types for the RSI iteration loop: artifacts, evaluations, checkpoints, hash-chained iteration records"` |
| `arkhe-secrets-management` | **FDS** | `Cargo.toml` → `"Secrets management with in-memory store, file backend, and audit logging"` |
| `arkhe-session-evaluator` | P2 / §14 — avaliação de sessões | `Cargo.toml` → `"Evaluates agent sessions for causal consistency and quality"` |
| `arkhe-storage` | P3 / §22 — armazenamento endereçado por conteúdo (**CHK**) | `Cargo.toml` → `"FI-032 — content-addressed storage: convergent CHK encryption, Merkle-verified chunked objects, and ML-KEM key encapsulation for cross-agent sharing"` |
| `arkhe-syscall-bridge` | **FDS** | `Cargo.toml` → `"Syscall mapping registry — Windows↔Linux bidirectional translation tables"` |
| `arkhe-tee` | P3 — atestação de hardware | `Cargo.toml` → `"arkhe-tee — TDX/RTMR attestation primitives: quote metadata (MRTD/MR* + RTMR0-3 + report_data)... No line of this crate has been validated on TDX hardware."` |
| `arkhe-testing-framework` | **FDS** | `Cargo.toml` → `"Shared test utilities, fixtures, and assertion helpers for Arkhe OS"`. `§23` (CI/gates) é o tópico mais próximo, mas não declara este crate. |
| `arkhe-tool-sandbox` | **FDS** | `Cargo.toml` → `"WASM-based tool sandboxing — AISVS C9.3.1"`. `src/` tem **0 arquivos**. |
| `arkhe-unified-fs` | **FDS** | `Cargo.toml` → `"Unified filesystem — single namespace across Windows (NTFS) and Linux (ext4)"` |
| `arkhe-vector-store` | P2 — recuperação/vector store | `Cargo.toml` → `"Vector store abstraction with policy enforcement"` |
| `arkhe-verify-wasm` | P3 — verificação client-side no browser | `Cargo.toml` → `"Plano Arkhe OS §2.1–2.3 — verificação client-side que carrega no navegador: SHA-256, Ed25519 contra trust root, inclusão Merkle RFC 6962, quórum de witnesses..."` |
| `arkhe-verify` | P3 — casca nativa de verificação | `Cargo.toml` → `"Plano Arkhe OS §2.3 — casca nativa sobre o core de verificação de arkhe-verify-wasm ... com o modelo de dados no formato Rekor/CT"` |
| `arkhe-web3-security` | P3 — invariantes de segurança Web3 | `Cargo.toml` → `"Web3 security invariants — OWASP Smart Contract Top 10 2026 (FI-W01..FI-W20)"` |
| `arkhe-wormgraph` | P3 — grafo causal append-only (história) | `Cargo.toml` → `"Plano Arkhe OS §1.5 — grafo causal (wormgraph): nós e arestas tipados, append-only com hash encadeado BLAKE3"` |
| `safe-core-crypto` | P1/P3 / §22 — cripto e identidade | Sem `description`. `crates/README.md` → `"├── safe-core-crypto/           # Camada 0+1 — cripto e identidade"` (5 arquivos em `src/`) |
| `safe-core-policy` | §32 — política / invariante | Sem `description`. `crates/README.md` → `"└── safe-core-policy/           # Camada 4 — política / Invariante I13"`; `src/consensus_guard.rs` → `"Policy::evaluate (estrutural, sem CoT)"` |

### 2.2 `arkhe-monorepo/packages/` — 61 dos 64 (excluídos os declarados `arkhe-core`, `arkhe-pqc`, `arkhe-p2p`)

| Diretório | Função no documento | Evidência (citação literal) |
|-|-|-|
| `arkhe-actuators` | **FDS** | `Cargo.toml` → `"ARKHE RF cell modulator (US10064941B2) — biological actuators controlled by evidence, guarded by the I18 calcium-drift invariant"` — hardware biológico; o documento não o declara. |
| `arkhe-aid` | P1 — identidade e delegação de agentes | `Cargo.toml` → `"AID — Agent Identity & Delegation (bloco 1074, VAIP). ...ancorada no IETF Internet-Draft draft-nyantakyi-vaip-agent-identity-01 (Ed25519, sete escopos hierarquicos, trust score 0-100...)"` |
| `arkhe-avalon-agc` | **FDS** | `Cargo.toml` → `"AVALON distributed AGC controller (Kyiv/Joule-style RF gain control) — Arkhe OS. No_std core..."` — RF/hardware. |
| `arkhe-avalon-bridge` | **FDS** | `Cargo.toml` → `"AVALON v2.3 PyO3 bridge — exposes the AGC controller and Safe-Core evidence to Python (3.14)"` |
| `arkhe-bitcoin` | P4 — custódia/economia Bitcoin | `Cargo.toml` → `"Geração de chaves e endereços Bitcoin para o ecossistema ARKHE — P2PKH, P2WPKH, P2TR (Taproot), WIF e proof-of-transit (PoTT) receipts opcionais."` |
| `arkhe-blink-bridge` | P4 — ponte económica MEV | `Cargo.toml` → `"UnifiedBlinkBridge (bloco 1052): consolidacao tripla MEV (001-006) + ANTH (001-006) + ARKHE (I619/I622/I623/I624) numa ponte unica..."` |
| `arkhe-block-registry` | P3 — registo canónico de blocos (candidato a `arkhe-ledger`, não confirmado) | `Cargo.toml` → `"ARKHE-BLOCK-REGISTRY — Registro canonico de blocos do ledger (bloco 1075, v582.0-exec). Formato canonico do hash como hex string de 64 caracteres..."` |
| `arkhe-buzz-bridge` | P1 — transporte Nostr | `Cargo.toml` → `"ARKHE Fountain Protocol <-> Buzz (Nostr) bridge — evidence transport with Z0-Z3 firewall validation"` |
| `arkhe-buzz` | **FDS** | `src/lib.rs:1` → `"ARKHE / SUBSTRATE FISSION THEORY (SFT) — Buzz Bridge (honest simulation core)"` — teoria de substrato, não declarada. (Nota: existe o **mesmo nome** em `safe-core-monorepo/crates/arkhe-buzz`, que está vazio.) |
| `arkhe-catedral-v152` | **FDS** | `Cargo.toml` → `"Catedral OS v152.0 — materialização física: estrutura espectral (I157–I160), construção atômica (I161–I166)..."` |
| `arkhe-crypto` | P3/P4 — deteção de falha DKG, BIP-322 | `src/lib.rs:1` → `"arkhe-crypto — DKG failure detection, hash security, BIP-322 verification."` |
| `arkhe-crystallography` | **FDS** | `Cargo.toml` → `"ARKHE CRYSTALLOGRAPHY — densidade eletrônica (#6) via FFT e transformadas de difração para materiais"` |
| `arkhe-did` | P1 — identidade descentralizada | `Cargo.toml` → `"ARKHE DID — validação de identidade descentralizada (#14): SIWE EIP-4361 e mensagens EIP-712 sobre secp256k1"` |
| `arkhe-ecdsa-mitm` | **FDS** | `Cargo.toml` → `"ARKHE ECDSA·MITM — analogia de rotação de fase: o parâmetro CP δ da matriz de mistura de camadas (arkhe-field) age como rotação do digest ECDSA..."` |
| `arkhe-epistemic` | §20 — verificação e ponte Lean | `Cargo.toml` → `"Módulo epistémico do ARKHE — verificação aritmética e ponte Lean"` |
| `arkhe-field-stability` | P3 parcial — quórum 2/3 (substrato mais próximo de §13) | `Cargo.toml` → `"Métricas de estabilidade de campo, relatórios de qualidade e stub MCP (reqwest)..."`; `src/validators.rs:24` → `pub const VALIDATOR_QUORUM: f64 = 2.0 / 3.0;` |
| `arkhe-field` | **FDS** | `Cargo.toml` → `"ARKHE Field 1.0 — escala de 2370 MeV (X(2370)/BESIII, ICHEP 2026), correspondência VSEPR-Glueball, constantes JUNO (Nature 654, 2026)..."` |
| `arkhe-firmware-consciousness` | **FDS** | `Cargo.toml` → `"ARKHE Firmware Consciousness v0.1.0 — no_std consciousness governance for embedded devices..."` |
| `arkhe-gap` | **FDS** | `Cargo.toml` → `"ARKHE-VSEPR Núcleo 3 — Gap de Proteção (Camada 3). Selo #2 (Robust Quantum Extremal Numbers, arXiv:2608.13907)..."` |
| `arkhe-gpu` | **FDS** | `Cargo.toml` → `"Camada GPU da Catedral (bloco 1011, v390.2): governança de lançamentos ... Track real Tile (crate cutile, feature tile) e SIMT (feature simt) off por default; nenhuma das tracks compila neste ambiente"`. §29 menciona "mercados de GPU" de forma aspiracional; não declara este crate. |
| `arkhe-grover` | **SEM MANIFESTO** | Sem `Cargo.toml`. Contém **3 arquivos rastreados**: `src/grover_corrected.py`, `src/grover_6_011001_ancilla.qasm`, `src/qft_5q_rf_tone.qasm` — projeto Python/QASM, não um crate Rust. Função não declarada → FDS. |
| `arkhe-haselgrove` | **FDS** | `Cargo.toml` → `"ARKHE Haselgrove ray tracer v3.2 — Hamiltonian radio-ray tracing in spherical coordinates (Jones & Stephenson OT 75-76)..."` — ionosfera. |
| `arkhe-hypergraph` | **FDS** | `Cargo.toml` → `"ARKHE Hypergraph Portal Gun: S01-S15 validator agents, hyperedges, and evolution operations (pure Rust)."` |
| `arkhe-ia-dtn` | **FDS** | `Cargo.toml` → `"ARKHE IA DTN — deep-space Delay/Disruption Tolerant Networking agent: BPv7 beacon discovery, CGR routing..."`; **excluído** do workspace: `arkhe-monorepo/Cargo.toml:7` → `exclude = ["packages/arkhe-ia-dtn", "packages/arkhe-core"]` |
| `arkhe-incentives` | P4 — economia de nós | `Cargo.toml` → `"ARKHE INCENTIVES — economia de nós de transporte (#13): fidelidade de contribuição, alocação de recompensas e sanções"` |
| `arkhe-invariant-registry` | P3 / §23 — registo de invariantes | `Cargo.toml` → `"Registry canonico de invariantes (bloco 1057 ratificado): IDs atestados apenas por realizacao testada; referencias sem substrato sao rejeitadas..."` |
| `arkhe-lean-bridge` | §20 — ponte Lean↔Rust | `Cargo.toml` → `"Ponte Lean↔Rust (v494.1 real): verificação centralizada no kernel Lean 4.33.1 como único ponto de confiança (FFI forte), fingerprints SHA3-256 (Ghost-1)..."` |
| `arkhe-mhd` | **FDS** | `Cargo.toml` → `"ARKHE Block 21 Resistive MHD phase-field engine — a pseudo-magnetic induction law whose magnetic reconnection drives Ledger handovers..."` |
| `arkhe-neurogenesis` | **FDS** | `Cargo.toml` → `"ARKHE Neurogenesis (344-AGENT-NEUROGENESIS) — deterministic dendritic growth driven by a non-Hermitian evolution operator"` |
| `arkhe-nostr` | P1 / §24 — semântica de eventos Nostr | `Cargo.toml` → `"Nostr-compatible event semantics with Arkhe PQC attestation layer (pure Rust, no C)."` |
| `arkhe-oam` | **FDS** | `Cargo.toml` → `"ARKHE OAM — pureza da carga de momento angular orbital (#10) para o z-puro photônico"` |
| `arkhe-observability` | **FDS** | `Cargo.toml` → `"ARKHE Observatory — bridges the Safe-Core anti-hallucination policy (491-AGI-CORTEX) into the Telegraph ξM-field bus..."` |
| `arkhe-orbital` | **FDS** | `Cargo.toml` → `"Consciência orbital do ARKHE OS — padrões emergentes de Saturno como arquitetura de agentes auto-organizáveis..."` |
| `arkhe-orchestrator-gate` | §4/§7 + §20 — gate de orquestração sobre veredito Lean | `Cargo.toml` → `"Gate mínimo real de orquestração (bloco 1010, v390.1): invariantes I534/I535 — autorização de passo só com veredito verified do kernel Lean (arkhe-lean-bridge)..."` |
| `arkhe-pacts` | P3/P4 — compromisso criptográfico temporal | `Cargo.toml` → `"PACTs commitment sobre BIP-322 ... prova vinculada ao momento (timestamp), compromisso irrevogavel hash(salt \|\| bip322_proof) e nao-transferivel."` |
| `arkhe-photonics` | **SEM MANIFESTO** | Sem `Cargo.toml`. **5 arquivos rastreados**, incluindo um projeto Lean: `PhotonicCore.lean`, `lakefile.toml`, `lake-manifest.json`, `lean-toolchain`, `photonics_sim.py`. Função não declarada → FDS. |
| `arkhe-pott` | P4 — recibos de trânsito Bitcoin | `Cargo.toml` → `"Proof-of-Transit Timestamping (PoTT) — hop-timed custody receipts for interplanetary Bitcoin over DTN (arXiv:2508.20591)..."` |
| `arkhe-qhttp` | **FDS** | `Cargo.toml` → `"ARKHE-VSEPR Núcleo 3 — qhttp (Camada 4, Processamento de Sinais de Fase). Selo #5 (arXiv:2608.14387)..."` |
| `arkhe-quantum-validator` | **SEM MANIFESTO** | Sem `Cargo.toml`. **1 arquivo rastreado**: `src/arkhe_quantum_validator/validator_v3.py` (Python). FDS. |
| `arkhe-recurrency-daemon` | **FDS** | `Cargo.toml` → `"gRPC transport for the ARKHE Recurrency engine (491-AGI-CORTEX)..."` |
| `arkhe-recurrency-temporal` | **FDS** | `Cargo.toml` → `"v0.4.3 cyber-physical safety layer for 491-AGI-CORTEX — TemporalStateDaemon (EWMA history), MissionSafetyKernel (iGuard webhook)..."` |
| `arkhe-recurrency` | **FDS** | `Cargo.toml` → `"ARKHE Recurrency (491-AGI-CORTEX) — functional simulation of 4-level recurrency (cellular/local/global/lateral)..."` |
| `arkhe-reputation` | P4 — reputação e governança de operadores | `Cargo.toml` → `"ARKHE REPUTATION — sistema de reputação TOON (#4), matriz de governança (#15) e certifier automático (#17)"` |
| `arkhe-reveng` | **SEM MANIFESTO** | Sem `Cargo.toml`. **4 arquivos rastreados**: `RevengCore.lean` + `lakefile.toml`, `lake-manifest.json`, `lean-toolchain` (projeto Lean). FDS. |
| `arkhe-safe-manifold` | §34 — conformidade regulatória (EU AI Act, NIST, ISO) | `Cargo.toml` → `"SafeManifold v0.8.0 — 16-invariant security projection with EU AI Act, NIST RMF, ISO 42001, hybrid PQC, bias detection..."` |
| `arkhe-sft-orchestrator` | **SEM MANIFESTO** | Sem `Cargo.toml`. **8 arquivos rastreados** (Python): `sft_orchestrator.py`, `arkhe_pipeline_full.py`, `arkhe_bayes_v22.py`, `arkhe_null_parallel.py`, `arkhe_sft_sim.py`, `magthomscatt_wrapper.py`, `extract_ixpe_stokes.py`, `NOTES.md`. FDS. |
| `arkhe-sft` | **SEM MANIFESTO** | Sem `Cargo.toml`. **4 arquivos rastreados**: `SFTCore.lean` + `lakefile.toml`, `lake-manifest.json`, `lean-toolchain` (projeto Lean). FDS. |
| `arkhe-shadow` | **FDS** | `Cargo.toml` → `"ARKHE Block 22 — the latent memory retained after SVD compression (the tail of the spectrum), its retrocausal echo, and the reintegration ('healing') step"` |
| `arkhe-smeasure` | **SEM MANIFESTO** | Sem `Cargo.toml`. **4 arquivos rastreados**: `SMeasureCore.lean` + `lakefile.toml`, `lake-manifest.json`, `lean-toolchain` (projeto Lean). FDS. |
| `arkhe-stark` | §21 — ZK (candidato a `arkhe-zk`, refutado como identidade) | `Cargo.toml` → `"STARKs pos-quanticos condicionais a hash function ... SHA-256 (Grover: 128 bits PQ) vs SHAKE256 (FIPS 202: 256 bits PQ)..."` |
| `arkhe-svd` | **FDS** | `Cargo.toml` → `"ARKHE Block 20 SVD Compression Protocol — truncated singular-value compression of EVO/state matrices..."` |
| `arkhe-thz-sensor` | **FDS** | `Cargo.toml` → `"Graphene metamaterial THz sensor surrogate model (Amraoui et al., 2026) — dual-band Lorentzian+Drude absorption..."` |
| `arkhe-timechain` | P3 — ledger e finalidade (candidato a `arkhe-ledger`/`arkhe-blockchain`, não confirmado) | `src/lib.rs:5-11` → `"ARKHE Timechain — closing the loop around the MHD phase-field and the Shadow with a real P2P overlay. ...a block becomes final when a majority of those echoes interfere constructively (consensus — 'loop closure'); balances live as flux-tube UTXOs..."` |
| `arkhe-tokenization` | P4 — tokenização de features | `Cargo.toml` → `"ARKHE TOKENIZATION — pipeline de tokenização determinística de features (#12) com orçamento de entropia (Gap-2)"` |
| `arkhe-topology` | **FDS** | `Cargo.toml` → `"ARKHE TOPOLOGY — invariantes topológicos: número de Skyrmion (#8), coerência φ (#7) e validação topológica via TDA..."` |
| `arkhe-tzinor` | **FDS** | `Cargo.toml` → `"ARKHE-VSEPR Núcleo 3 — Canal Tzinor (Quantum Switch ICO). Selo #1 (arXiv:2608.13997)..."` |
| `arkhe-vss` | P3 / §22 — partilha verificável de segredos | `Cargo.toml` → `"Feldman Verifiable Secret Sharing (ARKHE-CRYPTO-CODE-TESTABLE-2026-09-12) — share verification g^{s_i} = prod C_k^{i^k}..."` |
| `arkhe-web3-bounty` | **SEM MANIFESTO** | Sem `Cargo.toml`. **7 arquivos rastreados**: `bug_bounty_web3.lean` + projeto Lean, `lean_bounty.py`, `spec_erc20.json`, `test_fixtures/vault/Vault.sol`. FDS. |
| `arkhe-xloop` | **FDS** | `Cargo.toml` → `"Kronos field — the XLoop execution loop as a metric field (ds² = −c²dt² + τ_eff·dℓ²)..."` |
| `arkhe-xy-simulator` | **FDS** | `Cargo.toml` → `"ARKHE XY Simulator (P1-P4) — Kuramoto / Langevin(KT) / chiral-DMI / pinned-defect phase-lattice models for the Cathedral V8.6 NxN skyrmion mesh"` |
| `identity` | P1 — derivação de identidade pós-quântica | `Cargo.toml` → `"Post-quantum identity derivation for ARKHE — BIP39 → ML-DSA-65 via HKDF-SHA3-256"` |

### 2.3 `crates/` — os 5 diretórios restantes

| Diretório | Função no documento | Evidência |
|-|-|-|
| `crates/crates` | **contêiner, não é crate** | Sem `Cargo.toml`. Contém apenas `safe-core-crypto/` e `safe-core-policy/`. Ver §3.1. |
| `crates/crates/safe-core-crypto` | P1/P3 / §22 — cripto e identidade | **Membro declarado** do workspace: `crates/Cargo.toml:3` → `"crates/safe-core-crypto"`; resolvido por Cargo para este caminho (ver §3.1). Conteúdo byte-idêntico ao de `crates/safe-core-crypto`. |
| `crates/crates/safe-core-policy` | §32 — política / invariante | **Membro declarado**: `crates/Cargo.toml:4` → `"crates/safe-core-policy"`. Byte-idêntico a `crates/safe-core-policy`. |
| `crates/safe-core-crypto` | P1/P3 / §22 — cripto e identidade | **Órfão de manifesto**: nenhum `Cargo.toml` do repo o referencia. Byte-idêntico ao de `crates/crates/safe-core-crypto` (`diff -rq` → exit 0, 4763 bytes cada). |
| `crates/safe-core-policy` | §32 — política / invariante | **Órfão de manifesto**: idem. Byte-idêntico (`diff -rq` → exit 0, 1943 bytes cada). |

### 2.4 Resultado de (b)

| Classificação | §2.1 | §2.2 | §2.3 | Total |
|-|-:|-:|-:|-:|
| Serve função declarada no documento (P1/P2/P3/P4/§n) | 40 | 23 | 4 | **67** |
| `FORA DO ESCOPO DA ARQUITETURA` (FDS) | 17 | 30 | 0 | **47** |
| `SEM MANIFESTO` (FDS + sem `Cargo.toml` rastreado) | 2 | 8 | 0 | **10** |
| Contêiner sem `Cargo.toml` (`crates/crates`) | 0 | 0 | 1 | **1** |
| **Total** | **59** | **61** | **5** | **125** |

Nenhum dos 125 foi classificado como "resolve uma das 9 lacunas de §5/§6": a classificação acima diz que um crate *serve um eixo do documento*, não que seja o crate declarado. Os 9 nomes declarados continuam sem substrato com esse nome.

Três achados de leitura que vale reter, todos com evidência acima:

- **Dois crates com `src/` vazio não são membros de workspace nenhum:** `arkhe-output-filter` (0 arquivos em `src/`) e `arkhe-tool-sandbox` (0 arquivos em `src/`). Nenhum dos dois aparece em `"crates/…"` ativo do manifesto — não são compilados, não falham, simplesmente não existem.
- **`arkhe-pea` é membro ativo do workspace e está vazio:** `safe-core-monorepo/Cargo.toml:6` → `"crates/arkhe-pea"` (não comentado), e o `src/lib.rs` tem **2 linhas** (`// crates/arkhe-pea/src/lib.rs` e `#![warn(missing_docs)]`). Zero código.
- **`arkhe-hallucination` contradiz a própria `description`:** a `description` promete deteção de alucinações; `src/lib.rs:1` diz `"⚠️ STUB ARQUITETURAL — Não é detecção real de alucinações."`
- **A maioria dos FDS vem de um programa científico paralelo** — física de substratos, RF, materiais, DTN, quântica. Não é lixo: é um segundo projeto, que o Documento Mestre não descreve.

E uma correção de contagem que o manifesto obriga: `safe-core-monorepo/Cargo.toml` tem **42 linhas** contendo `"crates/…"`, mas **6 estão comentadas** → **36 membros ativos**, consistente com o comentário que o próprio ficheiro faz sobre as entradas desativadas.

---

## 3. O que a medição força — e o que ela não decide

### 3.1 `crates/crates/` — não é erro acidental; é o que o manifesto declara

`crates/Cargo.toml` está em `crates/`, logo os `members` são relativos a `crates/`. O manifesto diz `crates/safe-core-crypto`, que resolve para `crates/crates/safe-core-crypto`. Verificação empírica:

```bash
cargo metadata --manifest-path crates/Cargo.toml --no-deps --format-version 1 | tr ',' '\n' | grep manifest_path
# → "...\crates\crates\safe-core-crypto\Cargo.toml"
# → "...\crates\crates\safe-core-policy\Cargo.toml"
```

Consequências medidas:

1. **Os dois crates que o manifesto usa são os aninhados.**
2. **`crates/safe-core-crypto` e `crates/safe-core-policy` (ao nível de cima) não são membros de nenhum workspace** — são órfãos. E são **byte-idênticos** aos aninhados (`diff -rq` exit 0; 4763 e 1943 bytes em cada par).
3. O `crates/README.md` descreve a árvore como `safe-core/crates/safe-core-crypto/...` — isto é, descreve a **cópia de cima**, que é a que o Cargo **não** usa. O README e o manifesto discordam.
4. `crates/README.md` é explícito sobre o estado de verificação: *"este workspace **não** foi compilado no ambiente onde foi montado (sem toolchain Rust e sem acesso a crates.io). Foi validado por revisão estática."* E `crates/AUDIT.md` repete: *"Revisão estática apenas."*

**Não movi nem removi nada disto.** A recomendação fundamentada está no relatório.

### 3.2 O `arkhe-core` duplicado — qual é canónico pelo uso real

| | `safe-core-monorepo/crates/arkhe-core` | `arkhe-monorepo/packages/arkhe-core` |
|-|-|-|
| Papel no workspace | **membro** — `safe-core-monorepo/Cargo.toml:3` → `"crates/arkhe-core"` | **excluído** — `arkhe-monorepo/Cargo.toml:7` → `exclude = ["packages/arkhe-ia-dtn", "packages/arkhe-core"]` |
| Manifesto próprio | não | **sim** — `[workspace]` próprio (standalone), versão `0.3.3` |
| `description` | `"Foundation types, traits, and error handling for Arkhe OS"` | `"arkhe-core — pure transition (State/Event/apply) + append-only hash-chained ledger (Ghost-1)..."` |
| Arquivos em `src/` | **6** (`error.rs`, `hash.rs`, `lib.rs`, `memory_traits.rs`, `safety.rs`, `safety_traits.rs`) | **2** (`ledger.rs`, `lib.rs`) |
| Depende dele (medido) | **~45 crates** com `arkhe-core = { path = "../arkhe-core" }` ou `{ workspace = true }` | **0** — nenhum `Cargo.toml` do repositório tem uma dep `path` para `packages/arkhe-core` |
| Compila | sim — `cargo check --workspace` em `safe-core-monorepo` → **exit 0** | não é verificado por nenhum workspace |
| Documenta-se | — | o próprio crate declara o problema na sua `description`: `"Erratum (bloco 1077): workspace standalone proprio (o workspace arkhe-monorepo commitado referencia 22 crates nao commitados -> cargo nao resolve em CI)"` |

**Canónico pelo uso real: `safe-core-monorepo/crates/arkhe-core`.** A evidência é a assimetria de dependências (≈45 vs 0) e a participação no workspace que compila.

Terceira testemunha, independente: outros dois crates do mesmo monorepo citam este caso como precedente — `safe-core-monorepo/crates/arkhe-event-bus/Cargo.toml` e `arkhe-tee/Cargo.toml` dizem ambos: `"Same erratum pattern as arkhe-monorepo/packages/arkhe-core."`

E o essencial para a reconciliação: **nenhum dos dois faz o que §7 declara.** §7 diz que `arkhe-core` é "Binário de entrada. Carrega o modelo (via `arkhe-omni`), corre o pipeline de 10 passos". Nenhuma das duas `description` menciona pipeline, binário, modelo ou 10 passos.

### 3.3 As três raízes — quem depende de quem

Medido por `Cargo.toml` (`members`, `exclude`, deps `path`):

| Raiz | Papel medido | Manifesto | Membros | Depende de outra raiz? |
|-|-|-|-:|-|
| `crates/` (raiz) | mini-workspace autónomo "safe-core — núcleo canônico" | `crates/Cargo.toml` | 2 | **não** — não referencia nenhuma outra raiz |
| `safe-core-monorepo/` | workspace "Arkhe OS / 491-AGI-CORTEX" | `safe-core-monorepo/Cargo.toml` | 36 | **não** — nenhuma dep `path` sai da raiz |
| `arkhe-monorepo/` | workspace "Timechain / Catedral" | `arkhe-monorepo/Cargo.toml` | ~50 + `exclude` | **não** — nenhuma dep `path` sai da raiz |

**Não há dependências cruzadas entre as três raízes.** São três mundos disjuntos que partilham apenas nomes.

- **Crates que existem em duas árvores (mesmo nome exato).** Medido por interseção dos nomes de diretório com `comm -12`:

  ```bash
  ls -d safe-core-monorepo/crates/*/  | sed 's#.*/crates/##;s#/$##' | sort   > a
  ls -d arkhe-monorepo/packages/*/    | sed 's#.*/packages/##;s#/$##' | sort > b
  ls -d crates/*/                     | sed 's#crates/##;s#/$##'          | sort > c
  comm -12 a b   # → arkhe-buzz, arkhe-core
  comm -12 b c   # → (vazio)
  comm -12 a c   # → safe-core-crypto, safe-core-policy
  ```

  | Par de raízes | Nomes partilhados |
  |-|-|
  | `safe-core-monorepo` ↔ `arkhe-monorepo` | **`arkhe-buzz`, `arkhe-core`** |
  | `arkhe-monorepo` ↔ `crates/` | nenhum |
  | `safe-core-monorepo` ↔ `crates/` | **`safe-core-crypto`, `safe-core-policy`** |
  | **União de todos os nomes duplicados** | **`arkhe-buzz`, `arkhe-core`, `safe-core-crypto`, `safe-core-policy`** |

  Qualificações medidas, caso a caso:
  - `arkhe-core` — **duas implementações distintas** (6 arquivos `src/` vs 2; funções diferentes na `description`). Detalhe em §3.2.
  - `arkhe-buzz` — `arkhe-monorepo/packages/arkhe-buzz` tem `Cargo.toml` + `src/lib.rs`; `safe-core-monorepo/crates/arkhe-buzz` tem **zero arquivos rastreados**. Não são duas implementações: uma existe, a outra é um diretório vazio.
  - `safe-core-crypto` / `safe-core-policy` — **byte-idênticos** entre `crates/` e `crates/crates/` (`diff -rq` → exit 0). São a mesma implementação em dois caminhos.

- **Qual raiz concentra o que a arquitetura chama de núcleo?** §5 descreve um monorepo único `arkhe/` com `crates/` + `examples/` + `specs/`. Medido:
  - O nome `arkhe-core` aparece nas duas grandes raízes, com funções diferentes e sem deps cruzadas.
  - O crate que **mais se comporta como núcleo de facto** é `safe-core-monorepo/crates/arkhe-core`: ~45 dependentes, membro do único workspace que compila (exit 0), e é o tipo-base (`ArkheError`, `ArkheHash`, traits) do qual a maioria dos outros depende.
  - `arkhe-monorepo/packages/arkhe-core` está excluído, tem 2 arquivos e zero dependentes.
  - Logo: **o núcleo real está em `safe-core-monorepo/crates/`, não em `arkhe-monorepo/packages/` nem em `crates/`.** Mas note-se que nenhum destes é o núcleo *descrito* por §5–§7.

### 3.4 O que a medição não decide

- **Não decide intenção.** Que 9 dos 14 crates declarados não existam não diz que o autor não os quis fazer; diz que o documento não é um inventário do que existe.
- **Não decide prioridade.** Não há evidência, nos arquivos, de qual das três raízes deve sobreviver.
- **Não decide equivalência funcional onde só há semelhança de domínio.** Marquei `arkhe-inference`/`arkhe-stark`/`arkhe-timechain`/`arkhe-nostr-anchor` como candidatos ou refutações, nunca como correspondências — porque a evidência disponível é de *domínio*, não de *identidade de código*.

---

## Anexo A — Os 31 diretórios de crate fora das três raízes

Estes **não** entram na tabela (b) (que é estritamente sobre as três raízes), mas existem e alteram qualquer leitura do repositório como "três raízes". Listados com evidência.

| Diretório | `name` / evidência |
|-|-|
| `Cargo.toml` (raiz do repo) | `arkhe-safe-core-sdk` — **manifesto da raiz, já não parseável.** `cargo check --workspace` na raiz → **exit 101**: `"can't find phi_c_compute bench at benches\phi_c_compute.rs"`. O diretório `benches/` não existe. |
| `crates` | workspace "safe-core" — 2 membros (aninhados). Ver §3.1 |
| `safe-core-monorepo` | workspace raiz — 36 membros, **exit 0** |
| `arkhe-monorepo` | workspace raiz — ~50 membros + `exclude` |
| `arkhe-monorepo/arkhe-spectral` | workspace raiz (sem `description`) |
| `arkhe-monorepo/arkhe-spectral/crates/spectral-core` | `"Análise espectral de DAGs de tarefas: Laplaciano, Fiedler confiável/único, condicionamento e particionamento (ARKHE-SPECTRAL...)"` |
| `arkhe-monorepo/arkhe-spectral/crates/concept-pca` | `"Cobertura conceitual via PCA sobre embeddings: rank efetivo, entropia espectral e similaridade entre conceitos (ARKHE-SPECTRAL...)"` |
| `arkhe-monorepo/arkhe-spectral/crates/signature-audit` | `"Auditoria de assinaturas espectrais: comparação consistente/inconsistente e resíduos de verificação (ARKHE-SPECTRAL...)"` |
| `arkhe-monorepo/catedral-os-v304/rust` | workspace raiz (sem `description`) |
| `…/catedral-os-v304/rust/raft` | `catedral-raft` → `"Catedral OS v304.0 — Núcleo de consenso Raft com persistência e snapshots"` |
| `…/catedral-os-v304/rust/zk` | `catedral-zk` → `"Catedral OS v304.0 — Pool de provadores ZK com escalabilidade linear (I359)"` |
| `…/catedral-os-v304/rust/wasm` | `catedral-wasm` → `"Catedral OS v304.0 — Sandbox WASM para contratos de coerência (I361)"` |
| `…/catedral-os-v304/rust/hardware` | `catedral-hardware` → `"Catedral OS v304.0 — Hardware Abstraction Layer (HAL) e drivers de sensores de Φ"` |
| `arkhe-monorepo/catedral_os_v304/rust` (+ `raft`, `zk`, `wasm`, `hardware`) | **Segunda cópia, com hífen vs underscore no nome do diretório.** `diff -rq arkhe-monorepo/catedral-os-v304 arkhe-monorepo/catedral_os_v304` → **divergem** (playbook.yml, production.yaml, ProductionHardware.lean, coherence.py, zeno.py, decisor.py, orquestrador_v304.py, requisitos, Cargo.toml, hardware/Cargo.toml, ...). Não é duplicação byte-idêntica: são duas versões divergentes. |
| `arkhe-monorepo/services/safe-core` | `arkhe-safe-core` → `"Safe-Core policy for the 491-AGI-CORTEX substrate — the anti-hallucination 'collar of the StateDaemon'..."` — **membro do workspace `arkhe-monorepo`.** Nota do manifesto: `arkhe-monorepo/Cargo.toml:69-71` → `"renomeado de arkhe-safe-core para não colidir com services/safe-core"` |
| `arkhe-monorepo/tests/adversarial` | `arkhe-adversarial-corpus` → `"Declarative adversarial corpus exercising the SafeManifold constitutional invariants I-17..I-20 (arkhe-safe-manifold)"` |
| `arkhe-nostr` | `arkhe-nostr` → `"Nostr-compatible event encoding with Arkhe ML-DSA attestation layer (pure Rust, no C)."` — **nome já usado por `arkhe-monorepo/packages/arkhe-nostr`** |
| `arkhe-potts` | `arkhe-potts` → `"Potts-model energy, exact partition/probability, and MAP inference (exact brute-force oracle + Iterated Conditional Mode...)"` |
| `arkhe-saturon` | `arkhe-saturon` → `"Event-driven physics-verification orchestrator: a single-consumer async loop that registers arXiv-derived hypotheses..."` |
| `arkhe-soc-tlm` | `arkhe-soc-tlm` → `"Transaction-level golden model for the Arkhe SoC: AOTB payload, SRAM D/X, QPL, and the Smith-chart CORDIC..."` |
| `arkhe-ui/src-tauri` | `arkhe-ui-app` → `"Plano Arkhe OS §Fase 5 — o casco Tauri: a app de verificação como aplicativo instalável..."` — **fora do âmbito desta tarefa por instrução explícita** |
| `kernel` | membro do workspace da raiz (`Cargo.toml:4`) |
| `kernel/cmd/arkhe` | membro (`Cargo.toml:5`) |
| `kernel/bindings/cli` | membro (`Cargo.toml:6`) |
| `kernel/bindings/python` | membro (`Cargo.toml:7`) |
| `kernel/bindings/wasm` | membro (`Cargo.toml:8`) |
| `projects/arkhe-sagemaker-proxy` | membro (`Cargo.toml:8`) |

**Nota de honestidade sobre este anexo:** o `arkhe-monorepo/Cargo.toml` referencia `services/safe-core` e `packages/arkhe-ia-dtn`/`packages/arkhe-core` está excluído — mas o manifesto da raiz do repositório **já não parseia** (exit 101), portanto `kernel/*` e `projects/arkhe-sagemaker-proxy` são membros *declarados* de um workspace que não carrega. Não os verifiquei individualmente.

---

## Anexo B — Os 8 candidatos a lixo, medidos antes de qualquer remoção

Regra aplicada: **só se remove o que (a) é demonstravelmente vazio ou substituído, e (b) nada no conteúdo rastreado resolve para ele como caminho ou dependência.** Onde (b) falha, não se removeu.

| Arquivo | Tam. | Referências rastreadas (medido) | Veredicto |
|-|-:|-|-|
| `agi_core_v112.pl` | **0 B** | `ANALISE_CONSTITUCIONAL_v111.md:59` (diagrama `"PROLOG (agi_core_v112.pl)"`) e `:73` (tabela `"Arquivos Gerados"` → `"Núcleo Prolog com JWT HMAC RFC 7519 + RS256 verify"`) | **Referenciado.** Não removido. A referência é um documento que afirma conteúdo que o ficheiro não tem. |
| `arkhe_hermeneutic_wormhole_v4.py` | **0 B** | nenhuma (`git grep` → vazio) | Sem referências. |
| `arkhe_soliton_network_v4_1.py` | **0 B** | `soliton_network_report.json:2` → `"simulation": "ARKHE_SOLITON_NETWORK_v4_1"` (rótulo de dados, não caminho) | Referência fraca (etiqueta). |
| `cathedral_orchestrator_v112.py` | **0 B** | `ANALISE_CONSTITUCIONAL_v111.md:74` → `"Orquestrador Python com JWT Dual-Mode + revogação"` | **Referenciado.** Não removido. |
| `test_sub212_v112.py` | **0 B** | `ANALISE_CONSTITUCIONAL_v111.md:75` → `"Testes pytest para ambos os modos JWT"` | **Referenciado.** Não removido. |
| `zeno_veto (1).c` | 9 733 B | nenhuma referência ao nome `zeno_veto (1)` | **Substituído, não referenciado.** `diff` mostra ser uma revisão **anterior** de `zeno_veto.c`: usa `float`+globais (`g_zeno`, `g_coherence`) e `AQ2_MainLoop`, enquanto `zeno_veto.c` é a versão reentrante (`ZenoState *zeno`, `assert`, includes `coherence.h`) com hardening anotado Z1–Z6. |
| `dummy.rs` | 72 B | **`Cargo.toml:31-32`** → `[lib]` `path = "dummy.rs"` | **NÃO É LIXO — é referenciado pelo manifesto da raiz.** Provado empiricamente num workspace isolado: com `dummy.rs` presente `cargo check` → exit 0; removendo-o → **exit 101**, `"can't find lib 't' at path ...dummy.rs"`. O seu conteúdo é o seu propósito: `"// dummy — arkhe-safe-core-sdk lives at projects/arkhe-safe-core-sdk/"`. |
| `coherence.h` | 222 B | **`zeno_veto.c:14`**, **`observador_primordial.c:14`**, **`metatron_unitary_kernel.c:28`** — todos `#include "coherence.h"` | **NÃO É LIXO — é o cabeçalho de um subsistema C.** Não existe outro `coherence.h` no repositório, logo o `#include` relativo resolve para este. Declara `CoherenceState { phi_c, phi_delta, ratio, entropy }` — e `arkhe-monorepo/bloco_507/README.md:42` confirma que é esse o contrato REAL: `"coherence.h REAL tem apenas phi_c, phi_delta, ratio, entropy"`. Há ainda `arkhe-monorepo/bloco_517/metatron_observer_bridge.c:55` com o mesmo include. |

---

## Anexo C — Reprodução

```bash
# contagens
git ls-files '*Cargo.toml' | wc -l                                  # 151
ls -d crates/*/ | wc -l                                             # 3
ls -d crates/crates/*/ | wc -l                                      # 2
ls -d safe-core-monorepo/crates/*/ | wc -l                          # 62
ls -d arkhe-monorepo/packages/*/ | wc -l                            # 64

# qual cópia o Cargo usa
cargo metadata --manifest-path crates/Cargo.toml --no-deps --format-version 1

# as duas cópias são idênticas?
diff -rq crates/safe-core-crypto crates/crates/safe-core-crypto    # exit 0

# verificação que não regrediu
cd safe-core-monorepo && cargo check --workspace ; echo $?          # 0

# as provas Lean nomeadas existem?
git grep -ln 'compose_lipschitz\|precision_stability'               # vazio
git ls-files '*.lean' | wc -l                                       # 51

# hathor tem substrato?
git grep -il 'hathor'                                               # só ARKHE-Documento-Mestre.md
```
