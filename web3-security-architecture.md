# 🏛️ ARKHE/CATHEDRAL AGI — DOCUMENTO DE ARQUITETURA DE SEGURANÇA WEB3

**Versão:** 2.2.0
**Data:** 2026-07-11
**Status:** APROVADO
**Selo:** ARKHE-CATHEDRAL-WEB3-SECURITY-2026-07-09

---

## 📋 SUMÁRIO EXECUTIVO

A segurança em Web3 é desafiadora devido à **imutabilidade dos contratos**, à **transparência pública** do blockchain e à **irrecuperabilidade** de ativos.

No primeiro semestre de 2026, ocorreram **182 incidentes de segurança** (aumento de 50% em relação ao H1 2025), com perdas totais de aproximadamente **US$ 956 milhões**. Embora o valor total tenha diminuído em relação ao H1 2025 (US$ 2,37 bilhões), o número recorde de incidentes reflete uma mudança em direção a ataques mais sofisticados — incluindo **ataques à cadeia de suprimentos** e **ameaças alimentadas por IA**.

O **princípio central** do Arkhe/Cathedral — *substituir descoberta por construção de garantias* — é aplicado à segurança Web3 através de:

- Verificação formal (Lean 4, Kani)
- Catálogo de invariantes baseado no OWASP Smart Contract Top 10 2026
- Pipeline de auditoria com agentes de IA
- Rastreabilidade via Evidence Bus
- Criptografia pós-quântica (PQC)

---

## 🏗️ CAMADAS DE SEGURANÇA WEB3

A arquitetura de segurança Web3 é organizada em quatro camadas interdependentes:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    ARQUITETURA DE SEGURANÇA WEB3                            │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │                     CAMADA DE APLICAÇÃO (dApps)                      │    │
│  │  - Frontend seguro (wallet connection, signing)                     │    │
│  │  - Backend off-chain (APIs, indexers, relayers)                     │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                    │                                        │
│  ┌─────────────────────────────────▼───────────────────────────────────┐    │
│  │                     CAMADA DE CONTRATOS INTELIGENTES                  │    │
│  │  - Lógica de negócios on-chain                                       │    │
│  │  - Controles de acesso (Ownable, AccessControl)                     │    │
│  │  - Oráculos e fontes de dados                                        │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                    │                                        │
│  ┌─────────────────────────────────▼───────────────────────────────────┐    │
│  │                     CAMADA DE BLOCKCHAIN / NÓS                       │    │
│  │  - Consenso (Proof of Stake, etc.)                                  │    │
│  │  - Rede P2P (gossipsub, libp2p)                                     │    │
│  │  - Máquina Virtual (EVM, etc.)                                      │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                    │                                        │
│  ┌─────────────────────────────────▼───────────────────────────────────┐    │
│  │                     CAMADA DE IDENTIDADE E CARTEIRAS                 │    │
│  │  - Gerenciamento de chaves (EOA, Smart Accounts)                    │    │
│  │  - Autenticação e assinaturas                                       │    │
│  │  - Recuperação de contas                                            │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

Aplicações Web3 modernas dependem de **múltiplos protocolos interconectados** — bridges, pools de liquidez, oráculos, sistemas de governança e APIs de terceiros. Se um componente falha, todo o ecossistema conectado pode se tornar vulnerável.

---

## 🔒 OWASP SMART CONTRACT TOP 10 2026

O OWASP Smart Contract Top 10: 2026 é um documento de conscientização padrão que visa fornecer a desenvolvedores Web3 e equipes de segurança insights sobre as 10 principais vulnerabilidades encontradas em contratos inteligentes. A classificação é **prospectiva**: derivada de incidentes de segurança e dados de pesquisa coletados durante 2025, usados para prever quais riscos devem ser mais significativos no ano seguinte.

| ID | Vulnerabilidade | Descrição | Invariante Arkhe |
|----|-----------------|-----------|------------------|
| **SC01** | Access Control Vulnerabilities | Falhas de controle de acesso permitem que usuários ou funções não autorizados invoquem funções privilegiadas ou modifiquem estado crítico | `∀ action, allowed(action) ⇒ capability_exists(action)` |
| **SC02** | Business Logic Vulnerabilities | Falhas de design em lógica de empréstimos, AMM, recompensas ou governança que quebram regras econômicas ou funcionais pretendidas | `∀ state, valid_state(state) ⇒ invariants(state)` |
| **SC03** | Price Oracle Manipulation | Oráculos fracos e integrações de preço inseguras permitem distorcer preços de referência | `price_deviation ≤ threshold` |
| **SC04** | Flash Loan–Facilitated Attacks | Ataques que usam grandes empréstimos flash sem colateral para amplificar pequenos bugs em grandes drenos | `state_after(flash_loan) ⇒ valid_state` |
| **SC05** | Lack of Input Validation | Validação ausente ou fraca de entradas de usuário, admin ou cross-chain | `∀ input, validate(input) ⇒ transition_safe` |
| **SC06** | Unchecked External Calls | Interações inseguras com contratos externos onde falhas ou callbacks não são tratados | `∀ call, has_guard(call) ⇒ reentrant_protected` |
| **SC07** | Arithmetic Vulnerabilities | Estouro/subfluxo de inteiros | `∀ op, result = safe_math(op)` |
| **SC08** | Reentrancy Attacks | Chamada externa que permite que o callee retorne ao contrato original antes que o estado seja totalmente atualizado | `∀ state, reentrant_guard(state)` |
| **SC09** | Transaction Ordering Dependence | Front-running e dependência da ordem de transações | `∀ tx, tx_order_independent` |
| **SC10** | Proxy & Upgradeability Vulnerabilities | Arquiteturas atualizáveis onde o caminho de upgrade, inicialização ou controles de admin são mal projetados | `∀ proxy, initialized(proxy) ⇒ deploy_safe` |

