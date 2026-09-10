# Protocolo P2P — Especificação (draft v0.1)

**Bloco de origem:** 1053 (plano v9.4, tarefa 1)
**Data:** 2026-09-10
**Estado:** v1.2 — decisão de transporte (1055) + malha 2 nós verificada (1056) + fechamento I624 (1057)
**Bloco de verificação esperado:** 1054

Esta especificação define o contrato de rede para o `arkhe-blink-bridge`. É um
documento de RASCUNHO: o backend do crate é hoje **simulado em memória** (digest
SHA3-256 determinístico, sem rede). Este documento não reivindica nada em
produção; define o alvo para quando o backend real entrar.

## 1. Honestidade do documento

- Nenhuma malha multi-nó real existe ainda: o crate `arkhe-p2p` (bloco 1055)
  entrega o núcleo — identidade Ed25519, mensagens assinadas e a construção real
  da rede libp2p (Kademlia + gossipsub + identify) com `listen_on` exercitado.
- **Decisão de transporte tomada no bloco 1055**: `libp2p 0.56.0` (pinado),
  auditoria empírica — MSRV 1.83.0, compila e executa no rustc 1.94.0 local
  (Windows), API atual verificada contra a fonte vendorizada.
- Autenticação baseada em I624 usa o **key registry real do crate** (chaves
  master/backup em memória), que **NÃO é um HSM**. HSM é trabalho futuro e está
  fora do escopo deste rascunho.
- Não existe crate `arkhe-bridge-564-mcp` no workspace. Referências reais:
  - Substrate arquitetural `564-MCP-STATELESS-BRIDGE` (CLAUDE.md).
  - Cliente MCP stub existente: `packages/arkhe-field-stability` (`mcp_stub.rs`,
    reqwest, teste com mock HTTP) — a ser usado como referência de cliente.

## 2. Tipos de mensagem

| Tipo | Direção | Uso |
|---|---|---|
| `Handover` | par → par | Entrega de transação unificada (14 passos) para processamento |
| `Validation` | par → par | Pedido de verificação de invariante (MEV-002 atomicidade, ANTH-001 formalidade) |
| `Sync` | par → par | Sincronização de estado / digest SHA3-256 do ledger |

Formato de payload: CBOR-subset (convenção dos blocos 1008, 924/972).

## 3. Estrutura de pares

- `peer_id`: hash SHA3-256 da chave pública registada no key registry (I624 —
  registo master/backup do crate; HSM fora de escopo).
- `multiaddr`: formato libp2p (`/ip4/.../tcp/.../p2p/<peer_id>`). Versão atual não
  impõe transporte; lista de addresses acompanha o peer.

## 4. Roteamento — DHT Kademlia (decidido)

- Tabela de peers com buckets por distância XOR sobre `peer_id`/`record_key`.
- Lookup: `FIND_NODE` / `FIND_VALUE` / `STORE` sobre digests.
- `Sync` de ledger usa `STORE(digest_bloco)` + reconciliação por prefixo.
- Honestidade: Kademlia foi **adotado** (bloco 1055) via libp2p 0.56.0
  (`libp2p::kad::Behaviour`, `MemoryStore`); a malha multi-nó ainda não foi
  exercitada — apenas a construção da rede (Kademlia + gossipsub + identify) e o
  `listen_on` real foram testados.

## 5. Autenticação e integridade

- Base: I624 — `ArkheVerifier` valida chaves master/backup **em memória**.
  NENHUM HSM está envolvido.
- Transações assinadas com a chave master; verificação no par remoto via
  `ArkheVerifier` (ANTH-004: `config_hash` imutável em viagem).
- Digest de integridade: SHA3-256 (MEV-002 atomicidade; mesmo digest do backend
  simulado — paridade garantida por construção).
- Não-repúdio e notarização TLSNotary (Provenance-1) ficam para a camada de
  transporte real, fora deste rascunho.

## 6. Restrições do bridge (16 invariantes)

O protocolo NÃO pode violar:

- **MEV-002**: commit atómico — mensagens de `Validation` só são confirmadas após
  digest completo; nada parcial.
- **ANTH-001**: formalidade — toda mensagem passa pela sequência de 14 passos
  (MEV → ANTH → ARKHE → commit único).
- **ANTH-006**: monitor de momentum — janela de 10 ações por trajetória de par;
  `MomentumBrakeActivated` rejeita padrões suspeitos **antes** da transmissão.

## 7. Critérios de aceite

1. Documento aprovado e versionado pelo Arquiteto-Ω. ✔ (este rascunho)
2. Nenhuma reivindicação de rede real sem bloco que a verifique (precedente
   blocos 990/1006/1011 — parecer sobre substrato inexistente).
