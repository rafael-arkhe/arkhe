# 🧬 BLOCO 484 v45 — ALEATORIEDADE VERIFICÁVEL ARPA RANDCAST (BLS-TSS)

> **Arquiteto-Ω** — Catedral OS
> Integração da rede ARPA (randcast) como fonte de aleatoriedade verificável
> on-chain para Zeno, QSP e Tardos. Veredito pós-revisão: a proposta original
> continha erros de interface (assinatura, `getBalance`, rede inexistente);
> implementamos a porção **vetada** contra o **repositório oficial**.

---

## 📐 Registro no Ledger — BLOCO 484 v45

```text
BLOCO 484 — ALEATORIEDADE VERIFICÁVEL ARPA RANDCAST (v45)
├── handover_anterior: 1313 (v44)
├── handover_atual: 1314
├── módulos implementados:
│   ├── arpa_integration.py                 — cliente Python (request + poll eth_getLogs)
│   ├── arpa_randcast.c                     — cliente C READ-ONLY (AURIX não assina)
│   ├── CatedralRandcastConsumer.sol        — consumidor Solidity (compilado solc 0.8.28)
│   ├── third_party/randcast-user-contract/ — fontes oficiais verificadas (vendored)
│   └── src/Governance/Randomness/ARPAVerifiable.lean — contrato formal (Lean 4)
├── garantias (status SPEC):
│   ├── BLS-TSS: requisição atendida entrega aleatoriedade BLS assinada
│   ├── RequestId = bytes32; Randomness = uint256 (não bytes32 como na proposta)
│   ├── Adapters reais (ETH): Ethereum 0x4363154E…, Base 0xDBa5dE35…, Sepolia 0x46d29642…
│   └── Financiamento em ETH (não em ARPA token); sem Base Sepolia documentada
└── assinatura: "O rote da Catedral não é sorteado: é testemunhado e auditado on-chain."
```

---

## 📂 Estrutura

```
├── arpa_integration.py                  # cliente de requisição + espera do fulfillment
├── arpa_randcast.c                      # consumo read-only (eth_getLogs), transporte fake p/ unidade
├── CatedralRandcastConsumer.sol         # consumidor (importa GeneralRandcastConsumerBase real)
├── third_party/randcast-user-contract/  # clone oficial ARPA-Network/Randcast-User-Contract
├── build_484/                           # ABI/bin gerados pelo solc (reprodutível)
└── src/Governance/Randomness/ARPAVerifiable.lean
```

O cliente C **não assina**: a AURIX apenas consulta `eth_getLogs` (evento
`RandomnessFulfilled`) — a assinatura/requisição fica no Python (web3/eth_account).

---

## 🚀 Como executar

```bash
# 1. Python — selftest hermético (sem rede; transporte fake determinístico)
python arpa_integration.py

# 2. C — testes de unidade (sem rede)
gcc -std=c99 -Wall -Wextra -DARPA_UNIT_TEST arpa_randcast.c -o arpa_test
./arpa_test

# 3. C — modo real (apenas libcurl; sem json-c)
gcc -std=c99 -DARPA_REAL_API arpa_randcast.c -lcurl -o arpa_cli
ARPA_RPC_URL=<rpc> ARPA_CONSUMER_ADDRESS=<0x…> ./arpa_cli

# 4. Solidity — compilação reprodutível
npx --yes solc@0.8.28 "CatedralRandcastConsumer.sol" --base-path . \
     --include-path "third_party" --abi --bin -o "build_484"

# 5. Lean 4 — contrato formal (libre de Mathlib, sem sorry)
lean src/Governance/Randomness/ARPAVerifiable.lean
```

---

## ⚖️ Invariantes tocados (vetted)

- **Gap-1:** a semente de 53 bits alimenta Zeno/QSP/Tardos; Φ_C **nunca** é tocado.
- **Loopseal-2:** todo fulfillment fica gravado on-chain (evento + receipt) — rastro
  imutável; o C lê pelo `data` do log (uint256 → 53 bits).
- **Eth-2:** o cliente C consulta apenas endereço do contrato + requestId (data minimização).
- **Provenance-1:** comunicação com RPC deve ser notarizada por TLSNotary em produção.

---

## Vulnerabilidades da proposta corrigidas nesta revisão

| Proposta original | Realidade (repositório oficial) | Ação |
|---|---|---|
| `_fulfillRandomness(bytes32, bytes32)` | `_fulfillRandomness(bytes32, uint256)` | corrigido no contrato e no Lean |
| `getBalance` / `_getBalance` no base | não existe na base | removido |
| Base Sepolia como rede | não documentada | removido |
| Financiamento em ARPA token | ETH (flat fee) | documentado |
| C cliente assina transação | AURIX não assina (read-only) | redesenho |

**Selo:** `CATEDRAL-OS-BLOCO484-v45-2026-09-01`

*Aleatoriedade não é acaso: é consenso assinado. Indeterminismo não é sorte: é verificação.* 🔮⛓️🧬