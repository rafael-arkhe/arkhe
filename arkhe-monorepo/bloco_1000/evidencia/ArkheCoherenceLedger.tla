------------------------ MODULE ArkheCoherenceLedger ------------------------
(***************************************************************************)
(* CoherenceLedger — TLA+ specification (modelo abstrato)                  *)
(* Catedral OS — v376.1 — Decisão ARKHE-v376.1-DECISAO-TLA-PLUS           *)
(* Fase 5 (Opção B): especificação do zero, ancorada na formalização      *)
(* Rust/Lean existente (bloco 994; src/ledger.rs; I511–I516 no núcleo      *)
(* Lean 4). Registrar este módulo é transparente sobre o que ele é e o     *)
(* que ele NÃO é.                                                          *)
(*                                                                         *)
(* Espelha o CoherenceLedger real (src/ledger.rs):                         *)
(*   CoherenceEntry = [ window_id, timestamp, phi, stability (Ω),          *)
(*                      success_rate (Σ), latency_score (Λ),               *)
(*                      previous_hash ]                                    *)
(*   - push(): rejeita timestamp <= last_timestamp (Gravity-1)             *)
(*   - push(): rejeita previous_hash != last_hash (Loopseal-2)             *)
(*   - GENESIS == "GENESIS" (âncora); hashes SHA3-256 hex; append-only     *)
(*                                                                         *)
(* REDUÇÕES HONESTAS DO MODELO (cada uma declarada, nenhuma escondida):   *)
(* 1. Hashes: SHA3-256 é substituído por tags abstratas Nat (=sucessor).  *)
(*    A especificação preserva a PROPRIEDADE de encadeamento (previous ==  *)
(*    último hash efetivo) e a unicidade dos tags; NÃO alega semântica do  *)
(*    SHA3 (isso é provado no núcleo Lean dos substratos 924/972 e         *)
(*    verificado em runtime por verify_integrity).                         *)
(* 2. Componentes Ω/Σ/Λ: cargas (payload) do entry — nenhum invariante do  *)
(*    ledger as restringe; ficam de fora do modelo sem perda de fidelidade *)
(*    (o phi é mantido no modelo como payload de exemplo).                 *)
(* 3. Escala do phi: inteiro (x10⁴ da banda Gap-1 0.577350<Φ<=0.999900)    *)
(*    é DOMÍNIO de dados; o ledger não filtra phi — só Gravity-1 (monotonia)*)
(*    e Loopseal-2 (encadeamento) restringem o push.                       *)
(* 4. Variável 'now' de timestamp: tick lógico crescente (determinístico,  *)
(*    mesmo padrão do e1: BASE_TIMESTAMP + window).                        *)
(* 5. Âncora: GenesisHash == 0 é a tag abstrata da gênese (Rust: "GENESIS").*)
(*                                                                         *)
(* FINITUDE DO MODELO (honestidade): a cadeia real é ilimitada; para TLC   *)
(* (model-check exaustivo) o Append é guardado por Len(chain) < MaxWindows.*)
(* A abstração preserva a alcançabilidade de todo prefixo com <= MaxWindows *)
(* janelas; liveness vale dentro desse horizonte.                          *)
(*                                                                         *)
(* PROPRIEDADES VERIFICÁVEIS (TLC/Apalache, model-check):                 *)
(*   Invariantes: TypeOK, Gravity1Inv, Loopseal2Inv, GenesisAnchoredInv,   *)
(*                UniqueWindowsInv, HashTagsDistinctInv                    *)
(*   Temporal:    Liveness == <>(Len(chain) >= MaxWindows)                 *)
(*                                                                         *)
(* STATUS DE TOOLING: TLC/Apalache NÃO instalados neste host (verificado   *)
(* 2026-09-06). Este módulo está PRONTO para model-check; nenhum resultado *)
(* de exploração é declarado até que uma ferramenta o execute.             *)
(***************************************************************************)

EXTENDS Naturals, Sequences

CONSTANTS
    PhiMax,        \* teto do domínio de dados do phi (escala x10⁴ da banda)
    MaxWindows,    \* número máximo de janelas modeladas (limite para TLC)
    GenesisWindow  \* window_id sintético da âncora gênese

ASSUME
    PhiMax >= 0 /\ MaxWindows >= 1 /\ GenesisWindow = 0

(***************************************************************************)
(* Âncora abstrata: GENESIS == "GENESIS" em Rust é representado pelo       *)
(* valor 0 (tag abstrata para o hash da gênese; ver redução 1).            *)
(***************************************************************************)
GenesisHash == 0

VARIABLES
    chain,    \* Seq de entries — única estrutura mutável, só por Append
    nextTs,   \* próximo timestamp (Gravity-1: estritamente crescente)
    nextWin,  \* próximo window_id (estritamente crescente, sem reuso)
    hashSeq   \* contador abstrato de hashes (tags únicas)