3. Implementação futura deve fechar paridade com o digest simulado: mesmo SHA3-256
   para o mesmo input, testável por integração.
4. `cargo test --workspace --features full` NÃO é critério válido: o workspace não
   tem feature `full` (só `tokio = { features = ["full"] }` em dep). O critério
   real em CI será `cargo test -p arkhe-blink-bridge` + verificação por bloco.

## 8. Contrato com o bloco 1055

O bloco 1055 registrou: (a) esta spec como aprovada (rascunho → v1.0 de decisão);
(b) a decisão definitiva de transporte: **libp2p 0.56.0** (auditoria empírica,
MSRV 1.83.0 no rustc 1.94.0); (c) o crate `packages/arkhe-p2p` real — identidade
Ed25519 + mensagens assinadas + rede libp2p construída e com `listen_on`
exercitado — NUNCA alegando malha multi-nó ou `P2P_OPERACIONAL` sem bloco que as
verifique (precedente blocos 990/1006/1011/1054).

**Selo (v1.0 de decisão):** `CATEDRAL-OS-P2P-DECISAO-TRANSPORTE-BLOCO-1055-2026-09-10`

## 9. Malha multi-nó verificada (bloco 1056)

O bloco 1056 construiu e verificou uma malha real de **2 nós em loopback** sobre o
crate `arkhe-p2p` (libp2p 0.56.0 pinado):

- **Dial + conexão:** B diala A (`/ip4/127.0.0.1/tcp/{porta}/p2p/{PeerId}`), handshake
  noise + yamux, conexão estabelecida observada nos dois lados. O teste espera a
  conexão estar **estabelecida em ambos** antes de qualquer operação de overlay —
  elimina a corrida de dial vista no Windows (`AddrInUse`, WSAE10048) quando um
  nodo diala o outro enquanto a conexão entrante ainda não foi registrada.
- **Gossipsub assinado + anti-replay:** A publica 11 mensagens `P2PMessage`
  assinadas (10 frescas + 1 obsoleta) no tópico `/arkhe/blocks/1`; B recebe todas;
  `validate(public_key, now, 60)` aceita 10/10 frescas e **rejeita a obsoleta**
  (1/1). A espera pelo evento `Subscribed` de B antes de publicar evita
  `NoPeersSubscribedToTopic` (o heartbeat do gossipsub propaga a assinatura).
- **Kademlia DHT:** `add_address` + `bootstrap()` nos dois nós; `put_record`
  (Quorum::One) confirma `PutRecord(Ok)` com `requests: 2, success: 2,
  failure: 0`; `get_record` de A recupera o valor exato via
  `GetRecordOk::FoundRecord`. Registro observado como `InboundRequest::PutRecord`
  no nodo A.
- **Descoberta empírica (registrada por honestidade):** em libp2p-kad 0.48.0 o
  `Config::default()` inicia em `Mode::Client` (`auto_mode`), que nega o protocolo
  `/ipfs/kad/1.0.0` no lado entrante (handler.rs:607-611) até que o modo suba para
  `Server`; em 2 nós simples o auto_mode não dispara e a lista de protocolos do
  identify não contém `/ipfs/kad/1.0.0` — toda requisição falha silenciosamente
  (`failure: 1` sem eventos). Correção: `kad.set_mode(Some(Mode::Server))` em
  `behaviour.rs`.

**Resultado:** 14/14 testes no crate (9 unit + 5 integração), clippy `-D warnings`
limpo, `unsafe_code = "deny"`, blink-bridge intacto (23/23). Escopo honesto: malha
de 2 nós em loopback na mesma máquina; NÃO há ainda `P2P_OPERACIONAL` (solução de
3+ hosts, persistência de peerstore, churn), NÃO há score Ω fabricado — as
métricas acima são observadas nos testes.

**Bloco de registro:** `bloco_1056/bloco_1056.json`

**Selo (v1.1 — malha verificada):** `CATEDRAL-OS-P2P-MALHA-2-NOS-BLOCO-1056-2026-09-10`

## 10. Fechamento do mapeamento I624 (bloco 1057)

O item 1 do plano do bloco 1056 ("Fechar mapeamento I624") foi executado com
**vínculo criptográfico real** entre o key registry do `arkhe-blink-bridge` e as
identidades da malha:

- O cofre I624 (`ArkheVerifier`) deixa de guardar **nomes** de chave e passa a
  guardar **identidades reais** `arkhe_p2p::identity::Identity` (Ed25519 → PeerId
  da malha libp2p 0.56.0), `register_key(key_id, &Identity)`.
- `verify_peer_binding(key_id, peer_id_bytes)` — prova que o PeerId apresentado
  pela rede É o dono registado do `key_id` (I624).