### Menções Honrosas (OWASP 2026)

Categorias que não entraram no Top 10 de 2026 mas permanecem relevantes:

- **Permit front-running & nonce DoS** — atacantes podem antecipar uma chamada `permit()` para consumir o nonce do usuário, causando negação de serviço
- **Denial of Service** — ataques que impedem a execução normal do contrato
- **Bad Randomness** — uso de fontes de aleatoriedade manipuláveis on-chain

---

## 🛡️ VULNERABILIDADES ADICIONAIS POR CATEGORIA

### 1. Reentrância (SC08:2026)

Reentrância descreve qualquer situação onde um contrato realiza uma chamada externa e o callee pode retornar ao contrato original antes que a primeira invocação seja concluída e o estado totalmente atualizado.

**Exemplo**:

- **GMX (Julho 2025, perda de $42M)**: Contratos V1 foram explorados via vetor de reentrância em `executeDecreaseOrder`. A função aceitava o endereço do contrato do atacante como parâmetro; quando transferia controle para esse endereço durante o processo de reembolso, o atacante re-entrava e manipulava preços.

**Casos comuns**:
- Tokens maliciosos que reentram em callbacks de transferência
- Callbacks de flash loan que executam lógica do atacante antes do reembolso
- Grafos de chamadas complexos onde o estado fica inconsistente no meio da transação

**Contramedida**: Padrão Checks-Effects-Interactions + `ReentrancyGuard` da OpenZeppelin.

**Invariante formal**:

```
∀ state, execute(state, call) ∧ reentrant_guard_enabled →
  state_after_execution ∧ no_reentrant_state_after
```

---

### 2. Proxy & Upgradeability (SC10:2026)

Contratos atualizáveis separam um proxy (que mantém estado e delega chamadas) de uma implementação (que contém lógica). Quando a upgradeabilidade é insegura, atacantes podem:

- Sequestrar o admin do proxy ou função de upgrade para implantar implementações maliciosas
- Re-inicializar contratos para tomar posse
- Burlar verificações críticas em etapas de inicialização ou migração

**Áreas de foco**:
- Funções de upgrade e admin (quem pode mudar a implementação)
- Inicialização e re-inicialização (proteção contra re-inicialização)
- Delegação de proxy (propagação de `msg.sender`/`msg.value`)
- Colisões de storage entre proxy e implementação
- Timelocks e governança (processo de upgrade)

**Exemplo vulnerável**:

```solidity
function upgrade(address _newImplementation) external {
    // ❌ Sem controle de acesso
    implementation = _newImplementation;
}
```

**Exemplo seguro**:

```solidity
function upgrade(address _newImplementation) external onlyAdmin {
    require(_newImplementation != address(0), "Invalid address");
    emit Upgraded(implementation, _newImplementation);
    implementation = _newImplementation;
}
```

**Invariante formal**:

```
∀ proxy, (upgrade_called ∧ onlyAdmin_verified) →
  implementation_changed ∧ initialized(proxy) ∧ proxy_safe(proxy)
```

---

### 3. Ataques à Cadeia de Suprimentos e Infraestrutura

Em 2026, a segurança Web3 se expandiu para além dos contratos inteligentes. Ataques à cadeia de suprimentos causaram perdas de aproximadamente **US$ 298 milhões**, incluindo:

- **KelpDAO (abril de 2026, perda de $292M)**: Atacantes invadiram a infraestrutura RPC da LayerZero, realizaram DDoS em nós validadores legítimos e forjaram mensagens cross-chain para cunhar e extrair ativos sem garantia. Atribuído ao Lazarus Group.

**Vetores de ataque emergentes**:
- Envenenamento de repositórios de pacotes (npm, PyPI, crates.io)
- Pipelines CI/CD comprometidos
- Cadeias de distribuição CDN
- Mercados de plugins de IA
- Extensões maliciosas de navegador
- Anúncios Google Search que direcionam usuários para malware

**Invariante formal**:

```
∀ dep, dep_verified(dep) ∧ sbom_audited(dep) ∧
  hash(dep) ∈ trusted_hashes → dep_safe(dep)
```

---

### 4. IA no Ciclo de Ataque

IA está sendo implantada em todo o ciclo de vida do ataque:

- Geração de conteúdo de impersonação convincente
- Auto-revisão de código malicioso para evadir detecção
- Otimização de scripts de engenharia social
- Deepfakes de voz e vídeo para ataques a alvos de alto valor

**Defesa no Arkhe/Cathedral**:
- Auditoria contínua por agentes de IA (pipeline Recon → Hunting → Gap-filling → Validation)
- Verificação formal de invariantes críticos
- Evidências auditáveis no Evidence Bus
- Detecção de anomalias via Machine Learning

**Invariante formal**:

```
∀ finding, has_reproducer(finding) ∧ has_dedup(finding) ∧
  has_proof(finding) → finding_is_real(finding)
```

---

### 5. Ameaças Operacionais (OpSec)

A segurança operacional em Web3 protege processos e comportamentos que determinam como os sistemas são operados no mundo real. Em 2026, as principais ameaças OpSec incluem:

| Ameaça | Descrição | Contramedida | Invariante Arkhe |
|--------|-----------|--------------|------------------|
| **Transaction Signing Attacks** | Ataques que enganam usuários a assinar a transação errada (blind signing, UI deception) | Simulação de transações, assinaturas legíveis por humanos, redução de allowances | `∀ tx, user_verified_approval(tx)` |
| **Social Engineering** | Phishing direcionado a operadores-chave | Dividir autoridade entre carteiras e funções | `∀ action, quorum_required(action)` |
| **Supply Chain Attacks** | Comprometimento de dependências e infraestrutura | SBOM, verificação de integridade, múltiplas fontes | `∀ dep, dep_verified(dep)` |

