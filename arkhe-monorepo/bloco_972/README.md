# 🏛️ BLOCO 972 — ACEITAÇÃO DO SUBSTRATE ARKHE-BITCOIN (v0.1.0)

> **Arquiteto-Ω** — Catedral OS
> Handover 971 → 972 — Data: **2026-09-06** — **v360.0**
>
> O Substrate **ARKHE-BITCOIN** (`packages/arkhe-bitcoin`) foi **ACEITO** após
> auditoria: **11/11 testes** (10 unit + 1 PoTT integration), clippy limpo
> (`--all-targets --features pott`), **zero `unsafe`**, **zero warnings**,
> integração **CLI binária `arkhe-bitcoin`** para delegação de agentes, e
> invariantes **I505–I510** formalizadas e provadas no núcleo Lean 4
> (v4.33.1, SEM Mathlib) — sem `sorry`, sem dívida formal. A integração com o
> substrate **924-POTT** (bloco 971) usa a API real (`receipt::`), nunca
> ficção.

---

## 📐 Registro no Ledger — BLOCO 972 v360.0

```json
{
  "bloco": 972,
  "versao": "v360.0",
  "handover_anterior": 971,
  "data": "2026-09-06",
  "tipo": "ARKHE_BITCOIN",
  "descricao": "Aceitação do Substrate ARKHE-BITCOIN. 11/11 testes (10 unit + 1 PoTT integration), clippy limpo (--all-targets --features pott), zero unsafe, zero warnings, WIF Base58Check correta (não Base64 do rascunho), secp256k1 0.29 alinhado ao bitcoin 0.32, CLI bin 'arkhe-bitcoin' (generate/address/wif/sign/verify/receipt) para integração de agentes, invariantes I505-I510 formalizadas e provadas no núcleo Lean 4 (sem Mathlib, sem sorry).",
  "bugs_corrigidos": [
    "rascunho usava base64::encode para WIF (WIF é Base58Check sha256d)",
    "secp256k1 0.30 vs 0.29 — dois secp256k1 no dependency graph, tipos incompatíveis (Secp256k1, XOnlyPublicKey, CompressedPublicKey)",
    "bitcoin 0.32 removera o campo Address.network e o método PrivateKey::compress; p2pkh exigia pk_hash por valor, p2wpkh &CompressedPublicKey, p2tr hrp KnownHrp",
    "sign_schnorr exigia feature rand-std (0.29) e Message+Keypair — resolvido com sign_schnorr_with_aux_rand determinístico",
    "pott_integration referenciava arkhe_pott::{signing_key_from_secret, xonly_pubkey} no raiz do crate; a API real vive em arkhe_pott::receipt:: — corrigido",
    "teste y_coordinate era de secp256k1 0.30 — 0.29 usa pk.x_only_public_key().1 (Parity)"
  ],
  "invariantes_novos": {
    "I505": "SEC1: chave pública comprimida = 33 bytes (prefixo 0x02/0x03 + x de 32); 1+32=33",
    "I506": "WIF: payload comprimido = 34 bytes (versão+chave+sufixo), checksum sha256d = 4 bytes",
    "I507": "BIP-340: assinatura = 64 bytes (r ‖ s, |r|=|s|=32); x-only/NodeId = 32 bytes",
    "I508": "PoTT: janela tout = tin + 60 ≥ tin (monotônica, não-nula); nonce ν = 16 bytes",
    "I509": "Prefixos de rede: mainnet 0x00/0x05, testnet 0x6f/0xc4 — P2PKH≠P2SH e redes distintas",
    "I510": "Digest do endereço (SHA-256, pott_integration) = 32 bytes; digest + NodeId = 64 bytes"
  },
  "testes": {
    "unit": 10,
    "integration": 1,
    "total": 11,
    "falhou": 0
  },
  "clis": [
    "generate --network <net> (JSON: secret_hex, wif, p2pkh, p2wpkh, p2tr, node_id)",
    "address --secret <hex> --kind p2pkh|p2wpkh|p2tr|all",
    "wif --secret <hex>",
    "sign --secret <hex> --message <text> (BIP-340, 64 bytes)",
    "verify --address <addr> --network <net> (exit 0/1)",
    "receipt --secret <hex> --address <addr> (feature pott, auto-verificado)"
  ],
  "status": "ACEITO",
  "selo": "CATEDRAL-OS-SUBSTRATE-ARKHE-BITCOIN-VERIFIED-2026-09-06",
  "verificacao_deste_host": {
    "lean": "lean.exe src/lean/SubstrateBitcoin972.lean -> OK 0 warnings (I505-I510 provados sem sorry)",
    "cargo_test": "cargo test -p arkhe-bitcoin --features pott -> 11/11 OK",
    "cargo_test_default": "cargo test -p arkhe-bitcoin -> 10/10 OK (pott off)",
    "clippy": "cargo clippy -p arkhe-bitcoin --all-targets --features pott -> limpo",
    "cli_smoke": "generate/verify/receipt executados no host com saída JSON correta"
  }
}
```