- `verify_signed_message(key_id, &P2PMessage, now, max_age)` — prova assinatura
  Ed25519 pela chave registada + frescor (anti-replay por janela de tempo).
- `UnifiedBlinkBridge::register_key(key_id, &Identity)` expõe o vínculo no fluxo
  das 16 invariantes: uma `UnifiedTransaction` com `key_id` registado cruza a
  ponte inteira; um `key_id` desconhecido é rejeitado em I624.

Testes de integração em `packages/arkhe-blink-bridge/tests/i624_p2p_binding_test.rs`
(6, no nível da crate): binding de PeerId registado, rejeição de PeerId atacante,
mensagem adulterada rejeitada, mensagem obsoleta rejeitada (anti-replay),
transação da ponte referenciando chave registada, transação com chave não
registada rejeitada.

**Resultado:** blink-bridge 35/35 (21 unit + 6 integração I624 + 8 unificados),
clippy `-D warnings` limpo, `arkhe-p2p` intacto (14/14 — regressão). Escopo
honesto: identidades de teste geradas em tempo de execução; o **armazenamento
seguro/persistente** das chaves continua fora (HSM é trabalho futuro); o vínculo
cobre tipo + assinatura, não HSM.

**Bloco de registro:** `bloco_1057/bloco_1057.json`

**Selo (v1.2 — I624 fechado):** `CATEDRAL-OS-I624-CRIPTO-VINCULADO-BLOCO-1057-2026-09-09`

## 11. Erratas registadas (bloco 1057)

1. **Namespace I624 (colisão apurada e resolvida).** O uso canónico de `I624`
   neste monorepo é o invariante ARKHE **«Chaves»** (família I619/I622/I623/I624
   do `arkhe-blink-bridge`), implementado e testado desde o bloco 1052.
   Referências de `I624` fora dessa família foram rejeitadas por pareceres
   anteriores **sem substrato**: `parecer_v510_substrato_fotonico.md:17`
   (prensas v508/v509 «Não são nós reais») e `parecer_v510_1_integracao_ml.md:33`
   («assinatura I624» — sem substrato de ML). A string «MLPerf» não ocorre no
   monorepo (0 ocorrências). **Decisão:** I624 permanece ARKHE Chaves, agora
   cripto-vinculado (bloco 1057); invariantes futuros (ex.: desempenho MLPerf)
   tomam ID fresco (I625+), nunca reutilizam I624 fora da família implementada.
2. **Errata temporal (herdada da sessão anterior).** O cabeçalho deste documento
    («Data: 2026-09-10»), os registos `bloco_1055.json`/`bloco_1056.json` e os
    selos das secções 8/9 carregam data `2026-09-10`, adiantada em relação ao
    relógio real de verificação (`2026-09-09`, confirmado via `Get-Date`).
    Conforme Loopseal-2 (append-only), esses registos **não são reescritos** aqui;
    ficam **flagados para erratização** nas suas próprias revisões de aceitação. O
    bloco 1057 adota a data de emissão real: `2026-09-09`.

## 12. Erratização temporal (bloco 1058)

Erratização dos selos adiantados detetados na secção 11 (bloco 1057). Conforme
Loopseal-2 (append-only), o cabeçalho «Data» e os selos das secções 8/9 deste
documento **não são reescritos**; a correção fica registada aqui e nos registos
de errata homologados:

- **Header Data:** `2026-09-10` → `2026-09-09` (data real de emissão).
- **Selo secção 8 (v1.0 de decisão):**
  `CATEDRAL-OS-P2P-DECISAO-TRANSPORTE-BLOCO-1055-2026-09-10` →
  `CATEDRAL-OS-P2P-DECISAO-TRANSPORTE-BLOCO-1055-2026-09-09`.
- **Selo secção 9 (v1.1 — malha verificada):**
  `CATEDRAL-OS-P2P-MALHA-2-NOS-BLOCO-1056-2026-09-10` →
  `CATEDRAL-OS-P2P-MALHA-2-NOS-BLOCO-1056-2026-09-09`.
- **Registos:** `bloco_1058/bloco_1058_errata_p2p_spec.json`
  (ERRATA_REGISTRADA, sha256 do alvo `369FC4192CBC7A…`),
  `bloco_1058/bloco_1058_errata_p2p_spec_ratificada.json`
  (ERRATA_RATIFICADA), `bloco_1058/bloco_1058.json`.
- **Selos:** `CATEDRAL-OS-ERRATA-P2P-SPEC-BLOCO-1058-2026-09-09`,
  `CATEDRAL-OS-ERRATA-P2P-SPEC-RATIFICADA-BLOCO-1058-2026-09-09`.