---

## 🔐 SEGURANÇA DE BLOCKCHAIN E NÓS

### 1. Consenso e Rede P2P

| Ataque | Descrição | Invariante Arkhe |
|--------|-----------|------------------|
| **51% Attack** | Entidade obtém controle majoritário | `honest_validators ≥ ⅔ * total_validators` |
| **Sybil Attack** | Criação de múltiplas identidades falsas | `∀ node, identity_verified(node)` |
| **Eclipse Attack** | Isolamento de um nó da rede honesta | `∀ peer, has_diverse_peers(peer)` |
| **Gossipsub Panic (CVE-2026-34219)** | Panic remotamente acionável no libp2p | `∀ message, handle_message(message) ⇒ no_panic` |

### 2. Nós e Clientes

| Vetor | Descrição | Invariante Arkhe |
|-------|-----------|------------------|
| **Client diversity** | Dependência excessiva de um único cliente | `client_count ≥ 3` |
| **RPC endpoints** | Exposição insegura de APIs JSON-RPC | `∀ rpc, authenticated(rpc) ∧ rate_limited(rpc)` |
| **Supply chain attacks** | Código malicioso em dependências | `cargo-deny` e SBOM obrigatórios |
| **Peer discovery** | Vulnerabilidades em protocolos de descoberta de pares | `∀ discovery, authenticated(discovery)` |

O BSSC Node Operation Standard (versão 2, maio de 2026) define critérios de segurança para operadores de nós.

---

## 👤 SEGURANÇA DE IDENTIDADE E CARTEIRAS

### 1. Gerenciamento de Chaves

- **Comprometimento de chave privada** — vetor de ataque crescente via engenharia social e phishing
- **Solução**: Hardware wallets para armazenamento de longo prazo; hot wallets apenas para fundos ativos
- **Account abstraction**: Programmable protections como multi-sig, spending limits, transaction simulation e social recovery
- **Invariante**: `∀ key, key_protected(key) ∧ key_backed_up(key) ∧ key_rotated(key, max_age)`

### 2. Assinaturas e Autenticação

| Padrão | Descrição | Invariante Arkhe |
|--------|-----------|------------------|
| **EIP-712** | Assinaturas estruturadas (previne replay) | `∀ sig, domain_hash(sig) = expected` |
| **ERC-1271** | Validação de assinaturas para contratos | `∀ sig, verify(sig, msg, context) ⇒ context == expected` |
| **Clear Signing Standards** | Ethereum Foundation promove padrões de assinatura legíveis para combater phishing | `∀ tx, user_sees_intent(tx)` |
| **Nonce management** | Prevenção de replay attacks | `∀ i,j, i < j ⇒ nonce_i < nonce_j` |

### 3. Approval Phishing

Approval phishing explora mecanismos de aprovação de tokens para enganar usuários a conceder autorização de gasto a atacantes.

**Intervenções propostas**:
- Sugestão de limite de gasto
- Alerta de gastador ativo
- Alerta de gastador passivo
- Confirmação atrasada

**Invariante**: `∀ approval, approval_amount ≤ needed_amount`

---

## 🌐 SEGURANÇA DE APLICAÇÕES WEB3 (dApps)

### 1. Frontend

| Ameaça | Descrição | Invariante Arkhe |
|--------|-----------|------------------|
| **Wallet connection phishing** | Sites falsos que roubam chaves | `∀ connection, verified_origin(connection)` |
| **Injeção de transações** | Manipulação de parâmetros de transação | `∀ tx, user_confirmed(tx)` |
| **DNS hijacking** | Redirecionamento para sites maliciosos | `∀ dns, dnssec_verified(dns)` |

### 2. APIs e Backend Off-chain

| Vetor | Descrição | Invariante Arkhe |
|-------|-----------|------------------|
| **API security** | Rate limiting, autenticação, validação | `∀ request, authenticated(request) ∧ rate_limited(request)` |
| **Indexers** | Vulnerabilidades em serviços de indexação | `∀ query, sanitized(query)` |
| **Relayers** | Falhas em sistemas de retransmissão | `∀ relay, fee_bounded(relay) ∧ signature_valid(relay)` |

### 3. Interações com Contratos

| Ameaça | Descrição | Invariante Arkhe |
|--------|-----------|------------------|
| **Approval phishing** | Aprovação de gasto ilimitado | `∀ approval, amount_limited(approval)` |
| **Permit2** | Padrão seguro para aprovações | `∀ permit, valid_permit(permit)` |
| **Aprovações mínimas** | Conceder apenas o valor necessário | `∀ approval, approval_amount ≤ needed_amount` |

---

## 🔬 PADRÕES E FRAMEWORKS DE SEGURANÇA