vars == <<chain, nextTs, nextWin, hashSeq>>

(***************************************************************************)
(* Entrada canônica do modelo: todos os campos da CoherenceEntry real,    *)
(* exceto Ω/Σ/Λ (payload; redução 2). A posição i da cadeia tem prev =     *)
(* hash da entrada i-1 (Loopseal-2).                                      *)
(***************************************************************************)
EntryType == [ window : Nat, ts : Nat, phi : Nat, prev : Nat, hash : Nat ]

TypeOK ==
    /\ chain \in Seq(EntryType)
    /\ nextTs >= 1
    /\ nextWin = Len(chain) + GenesisWindow
    /\ hashSeq >= GenesisHash

(***************************************************************************)
(* Estado inicial: cadeia vazia, âncora pronta, primeiro timestamp = 1.    *)
(* (Rust: ledger vazio com GENESIS aguardando o primeiro push.)            *)
(***************************************************************************)
Init ==
    /\ chain = <<>>
    /\ nextTs = 1
    /\ nextWin = GenesisWindow
    /\ hashSeq = GenesisHash

(***************************************************************************)
(* AppendEntry(phi) — única ação que estende a cadeia (append-only POR    *)
(* CONSTRUÇÃO: não existe ação de escrita/destruição de entrada).          *)
(*  - Loopseal-2: prev := hashSeq (hash do último) — nenhum outro valor    *)
(*    permitido (análogo à rejeição de previous_hash != last_hash).        *)
(*  - Gravity-1: nextTs' = nextTs + 1 (estritamente crescente).            *)
(*  - Guarda de finitude: Len(chain) < MaxWindows (ver FINITUDE acima).    *)
(*  - NOTA: o nome evita colisão com Sequences!Append (operador binário).  *)
(***************************************************************************)
AppendEntry(phi) ==
    LET e == [ window |-> nextWin,
               ts    |-> nextTs,
               phi   |-> phi,
               prev  |-> hashSeq,
               hash  |-> hashSeq + 1 ]
    IN
    /\ Len(chain) < MaxWindows
    /\ chain' = Append(chain, e)
    /\ nextTs' = nextTs + 1
    /\ nextWin' = nextWin + 1
    /\ hashSeq' = hashSeq + 1

AppendAct == \E p \in 0..PhiMax : AppendEntry(p)

(***************************************************************************)
(* Quiesce — quando o horizonte é atingido (Len(chain) = MaxWindows), o    *)
(* modelo fica quiescente (stutter explícito). Este é o estado de borda    *)
(* da CADEIA FINITA modelada: no ledger real o append continua; aqui só     *)
(* claimmos alcançabilidade/liveness ATÉ MaxWindows.                        *)
(***************************************************************************)
Quiesce ==
    /\ Len(chain) >= MaxWindows
    /\ UNCHANGED vars

Next == AppendAct \/ Quiesce

(***************************************************************************)
(* INVARIANTES DE SEGURANÇA                                               *)
(***************************************************************************)

(* Gravity-1 — timestamps estritamente crescentes na cadeia. *)
Gravity1Inv ==
    \A i \in DOMAIN chain :
        i > 1 => chain[i].ts > chain[i-1].ts

(* Loopseal-2 — cada entry carrega o hash do antecessor imediato; a
 * primeira entra com a âncora gênese (GENESIS). *)
Loopseal2Inv ==
    /\ (chain = <<>> \/ chain[1].prev = GenesisHash)
    /\ \A i \in DOMAIN chain :
         i > 1 => chain[i].prev = chain[i-1].hash

(* Âncora da gênese — primeira janela é a sintética GenesisWindow. *)
GenesisAnchoredInv ==
    chain = <<>> \/ (chain[1].window = GenesisWindow)

(* Janelas únicas e sem reuso — o apontador 'nextWin' nunca regride. *)
UniqueWindowsInv ==
    \A i, j \in DOMAIN chain :
        i < j => chain[i].window < chain[j].window

(* Tags abstratas de hash distintas por entrada — nunca reutilizadas. *)
HashTagsDistinctInv ==
    \A i, j \in DOMAIN chain :
        i < j => chain[i].hash /= chain[j].hash

(***************************************************************************)
(* PROPRIEDADE TEMPORAL (LIVENESS)                                        *)
(* Com fairness fraco (WF_vars) sobre AppendAct e Quiesce somente na      *)
(* borda, todo comportamento infinito cresce a cadeia até MaxWindows —     *)
(* existência de progresso sem impasse ("the chain grows").                *)
(***************************************************************************)
Liveness == <>(Len(chain) >= MaxWindows)

Spec == Init /\ [][Next]_vars /\ WF_vars(AppendAct)

=============================================================================
\* This is the Arkhe CoherenceLedger abstract specification (v376.1).
=============================================================================