> **Nota de encadeamento:** o bloco 971 (`SUBSTRATE_924_POTT_INTERPLANETARY`,
> v359.0, `bloco_971/`) é o `handover_anterior` materializado neste monorepo.
> O bloco 972 fecha o encadeamento: é a primeira dependência *de facto* do
> substrate 924 (a integração `arkhe-pott` vive neste crate via feature
> `pott = ["dep:arkhe-pott"]`).

---

## ⚖️ PARECER DA AUDITORIA — Tabela de Realidade

| O que o Substrate declara | O que foi verificado | Veredito |
| :--- | :--- | :--- |
| 11/11 testes passando | 10 unit + 1 PoTT integration, 0 falhou | **Real** |
| Clippy limpo | `cargo clippy --all-targets --features pott` sem warnings | **Real** |
| Zero `unsafe` | `#![deny(unsafe_code)]` em `lib.rs` e lints do crate | **Real** |
| Zero warnings de build | `cargo build` default e `--features pott` sem warnings | **Real** |
| WIF Base58Check (não Base64) | render via `bitcoin::PrivateKey::to_wif` — prefixo 128 main / 239 test (verificado em WIF `c…` testnet) | **Real** |
| `secp256k1 0.29` alinhado | `bitcoin 0.32.102` usa secp256k1 0.29.1 internamente — um único no grafo | **Real** |
| Integração PoTT real | usa `arkhe_pott::receipt::{signing_key_from_secret,xonly_pubkey}` + `Chain::append_signed` + `validate verifica sinais` | **Real** |
| CLI bin operacional | `generate`, `verify` (válido **1.0**, network mismatched **recusado**), `receipt` com JSON correto | **Real** |
| I505–I510 formalizados | `src/lean/SubstrateBitcoin972.lean` — core Lean 4.33.1, 0 warnings, 0 `sorry` | **Real** |

---

## 📂 Estrutura

```
bloco_972/
└── README.md                       # este ledger (registro + parecer + verificação)

packages/arkhe-bitcoin/
├── Cargo.toml                      # clap, serde_json, hex; feature `pott`
├── README.md                       # doc do crate + checklist de correções
├── src/
│   ├── lib.rs                      # #![deny(unsafe_code)], re-exporta API
│   ├── error.rs                    # BitcoinError (mapa de erros da rede)
│   ├── network.rs                  # enum Network (4 redes) + prefixos
│   ├── key.rs                      # PrivateKey (chave, endereços, WIF, BIP-340)
│   ├── address.rs                  # parse/validate + describe_network
│   ├── pott_integration.rs         # receipts PoTT (feature `pott`)
│   └── bin/arkhe-bitcoin.rs        # CLI (generate/address/wif/sign/verify/receipt)
└── AGENTS.md                       # contrato de delegação para agentes (570)
```

Os artefatos Lean vivem em `src/lean/SubstrateBitcoin972.lean` (núcleo canônico,
não duplicado — decisão de higiene Ghost-3 do bloco 971).

---

## 📚 Invariantes formalizados (I505–I510) — núcleo puro, sem dívida

| Invar. | Teorema (arith.) | Prova |
| :-- | :-- | :-- |
| I505 | `compressed_public_key_len = 33` (1 + 32 SEC1) | `native_decide` |
| I506 | `wif_payload_len_compressed = 34`, checksum `= 4` | `native_decide` / `rfl` |
| I507 | `schnorr_signature_len = 64`, `r_len = s_len`, x-only `= 32` | `native_decide` / `rfl` |
| I508 | `tin ≤ tin + 60`; `60 > 0`; nonce `= 16` | `Nat.le.intro rfl` / `native_decide` / `rfl` |
| I509 | prefixos P2PKH≠P2SH e testnet≠mainnet | `decide` |
| I510 | digest SHA-256 `= 32`; digest + NodeId `= 64` | `rfl` / `native_decide` |

Verificação (executada neste host):

```bash
# Lean — compila SEM warnings e SEM sorries (todos os teoremas provados)
lean.exe src/lean/SubstrateBitcoin972.lean

# Rust — 11/11 (com PoTT) e 10/10 (sem)
cargo test -p arkhe-bitcoin --features pott
cargo test -p arkhe-bitcoin

# Rust — clippy limpo (bin + lib + exemplo)
cargo clippy -p arkhe-bitcoin --all-targets --features pott
```

