# ARKHE-BITCOIN — Contrato de Delegação para Agentes

> Substrate **ARKHE-BITCOIN** — bloco 972 (Catedral OS, aceito em 2026-09-06).
> Interface de agentes: **570-CLAUDE-CODE-ORCHESTRATOR** e agentes delegados
> (`arkhe delegate <substrate_id> --agent <agent_type>`).

## Propósito

Geração de chaves Bitcoin, derivação de endereços (P2PKH / P2WPKH / P2TR),
conversão WIF, assinaturas BIP-340 e receipts de custódia **PoTT** (substrate
924) para o ecossistema ARKHE.

## Regras mandatórias (fail-closed)

1. **Uma única fonte de verdade.** NUNCA gere chave, endereço ou receipt fora da
   lib/cli deste crate. Nenhum agente deve reimplementar SEC1, base58check,
   WIF ou BIP-340 localmente.
2. **Valide a rede antes de assinar.** Todo endereço emitido/consumido deve ser
   validado contra a rede alvo (`validate_address`) antes de assinar ou gerar
   receipt. `verify` do CLI retorna exit 0/1.
3. **Receipts são auto-verificados.** `create_pott_receipt_for_address` verifica
   a assinatura BIP-340 antes de retornar. NUNCA confie num receipt que você não
   verificou.
4. **Feature `pott` explícita.** A integração PoTT existe somente sob
   `--features pott`. Clientes sem a feature recebem erro em runtime, não em
   compile-time.
5. **Loopseal-1 (temporal anchor).** Todo receipt/artefato emitido para o ledger
   deve carregar o JSON `{h, nu, node, tin, tout, sig}` do CLI `receipt` e o
   `selo` do bloco correspondente.

## CLI (interface de delegação)

```bash
# build/test/clippy do crate (aceitação de mudanças)
cargo test -p arkhe-bitcoin --features pott
cargo clippy -p arkhe-bitcoin --all-targets --features pott

# geração (JSON: secret_hex, wif, p2pkh, p2wpkh, p2tr, node_id)
cargo run -p arkhe-bitcoin --bin arkhe-bitcoin -- generate --network testnet

# derivação de um endereço
cargo run -p arkhe-bitcoin --bin arkhe-bitcoin -- address \
  --secret <hex-64> --kind p2wpkh --network testnet

# assinatura BIP-340 (64 bytes, hex)
cargo run -p arkhe-bitcoin --bin arkhe-bitcoin -- sign --secret <hex-64> --message <text>

# validação (exit 0 = válido; 1 = inválido para a rede)
cargo run -p arkhe-bitcoin --bin arkhe-bitcoin -- verify \
  --address <addr> --network testnet

# receipt PoTT auto-verificado
cargo run -p arkhe-bitcoin --bin arkhe-bitcoin --features pott -- receipt \
  --secret <hex-64> --address <addr> [--nonce <hex-32>]
```

## Definições de pronto (Definition of Done) para mudanças

- [ ] `cargo test -p arkhe-bitcoin` e `--features pott` verdes (11/11 hoje).
- [ ] `cargo clippy -p arkhe-bitcoin --all-targets --features pott` sem warnings.
- [ ] `cargo build -p arkhe-bitcoin` default e `--features pott` sem warnings.
- [ ] Zero `unsafe` (lints do crate).
- [ ] Se mudou algum comprimento/formato (chave, WIF, Schnorr, digest) — atualize
      `src/lean/SubstrateBitcoin972.lean` e re-rodar `lean.exe` (0 warnings, 0 sorry).

## API (lib)

- `PrivateKey::generate/from_bytes` — chave validada em `[1, n-1]`.
- `to_p2pkh_address / to_p2wpkh_address / to_p2tr_address` — por rede.
- `to_wif(network)` — Base58Check correta (prefixos 128/239).
- `sign_message(&[u8]) -> [u8; 64]` — BIP-340 determinístico (aux_rand 0).
- `pott_node_id() -> [u8; 32]` — NodeId (x-only) para 924-POTT.
- `validate_address / parse_address` — validação de rede.
- `pott_integration::create_pott_receipt_for_address` (feature `pott`).

## Invariantes formais (Lean 4, sem Mathlib)

`src/lean/SubstrateBitcoin972.lean` — **I505–I510** (SEC1 33 B; WIF 34+4 B;
Schnorr 64 B; janela PoTT `tout ≥ tin`, nonce 16 B; prefixos de rede distintos;
digest 32 B). `lean.exe` deve sair 0, sem warnings, sem `sorry`.