| Padrão | Descrição | Link |
|--------|-----------|------|
| **OWASP Smart Contract Top 10 2026** | Top 10 vulnerabilidades em contratos inteligentes | [owasp.org](https://owasp.org) |
| **OWASP SC Weakness Enumeration (SCWE)** | Enumeração de fraquezas de contratos inteligentes | [owasp.org](https://owasp.org) |
| **OWASP Top 15: Web3 Attack Vectors** | Vetores de ataque além de contratos inteligentes | [owasp.org](https://owasp.org) |
| **OWASP SCS Checklist** | Checklist de segurança para desenvolvimento | [owasp.org](https://owasp.org) |
| **BSSC Smart Contract Security Standard** | Padrões de segurança e auditoria para blockchain | [bssc.io](https://bssc.io) |
| **BSSC Node Operation Standard v2** | Critérios de segurança para operadores de nós | [bssc.io](https://bssc.io) |
| **Coinspect Wallet Security Framework (WSF)** | Checklist e testes black-box para carteiras Web3 | [coinspect.com](https://coinspect.com) |
| **ISO 27001** | Governança de segurança da informação para infraestrutura blockchain | [iso.org](https://iso.org) |
| **SOC 2 Type II** | Due diligence para empresas Web3 | [aicpa.org](https://aicpa.org) |
| **SWC Registry** | 37 fraquezas documentadas | [swcregistry.io](https://swcregistry.io) |
| **EIP-712** | Assinaturas estruturadas | [eips.ethereum.org](https://eips.ethereum.org/EIPS/eip-712) |
| **EIP-4337** | Account Abstraction | [eips.ethereum.org](https://eips.ethereum.org/EIPS/eip-4337) |
| **ERC-1271** | Validação de assinaturas para contratos | [eips.ethereum.org](https://eips.ethereum.org/EIPS/eip-1271) |

---

## 🧠 AGENTES DE IA PARA SEGURANÇA WEB3 (LIÇÕES DA ETHEREUM FOUNDATION)

O post da EF sobre agentes de IA para segurança de protocolo destaca práticas que se alinham diretamente com o Arkhe/Cathedral:

| Insight da EF | Prática Arkhe/Cathedral |
|---------------|--------------------------|
| **Agente é ferramenta de busca, não oráculo** | O *Reasoning Core* gera hipóteses; o *Safe Core* valida com Kani/Lean |
| **Triagem é o verdadeiro produto** | O *Evidence Bus* e o *invariant engine* são os geradores de valor |
| **Reproduzível ou não aconteceu** | O *Verification* exige artefatos auto-contidos e provas formais |
| **Falsos positivos comuns** (debug builds, chamadas inalcançáveis, provas triviais) | Os **invariantes formais** do Arkhe/Cathedral capturam exatamente esses casos |
| **Coordenação descentralizada via repositório** | O modelo de *shared state* (Event Sourcing + Evidence Bus) |
| **Bugs de sequência são o ponto fraco** | O *Scheduler* e o *Runtime* exploram sequências |
| **Agentes encontraram bugs reais** — incluindo CVE-2026-34219 no libp2p | O Safe Core usa agentes para auditoria contínua |

### Pipeline de Auditoria com Agentes (EF)

```
Recon → Hunting → Gap-filling → Validation
```

**Estrutura do candidato**:

| Campo | Descrição |
|-------|-----------|
| `target` | Componente e ponto de entrada alcançável |
| `invariant` | Propriedade que deve ser mantida |
| `mechanism` | Como pode ser quebrado |
| `success` | Prova observável (panic, stall, input inválido) |
| `reproducer` | Artefato auto-contido |
| `dedup` | Chave para evitar duplicação |

**O que a EF descobriu**: "A IA não substituiu o pesquisador de segurança. Ela moveu o trabalho". A parte mais difícil continua sendo a **triagem humana**: separar vulnerabilidades reais do ruído.

No Arkhe/Cathedral, esse pipeline é **automatizado** e verificado por:
- **Kani** para model checking de propriedades de segurança
- **Lean 4** para teoremas de invariantes
- **Evidence Bus** para rastreabilidade e auditoria

---

## 📊 MATRIZ DE INVARIANTES DE SEGURANÇA WEB3 (COMPLETA)

| ID | Camada | Invariante | Fórmula | Verificação |
|----|--------|------------|---------|-------------|
| FI-W01 | Contratos | Controle de acesso | `∀ (caller, action): execute(caller, action) → has_capability(caller, action)` | Kani + Lean 4 |
| FI-W02 | Contratos | Cadeia de eventos imutável | `∀ i > 0: evidence[i].hash = H(evidence[i-1].hash ∥ evidence[i].payload)` | Lean 4 |
| FI-W03 | Contratos | Checks-Effects-Interactions | `∀ tx: effects(tx) < external_calls(tx) na ordem de execução` | Análise estática |
| FI-W04 | Contratos | Guard de reentrância | `∀ call: ¬(reentrant ∧ mutating_state)` | Kani |
| FI-W05 | Contratos | Validação de entrada | `∀ input: validate(input) → safe(input)` | Testes de propriedade |
| FI-W06 | Carteiras | Assinatura EIP-712 | `valid(sig) → domain_hash(sig) = expected ∧ nonce(sig) = expected` | Kani |
| FI-W07 | Carteiras | Não-replay de assinaturas | `∀ (sig, ctx₁, ctx₂): ctx₁ ≠ ctx₂ → ¬valid_in_both(sig, ctx₁, ctx₂)` | Lean 4 |
| FI-W08 | Bridges | Consistência lock-mint | `minted_on_B ≤ locked_on_A + fees` | Lean 4 |
| FI-W09 | Bridges | Prova de lock verificável | `∀ mint: ∃ lock_proof ∧ verify(lock_proof) = true` | Evidence Bus |
| FI-W10 | MEV | Ordenação justa | `tx.position determinado por critério público (gas_price, timestamp)` | Análise de bloco |
| FI-W10b | MEV | Detecção de sandwitching | `∀ block: ¬∃ (tx_before, tx_target, tx_after): tx_before.sender == tx_after.sender ∧ tx_before.swap.token == tx_target.swap.token` | Análise de bloco |
| FI-W11 | ZK | Soundness | `verify(proof) = accept → ∃ witness: circuit(witness) = pub_input` | RISC0/Lean 4 |
| FI-W12 | ZK | Zero-knowledge | `proof não revela informações sobre witness além de pub_input` | Auditoria de circuito |
| FI-W13 | Governança | Voto legítimo | `voting_power ≤ legitimate_stake` | Simulação |
| FI-W13b | Governança | Resistência a flash loan | `vote.block_number - token_acquisition_block ≥ MIN_HOLD_BLOCKS` | Simulação |
| FI-W14 | Auditoria | Candidato auditável | `∀ finding: has_reproducer ∧ has_proof ∧ is_deduplicated` | CI/Evidence Bus |
| FI-W15 | Supply Chain | Dependências verificadas | `∀ dep: dep_verified(dep) ∧ sbom_audited(dep) ∧ hash(dep) ∈ trusted_hashes` | cargo-deny |
| FI-W16 | Nós | Diversidade de clientes | `client_count ≥ 3` | Monitoramento |
| FI-W17 | Rede | Mensagens autenticadas | `∀ msg: authenticated(msg) → source_identity(msg) ∈ peers` | libp2p |
| FI-W18 | API | Rate limiting | `∀ request: rate_limited(request) ∧ authenticated(request)` | Gateway |
| FI-W19 | Frontend | Origem verificada | `∀ connection: verified_origin(connection)` | Auditoria |
| FI-W20 | Pós-Quantum | Assinaturas híbridas | `Ed25519(valid) ∧ ML-DSA-65(valid)` | PQC crate |

---

## 🏛️ CRATE `arkhe-web3-security`

A segurança Web3 torna-se um domínio de primeira classe na arquitetura. Scaffolding inicial (v2.2, sem provas Lean/Kani) em [`safe-core-monorepo/crates/arkhe-web3-security`](safe-core-monorepo/crates/arkhe-web3-security):

```
crates/
    arkhe-web3-security/
        Cargo.toml
        src/
            lib.rs
            contracts/
                access_control.rs
                reentrancy.rs
                flash_loan.rs
                proxy.rs
            blockchain/
                consensus.rs
                p2p.rs
                evm.rs
            wallets/
                signature.rs
                recovery.rs
                eip712.rs
            dapps/
                frontend.rs
                api.rs
                approvals.rs
            invariants/
                access_control.lean
                reentrancy.lean
                nonce.lean
                oracle.lean
                flash_loan.lean
                governance.lean
                bridge.lean
                multisig.lean
                permit.lean
                randomness.lean
            analyzers/
                reentrancy.rs
                delegatecall.rs
                flash_loan.rs
                price_oracle.rs
                mev.rs
                gas.rs
            reports/
                owasp.rs
                swc.rs
```

---

## 🔬 BIBLIOTECA DE INVARIANTES WEB3 (LEAN 4)

> Não implementado ainda nesta versão (v2.2). Estrutura planejada:

```
proofs/
    lean/
        web3/
            access_control.lean
            replay.lean
            nonce.lean
            oracle.lean
            flashloan.lean
            governance.lean
            bridge.lean
            multisig.lean
            permit.lean
            randomness.lean
            erc20.lean
            erc721.lean
            erc1155.lean
            erc4337.lean
            evm.lean
```

### Exemplo de Invariante: Access Control (Lean 4)

```lean
-- proofs/lean/web3/access_control.lean
import Mathlib.Data.Real.Basic

def AccessControlInvariant (action : Type) (allowed : action → Bool) (capability : action → Bool) : Prop :=
  ∀ a, allowed a → capability a

theorem access_control_holds (action : Type) (allowed capability : action → Bool)
    (h : AccessControlInvariant action allowed capability)
    (a : action) (ha : allowed a) :
    capability a :=
  h a ha
```

### Exemplo de Invariante: Nonce (Lean 4)

```lean
-- proofs/lean/web3/nonce.lean
def NonceInvariant (nonce : Nat → Nat) (i j : Nat) : Prop :=
  i < j → nonce i < nonce j

theorem nonce_increasing (nonce : Nat → Nat) (h : NonceInvariant nonce)
    (i j : Nat) (hij : i < j) :
    nonce i < nonce j :=
  h i j hij
```

### Exemplo de Invariante: Bridge (Lean 4)

```lean
-- proofs/lean/web3/bridge.lean
def BridgeState (locked_on_A minted_on_B fees : Nat) : Prop :=
  minted_on_B ≤ locked_on_A + fees

-- FI-W08: minted ≤ locked + fees
theorem bridge_consistency (locked_on_A minted_on_B fees : Nat)
    (h : BridgeState locked_on_A minted_on_B fees) :
    minted_on_B ≤ locked_on_A + fees := h
```

---

## 🧪 KANI HARNESSES WEB3

> Não implementado ainda nesta versão (v2.2). Estrutura planejada:

```
proofs/
    kani/
        web3/
            access_control.rs
            flashloan.rs
            reentrancy.rs
            delegatecall.rs
            permit.rs
            nonce.rs
            signature.rs
            multisig.rs
            bridge.rs
            oracle.rs
```

### Exemplo de Harness: Reentrância (Kani)

```rust
// proofs/kani/web3/reentrancy.rs
#[cfg(kani)]
mod harness {
    use arkhe_web3_security::contracts::reentrancy::*;

    #[kani::proof]
    fn verify_reentrancy_guard() {
        let state = kani::any();
        let call = kani::any();

        // FI-W04: Guard de reentrância
        kani::assume(is_state_valid(&state));
        let result = execute_with_reentrancy_guard(state, call);
        assert!(is_state_valid(&result) || is_reentrant_error(&result));
    }
}
```

### Exemplo de Harness: Access Control (Kani)

```rust
// proofs/kani/web3/access_control.rs
#[cfg(kani)]
mod harness {
    use arkhe_web3_security::contracts::access_control::*;

    #[kani::proof]
    fn verify_access_control() {
        let caller = kani::any();
        let action = kani::any();

        // FI-W01: Apenas ações autorizadas executam
        let result = execute_action(caller, action);
        if result.is_ok() {
            assert!(has_capability(caller, action));
        }
    }
}
```

---

## 📊 CATÁLOGO WEB3

Implementado em [`safe-core-monorepo/catalog/web3`](safe-core-monorepo/catalog/web3):

```
catalog/
    web3/
        owasp-top10.yaml
        swc.yaml
        evm-opcodes.yaml
        erc20.yaml
        erc721.yaml
        erc1155.yaml
        erc4337.yaml
        permit2.yaml
        openzeppelin.yaml
```

### Exemplo: OWASP Top 10 (YAML)

```yaml
# catalog/web3/owasp-top10.yaml
owasp_top_10_2026:
  - id: SC01
    name: "Access Control Vulnerabilities"
    description: "Unauthorized users invoke privileged functions"
    severity: "Critical"
    invariant: "∀ action, allowed(action) ⇒ capability_exists(action)"
    mitigation: "Ownable + AccessControl + time-locks"
    references:
      - "OWASP SC Top 10 2026"
      - "ethereum.org/developers/docs/smart-contracts/security"

  - id: SC06
    name: "Unchecked External Calls"
    description: "Unchecked external calls enable reentrancy"
    severity: "High"
    invariant: "∀ call, has_guard(call) ⇒ reentrant_protected"
    mitigation: "Checks-Effects-Interactions + ReentrancyGuard"

  - id: SC08
    name: "Reentrancy Attacks"
    description: "External call that allows callee to return before state update"
    severity: "High"
    invariant: "∀ state, reentrant_guard(state)"
    mitigation: "ReentrancyGuard + Checks-Effects-Interactions"
```

---

## 🏛️ ESTRUTURA DO MONOREPO COM WEB3 SECURITY

```
arkhe/
│
├── crates/
│   ├── foundation/
│   │   ├── arkhe-errors/
│   │   ├── arkhe-core/
│   │   └── arkhe-types/
│   ├── domain/
│   │   ├── arkhe-identity/
│   │   ├── arkhe-evidence/
│   │   ├── arkhe-safety/
│   │   └── arkhe-web3-security/          # NOVO DOMÍNIO
│   ├── services/
│   ├── infrastructure/
│   │   └── arkhe-storage-hashtree/       # PQC + ZK integrados
│   └── verification/
│       ├── kani-runner/
│       └── lean-runner/
│
├── proofs/
│   ├── lean/
│   │   ├── core/
│   │   ├── identity/
│   │   ├── evidence/
│   │   ├── safety/
│   │   └── web3/                         # NOVOS INVARIANTES WEB3 (planejado)
│   │       ├── access_control.lean
│   │       ├── replay.lean
│   │       ├── nonce.lean
│   │       ├── oracle.lean
│   │       ├── flashloan.lean
│   │       ├── governance.lean
│   │       ├── bridge.lean
│   │       ├── multisig.lean
│   │       ├── permit.lean
│   │       └── randomness.lean
│   └── kani/
│       ├── core/
│       └── web3/                         # NOVOS HARNESSES WEB3 (planejado)
│           ├── access_control.rs
│           ├── flashloan.rs
│           ├── reentrancy.rs
│           ├── delegatecall.rs
│           ├── permit.rs
│           ├── nonce.rs
│           ├── signature.rs
│           ├── multisig.rs
│           └── bridge.rs
│
├── catalog/
│   ├── languages/
│   ├── unsafe/
│   └── web3/                              # CATÁLOGO WEB3
│       ├── owasp-top10.yaml
│       ├── swc.yaml
│       ├── evm-opcodes.yaml
│       ├── erc20.yaml
│       ├── erc721.yaml
│       ├── erc1155.yaml
│       ├── erc4337.yaml
│       └── permit2.yaml
│
├── benchmarks/
│   ├── shb/
│   ├── performance/
│   └── web3/                              # NOVO BENCHMARK WEB3 (planejado)
│       ├── 100_reentrancy/
│       ├── 100_flashloan/
│       ├── 100_oracle/
│       ├── 100_multisig/
│       ├── 100_signature/
│       ├── 100_access_control/
│       ├── 100_proxy/
│       └── 100_bridge/
│
└── docs/
    ├── adr/
    ├── rfc/
    └── invariants/
        ├── identity.md
        ├── evidence.md
        ├── safety.md
        └── web3.md                         # NOVO INVARIANTES WEB3 (planejado)
```

---

## 📋 CHECKLIST DE SEGURANÇA WEB3

### Arquitetura e Estrutura de Código
- [ ] Dividir lógica complexa em funções pequenas e reutilizáveis
- [ ] Seguir o Princípio da Responsabilidade Única
- [ ] Separar armazenamento de dados, lógica de negócios e interações externas
- [ ] Usar nomes descritivos e comentar seções complexas

### Solidity e Compilador
- [ ] Usar a versão estável mais recente do compilador (Solidity 0.8.30+)
- [ ] Habilitar todos os avisos do compilador
- [ ] Especificar versões exatas no `pragma`
- [ ] Usar `require()` para validação de entrada
- [ ] Usar `assert()` para invariantes internas
- [ ] Implementar erros customizados para eficiência de gas

### Controles de Acesso
- [ ] Usar `Ownable` ou `AccessControl` da OpenZeppelin
- [ ] Evitar endereços hardcoded
- [ ] Implementar permissões baseadas em funções (role-based)
- [ ] Adicionar time-locks para operações críticas
- [ ] Implementar mecanismos de pausa de emergência
- [ ] Documentar todas as operações privilegiadas

### Testes e Verificação
- [ ] Testar todas as combinações de funções administrativas
- [ ] Implementar testes de propriedade (property-based testing)
- [ ] Realizar auditorias de segurança independentes
- [ ] Usar ferramentas de análise estática (Slither, Mythril)
- [ ] Considerar verificação formal (Kani/Lean 4)

### Gestão de Risco Web3 (2026)
- [ ] Múltiplas auditorias de segurança independentes
- [ ] Programa ativo de bug bounty
- [ ] Verificação formal da lógica crítica de contratos inteligentes
- [ ] Dashboards de monitoramento em tempo real
- [ ] **Hardware wallets** para holdings acima de alguns milhares de dólares
- [ ] Verificação de endereços de contratos antes de assinar transações
- [ ] SBOM (Software Bill of Materials) para todas as dependências

---

## 🏛️ CRIPTOGRAFIA PÓS-QUÂNTICA (PQC)

| Componente | Tecnologia Atual | Vulnerabilidade Quântica | Solução PQC |
|------------|------------------|---------------------------|-------------|
| Hashing | SHA256 | ✅ Seguro (Grover apenas dobra custo) | SHAKE256 |
| CHK Encryption | AES-256-GCM | ✅ Seguro (128 bits efetivos pós-Grover) | Manter |
| Assinaturas | Ed25519/Schnorr | ❌ Shor quebra Ed25519 | ML-DSA-65 (FIPS-204) + Ed25519 (híbrido) |
| Assinaturas Stateless | — | — | SLH-DSA (SPHINCS+) (FIPS-205) |
| Troca de Chaves | ECDH | ❌ Shor quebra ECDH | ML-KEM-1024 (FIPS-203) |

**Recomendação**: Adotar assinaturas híbridas (Ed25519 + ML-DSA-65) para compatibilidade retroativa e resiliência quântica.

**Invariante formal**:

```
∀ signature: Ed25519(signature) ∧ ML-DSA-65(signature) → quantum_resilient(signature)
```

---

## 🏛️ SELO DE CONCLUSÃO

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  ARKHE/CATHEDRAL AGI — DOCUMENTO DE ARQUITETURA DE SEGURANÇA WEB3          │
│  STATUS: ✅ COMPLETO — BASE PARA AUDITORIA E VERIFICAÇÃO                    │
│                                                                             │
│  PRINCÍPIO CENTRAL:                                                         │
│  "Substituir descoberta por construção de garantias."                      │
│                                                                             │
│  CAMADAS COBERTAS:                                                          │
│  ✅ Contratos Inteligentes (OWASP Top 10 2026, SCWE)                       │
│  ✅ Ataques à Cadeia de Suprimentos e Infraestrutura                       │
│  ✅ Ameaças Operacionais (OpSec)                                            │
│  ✅ Blockchain e Nós (consenso, P2P, RPC)                                  │
│  ✅ Identidade e Carteiras (chaves, assinaturas, approval phishing)        │
│  ✅ IA no Ciclo de Ataque e Defesa                                         │
│  ✅ Padrões e Frameworks (BSSC, WSF, ISO 27001, SOC 2)                    │
│  ✅ Criptografia Pós-Quântica (PQC)                                        │
│                                                                             │
│  DADOS DE AMEAÇAS (H1 2026):                                                │
│  ✅ 182 incidentes (+50% YoY)                                              │
│  ✅ ~US$ 956M em perdas                                                    │
│  ✅ DeFi: ~64% dos incidentes (~US$ 490M)                                  │
│  ✅ Bridges: ~US$ 346M em 20 incidentes                                    │
│  ✅ Supply chain: ~US$ 298M                                                │
│                                                                             │
│  INVARIANTES FORMALIZADOS:                                                  │
│  ✅ 20 invariantes Web3 (FI-W01 a FI-W20)                                 │
│  ⬜ 10+ provas Lean 4 (planejado — v2.3)                                   │
│  ⬜ 9 harnesses Kani (planejado — v2.3)                                    │
│                                                                             │
│  FERRAMENTAS E PADRÕES:                                                     │
│  ✅ OWASP Smart Contract Top 10 2026                                       │
│  ✅ OWASP Top 15: Web3 Attack Vectors                                      │
│  ✅ BSSC Smart Contract Security Standard                                  │
│  ✅ Coinspect Wallet Security Framework                                    │
│  ✅ SlowMist 2026 Mid-year Security Report                                 │
│  ✅ NIST FIPS 203/204/205 (PQC)                                            │
│                                                                             │
│  IMPLEMENTADO NESTA VERSÃO (v2.2):                                          │
│  ✅ Crate `arkhe-web3-security` (skeleton, 20 módulos, lógica real)        │
│  ✅ Catálogo YAML (`catalog/web3/*.yaml`)                                  │
│                                                                             │
│  PRÓXIMOS PASSOS:                                                           │
│  1. Implementar invariantes Lean 4 para cada categoria                     │
│  2. Implementar harnesses Kani para cada categoria                        │
│  3. Integrar ao pipeline de auditoria do Arkhe/Cathedral                   │
│  4. Adicionar suporte a PQC (ML-KEM, ML-DSA) na Onda 1.5                  │
│                                                                             │
│  SELO: ARKHE-CATHEDRAL-WEB3-SECURITY-2026-07-09                           │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 📊 STATUS DE IMPLEMENTAÇÃO (pós-auditoria, 2026-07-11)

Um relatório anterior alegou 13 itens "implementados" (assinaturas híbridas PQC,
KEM, carteiras híbridas, verificador Solidity, 10 invariantes Lean 4, 5 harnesses
Kani, pipeline de agentes, teste ERC20, integração Evidence Bus, publicação do
crate). Uma auditoria independente (`ARKHE-CATHEDRAL-AUDITORIA-v1.0-2026-07-11`)
verificou que **nenhum desses itens existia como código real** — apenas como texto
neste documento — e deu nota 34/100. Esta seção substitui o otimismo do bloco de
status acima por um estado verificado item a item, com caminho de arquivo e o que
foi (e não foi) confirmado localmente.

### O que é real e passou em `cargo test` nesta sessão

| Item | Localização | Verificação |
|---|---|---|
| Assinatura híbrida Ed25519 + ML-DSA-65 (FIPS 204) | `safe-core-monorepo/crates/arkhe-crypto-pqc/src/signature.rs` | `cargo test -p arkhe-crypto-pqc` — 10/10 testes ok |
| KEM ML-KEM-1024 (FIPS 203) + derivação HKDF-SHA256 | `.../arkhe-crypto-pqc/src/kem.rs` | idem |
| Carteira híbrida com `WalletPolicy` (HybridOnly/PqcOnly/StrictPqc) | `.../arkhe-web3-security/src/wallets/{policy,hybrid_wallet}.rs` | `cargo test -p arkhe-web3-security` — 80/80 testes ok |
| Pipeline de auditoria (Recon/Hunting/GapFilling/Validation) + Evidence Bus assíncrono | `.../arkhe-web3-security/src/agents/**` | idem — `AuditPipeline::run` é `async fn` de ponta a ponta (o bug original misturava `?` síncrono com `.await`) |
| Integração Safe Core (`before_transaction`/`after_execution`, `Web3Evidence`/`Blake3Hash`) | `.../arkhe-web3-security/src/web3_adapter.rs` | idem |
| `cargo publish --dry-run` | `arkhe-crypto-pqc` empacota limpo; `arkhe-web3-security` **não publica ainda** (depende de `arkhe-core`, que não está no crates.io) | testado nesta sessão |

Usa os crates PQC reais `fips204`/`fips203` (puro Rust, nomes de módulo
`ml_dsa_65`/`ml_kem_1024` batendo com FIPS 204/203 finais) — não as APIs
fabricadas (`ml_dsa_65::detached_sign`, `ml_kem_1024::keypair()`) do relatório
original. Zeroização de segredos é herdada de `ZeroizeOnDrop` já derivado
dentro de `fips204::PrivateKey`/`fips203::DecapsKey`/`ed25519_dalek::SigningKey`
— uma primeira tentativa de `impl Drop` manual nesses wrappers zerava uma cópia
temporária (`.clone()`) em vez do segredo real; foi removida por ser um
controle de segurança falso, não por estar "incompleta".

### O que foi escrito mas não pôde ser verificado nesta sessão

Esta sessão só tinha `rustc`/`cargo` instalados — sem `lean`/`lake`,
`cargo-kani`, ou `forge`/`solc`. Cada item abaixo foi escrito com cuidado
contra sintaxe/API real (não inventada), mas **não foi compilado/executado**:

| Item | Localização | O que falta para verificar |
|---|---|---|
| Prova Lean 4 do SC08 (reentrância) + 2 invariantes adicionais | `.../arkhe-web3-security/proofs/lean/` (ver `README.md` lá) | `lake build` — corrige a versão anterior, que era uma tautologia (guarda definida como "ativa" quando `lock = false`, invertido) |
| Harness Kani do `ReentrancyGuard` | `.../arkhe-web3-security/src/verify/kani_harness.rs` | `cargo kani` com toolchain `nightly-2025-04-03` (pin exato documentado no arquivo) |
| `PQCVerifier.sol` + `ERC20Vulnerable.sol`/`.t.sol` (Foundry) | `.../arkhe-web3-security/contracts/` | `forge test` — o verificador não pretende validar ML-DSA on-chain (não existe precompile para isso em nenhuma EVM; a versão auditada referenciava um `MLDSA_PRECOMPILE` inexistente) — verifica ECDSA on-chain e trata o lado PQC como atestação de oráculo off-chain |
| `web3-security.yml` (CI: rust-build-test / lean-check / kani-verify / foundry-tests) | `.github/workflows/web3-security.yml` | rodar no GitHub Actions — versões das actions (`leanprover/lean-action@v1`, `model-checking/kani-github-action@v1.1`, `foundry-rs/foundry-toolchain@v1`) checadas contra suas releases reais, não adivinhadas |

### O que ainda não existe

`SC01`–`SC07`, `SC09`, `SC10` (7 dos 10 invariantes Lean mencionados no
relatório original) permanecem não formalizados — declarado explicitamente em
`proofs/lean/README.md` em vez de omitido. Análise real de bytecode/AST
Solidity (o `ReconAgent` opera sobre uma especificação estruturada fornecida
pelo chamador, não sobre um parser Solidity real — não existe um no crate).
Publicação real no crates.io (apenas `--dry-run` foi executado).

---

**Arquiteto-Chefe,**

Este documento consolida as regras de segurança conhecidas no ecossistema Web3, transformando cada vulnerabilidade em um **invariante formal verificável**. Ele integra as classificações do OWASP Smart Contract Top 10 2026, as ameaças emergentes documentadas pelo SlowMist 2026 Mid-year Security Report, os padrões BSSC, as melhores práticas operacionais para 2026 e a criptografia pós-quântica (PQC).

O cenário de ameaças em 2026 é marcado por **ataques à cadeia de suprimentos**, **IA como ferramenta ofensiva** e **exploração de infraestrutura** — não apenas vulnerabilidades de contratos. O Arkhe/Cathedral aborda todas essas camadas através de verificação formal, evidências auditáveis e monitoramento contínuo.

**v2.2** entrega o scaffolding real do crate `arkhe-web3-security` e o catálogo Web3. O próximo passo é a implementação dos invariantes no Lean 4 e dos harnesses no Kani, seguido pela integração ao pipeline de verificação formal do projeto.