---

## 🤖 Integração de agentes + CLI (bloco 972)

### CLI binária (`src/bin/arkhe-bitcoin.rs`)

Ponto de entrada determinístico e auditable para o orquestrador
(**570-CLAUDE-CODE-ORCHESTRATOR**) e para agentes delegados:

```bash
# gerar chave + artefatos (JSON)
cargo run -p arkhe-bitcoin --bin arkhe-bitcoin -- generate --network testnet

# validar endereço contra a rede (exit 0 = válido, 1 = inválido)
cargo run -p arkhe-bitcoin --bin arkhe-bitcoin -- verify \
  --address tb1qlkazkq82ecw9kh0vyuc7k8hujfg0hvkdx5rp7s --network testnet

# receipt PoTT auto-verificado (feature pott)
cargo run -p arkhe-bitcoin --bin arkhe-bitcoin --features pott -- receipt \
  --secret <hex> --address <addr>
```

* `verify` recusou endereço testnet (`m…`) contra `--network bitcoin` (exit 1) — validação de rede real.
* `receipt` imprime `{h, nu, node, tin, tout, sig}` — o receipt foi auto-verificado
  dentro de `create_pott_receipt_for_address` antes de sair do crate.

### Contrato de delegação

`packages/arkhe-bitcoin/AGENTS.md` documenta, para os agentes do orquestrador:

1. **Nunca** gerar chave/endereço fora do CLI ou da lib (uma única fonte).
2. Sempre validar endereço contra a rede alvo antes de assinar/emitir receipt.
3. `receipt` exige a feature `pott`; o CLU deve indicá-la (`cargo … --features pott`).
4. Levar o `selo`/JSON do receipt ao ledger (Loopseal-1: temporal chain anchor).

---

## 🧵 Histórico de correções (bitrot do rascunho vs. API real)

1. **WIF**: o rascunho usava `base64::encode` — WIF é **Base58Check** com
   checksum `sha256d`. Corrigido delegando a `bitcoin::PrivateKey::to_wif`
   (prefixos 128/239 corretos por rede).
2. **secp256k1 0.30 vs 0.29**: o crate direto 0.30 colidiu com o secp256k1
   **0.29.1** interno do `bitcoin 0.32.102` (`Secp256k1`, `XOnlyPublicKey`,
   `PublicKey`, `CompressedPublicKey`) — alinhado a `0.29`.
3. **API bitcoin 0.32**: `Address` perdeu o campo `.network` (agora
   `is_valid_for_network` + `describe_network`); `PrivateKey::compress` não
   existe (campo `pub compressed`); `p2pkh(pk, net)` por valor;
   `p2wpkh(&CompressedPublicKey, hrp)`; `p2tr(secp, xonly, None, hrp)`.
4. **BIP-340 0.29**: `sign_schnorr` exigia feature `rand-std`; usado
   `sign_schnorr_with_aux_rand(msg, keypair, &[0u8;32])` (determinístico) +
   `Signature::serialize()`.
5. **PoTT real**: o rascunho inventava `ReceiptBuilder`/`Nonce`/
   `PayloadHash`/`with_secret_key` — a API real é
   `arkhe_pott::receipt::{signing_key_from_secret, xonly_pubkey}`,
   `Chain::new(h, nu)`, `append_signed(&[u8;32], node, tin, tout)`,
   `verify_schnorr(&node)`, `receipt.{h,nu,node,tin,tout,sig}`.
6. **Paridade y**: `y_coordinate()` é 0.30; em 0.29 usa-se
   `pk.x_only_public_key().1` (`Parity`).

---

## 🏛️ PARECER FINAL

O Substrate **ARKHE-BITCOIN** é **ACEITO**. A auditoria corrigiu **6** divergências
do rascunho contra as APIs reais — todas antes da aceitação — com testes de
regressão (WIF `K/L`/`c` por prefixo, 64 bytes Schnorr, mismatch de rede,
receipt PoTT roundtrip + auto-verify). As invariantes **I505–I510** estão
provadas no núcleo Lean 4 (sem Mathlib, sem `sorry`), mantendo o precedente de
honestidade dos blocos 966/971. A integração com o substrate 924 (`arkhe-pott`)
usa a **API real auditada no bloco 971** — a custódia viaja, a prova viaja no Lean.

```text
O endereço nasce da curva.
O receipt prova o trânsito.
O agente delega e o Lean testemunha.
```

**Selo:** `CATEDRAL-OS-SUBSTRATE-ARKHE-BITCOIN-VERIFIED-2026-09-06`