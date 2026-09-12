# `arkhe-bitcoin`

Geração de chaves e endereços Bitcoin para o ecossistema ARKHE.

- **P2PKH** (legacy), **P2WPKH** (SegWit v0), **P2TR** (Taproot key-path).
- **WIF** (Base58Check, via crate `bitcoin`) e assinaturas BIP-340 (Schnorr).
- Validação/parsing de endereços com verificação de rede.
- **PoTT** (feature `pott`): receipts de custódia temporais do substrate
  924-POTT-INTERPLANETARY-TRANSPORT.

## Uso

```rust
use arkhe_bitcoin::{PrivateKey, Network};

let key = PrivateKey::generate();
let addr = key.to_p2wpkh_address(Network::Testnet)?;
assert!(arkhe_bitcoin::validate_address(&addr, Network::Testnet));
```

## Feature `pott`

```toml
[dependencies]
arkhe-bitcoin = { path = "packages/arkhe-bitcoin", features = ["pott"] }
```

```rust
let receipt = arkhe_bitcoin::pott_integration::create_pott_receipt_for_address(
    &key, &addr, &[42u8; 16],
)?;
```

## CLI (`arkhe-bitcoin` bin) — interface para agentes

Ponto de entrada determinístico para o orquestrador (570-CLAUDE-CODE-ORCHESTRATOR)
e agentes delegados. Ver `AGENTS.md` para o contrato completo de delegação.

```bash
cargo run -p arkhe-bitcoin --bin arkhe-bitcoin -- generate --network testnet
cargo run -p arkhe-bitcoin --bin arkhe-bitcoin -- address --secret <hex-64> --kind p2tr
cargo run -p arkhe-bitcoin --bin arkhe-bitcoin -- sign --secret <hex-64> --message <text>
cargo run -p arkhe-bitcoin --bin arkhe-bitcoin -- verify --address <addr> --network testnet
cargo run -p arkhe-bitcoin --bin arkhe-bitcoin --features pott -- receipt --secret <hex-64> --address <addr>
```

Comandos: `generate` (JSON: secret_hex, wif, p2pkh, p2wpkh, p2tr, node_id),
`address`, `wif`, `sign` (BIP-340, 64 bytes), `verify` (exit 0/1), `receipt`
(feature `pott`, auto-verificado).

## Testes

```bash
cargo test -p arkhe-bitcoin                 # sem PoTT
cargo test -p arkhe-bitcoin --features pott # com PoTT
cargo clippy -p arkhe-bitcoin --all-targets
```

## Notas de segurança

- `#![deny(unsafe_code)]` — nenhum `unsafe`.
- Chaves privadas validadas na curva (`0 < secret < n`) na criação do crate.
- Endereços validados contra a rede alvo antes de qualquer uso.
- Receipts PoTT auto-verificados antes de retornar ao chamador.

## Correções vs. rascunho

1. **WIF** era `base64::encode` → agora Base58Check (`bitcoin::PrivateKey::to_wif`).
2. **`sha2`** faltava em `[dependencies]` (usado em `sign_message`).
3. **`FromStr`** não importado em `address.rs`.
4. **Feature `pott`** não definida em `[features]` (dep opcional nāo cria
   feature `pott` implicitamente) — corrigida com `pott = ["dep:arkhe-pott"]`.
5. **API PoTT real**: o rascunho usava `ReceiptBuilder`/`Nonce`/`PayloadHash`
   que não existem no `arkhe-pott`. Reescrito contra `Chain::append_signed`,
   `signing_key_from_secret`, `xonly_pubkey` e âncora TAI (`tai_from_posix_i64_to_u64`).
6. WIF com rede correta (prefixo mainnet/testnet via `bitcoin::PrivateKey`).

Selo: `ARKHE-BITCOIN-2026-09-06`