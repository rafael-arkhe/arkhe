//! CLI `arkhe-bitcoin` — interface de linha de comando do substrate
//! ARKHE-BITCOIN (bloco 972).
//!
//! Ponto de integração de agentes: o orquestrador (570-CLAUDE-CODE-ORCHESTRATOR)
//! e os agentes delegados podem invocar a geração de chaves, endereços, WIF,
//! assinaturas BIP-340 e receipts PoTT de forma determinística e auditável.
//!
//! ```text
//! cargo run -p arkhe-bitcoin --bin arkhe-bitcoin -- generate
//! cargo run -p arkhe-bitcoin --bin arkhe-bitcoin -- verify --address tb1q... --network testnet
//! cargo run -p arkhe-bitcoin --bin arkhe-bitcoin --features pott -- receipt --secret <hex> --address <addr>
//! ```

use arkhe_bitcoin::{validate_address, PrivateKey, Network};
use clap::{Parser, Subcommand};
use serde::Serialize;
use std::process::ExitCode;

/// ARKHE Bitcoin — geração de chaves/endereços e receipts PoTT.
#[derive(Parser, Debug)]
#[command(name = "arkhe-bitcoin", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Gera uma chave nova e imprime os artefatos derivados (JSON).
    Generate {
        /// Rede alvo (bitcoin, testnet, regtest, signet).
        #[arg(long, default_value = "testnet")]
        network: String,
    },
    /// Deriva endereço(s) a partir de uma chave secreta em hex.
    Address {
        /// Chave secreta de 32 bytes em hex.
        #[arg(long)]
        secret: String,
        /// Tipo do endereço: p2pkh | p2wpkh | p2tr | all.
        #[arg(long, default_value = "all")]
        kind: String,
        /// Rede alvo.
        #[arg(long, default_value = "testnet")]
        network: String,
    },
    /// Converte uma chave para WIF (Base58Check).
    Wif {
        /// Chave secreta de 32 bytes em hex.
        #[arg(long)]
        secret: String,
        /// Rede alvo.
        #[arg(long, default_value = "testnet")]
        network: String,
    },
    /// Assina uma mensagem com BIP-340 (Schnorr) e imprime a assinatura (hex).
    Sign {
        /// Chave secreta de 32 bytes em hex.
        #[arg(long)]
        secret: String,
        /// Mensagem a assinar.
        #[arg(long)]
        message: String,
    },
    /// Valida um endereço contra uma rede (exit 0 = válido, 1 = inválido).
    Verify {
        /// Endereço (P2PKH/P2WPKH/P2TR).
        #[arg(long)]
        address: String,
        /// Rede esperada.
        #[arg(long, default_value = "testnet")]
        network: String,
    },
    /// Cria um receipt PoTT auto-verificado para um endereço (feature `pott`).
    Receipt {
        /// Chave secreta de 32 bytes em hex.
        #[arg(long)]
        secret: String,
        /// Endereço alvo do receipt.
        #[arg(long)]
        address: String,
        /// Nonce de 16 bytes em hex (32 caracteres).
        #[arg(long, default_value = "00000000000000000000000000000042")]
        nonce: String,
    },
}

fn parse_secret(hex_input: &str) -> Result<[u8; 32], String> {
    let mut bytes = [0u8; 32];
    let decoded = hex::decode(hex_input).map_err(|e| format!("secreto inválido: {e}"))?;
    if decoded.len() != 32 {
        return Err(format!(
            "secreto deve ter 32 bytes (64 hex chars), tem {} bytes",
            decoded.len()
        ));
    }
    bytes.copy_from_slice(&decoded);
    Ok(bytes)
}

#[cfg(feature = "pott")]
fn parse_nonce(hex_input: &str) -> Result<[u8; 16], String> {
    let mut bytes = [0u8; 16];
    let decoded = hex::decode(hex_input).map_err(|e| format!("nonce inválido: {e}"))?;
    if decoded.len() != 16 {
        return Err("nonce deve ter 16 bytes (32 hex chars)".to_owned());
    }
    bytes.copy_from_slice(&decoded);
    Ok(bytes)
}

fn parse_network(name: &str) -> Result<Network, String> {
    match name.to_ascii_lowercase().as_str() {
        "bitcoin" | "mainnet" | "main" => Ok(Network::Bitcoin),
        "testnet" | "test" => Ok(Network::Testnet),
        "regtest" => Ok(Network::Regtest),
        "signet" => Ok(Network::Signet),
        other => Err(format!("rede desconhecida: {other}")),
    }
}

#[derive(Serialize)]
struct GeneratedOutput {
    secret_hex: String,
    wif: String,
    p2pkh: String,
    p2wpkh: String,
    p2tr: String,
    node_id: String,
    network: String,
}

#[derive(Serialize)]
struct AddressOutput {
    kind: String,
    address: String,
}

#[derive(Serialize)]
struct SignatureOutput {
    message: String,
    signature_hex: String,
    algorithm: String,
}

#[derive(Serialize)]
struct VerifyOutput {
    address: String,
    network: String,
    valid: bool,
}

#[cfg(feature = "pott")]
#[derive(Serialize)]
struct ReceiptOutput {
    h: String,
    nu: String,
    node: String,
    tin: u64,
    tout: u64,
    sig: String,
}

fn run(cli: Cli) -> Result<(), String> {
    match cli.command {
        Command::Generate { network } => {
            let network = parse_network(&network)?;
            let sk = PrivateKey::generate();
            let out = GeneratedOutput {
                secret_hex: hex::encode(sk.secret),
                wif: sk.to_wif(network).map_err(|e| e.to_string())?,
                p2pkh: sk.to_p2pkh_address(network).map_err(|e| e.to_string())?,
                p2wpkh: sk.to_p2wpkh_address(network).map_err(|e| e.to_string())?,
                p2tr: sk.to_p2tr_address(network).map_err(|e| e.to_string())?,
                node_id: hex::encode(sk.pott_node_id().map_err(|e| e.to_string())?),
                network: format!("{network:?}"),
            };
            println!("{}", serde_json::to_string_pretty(&out).unwrap());
        }
        Command::Address {
            secret,
            kind,
            network,
        } => {
            let network = parse_network(&network)?;
            let sk = PrivateKey::from_bytes(&parse_secret(&secret)?)
                .map_err(|e| e.to_string())?;
            let kind = kind.to_ascii_lowercase();
            let kinds: Vec<&str> = match kind.as_str() {
                "all" => vec!["p2pkh", "p2wpkh", "p2tr"],
                "p2pkh" | "p2wpkh" | "p2tr" => vec![&kind],
                other => return Err(format!("tipo de endereço desconhecido: {other}")),
            };
            for k in kinds {
                let address = match k {
                    "p2pkh" => sk.to_p2pkh_address(network).map_err(|e| e.to_string())?,
                    "p2wpkh" => sk.to_p2wpkh_address(network).map_err(|e| e.to_string())?,
                    _ => sk.to_p2tr_address(network).map_err(|e| e.to_string())?,
                };
                let out = AddressOutput {
                    kind: k.to_string(),
                    address,
                };
                println!("{}", serde_json::to_string(&out).unwrap());
            }
        }
        Command::Wif { secret, network } => {
            let network = parse_network(&network)?;
            let sk = PrivateKey::from_bytes(&parse_secret(&secret)?)
                .map_err(|e| e.to_string())?;
            println!("{}", sk.to_wif(network).map_err(|e| e.to_string())?);
        }
        Command::Sign { secret, message } => {
            let sk = PrivateKey::from_bytes(&parse_secret(&secret)?)
                .map_err(|e| e.to_string())?;
            let sig = sk.sign_message(message.as_bytes()).map_err(|e| e.to_string())?;
            let out = SignatureOutput {
                message,
                signature_hex: hex::encode(sig),
                algorithm: "BIP-340 Schnorr".to_string(),
            };
            println!("{}", serde_json::to_string_pretty(&out).unwrap());
        }
        Command::Verify { address, network } => {
            let network = parse_network(&network)?;
            let valid = validate_address(&address, network);
            let out = VerifyOutput {
                address,
                network: format!("{network:?}"),
                valid,
            };
            println!("{}", serde_json::to_string_pretty(&out).unwrap());
            if !valid {
                return Err(String::new());
            }
        }
        Command::Receipt {
            secret,
            address,
            nonce,
        } => {
            #[cfg(feature = "pott")]
            {
                let sk = PrivateKey::from_bytes(&parse_secret(&secret)?)
                    .map_err(|e| e.to_string())?;
                let nonce = parse_nonce(&nonce)?;
                let receipt =
                    arkhe_bitcoin::pott_integration::create_pott_receipt_for_address(
                        &sk, &address, &nonce,
                    )
                    .map_err(|e| e.to_string())?;
                let out = ReceiptOutput {
                    h: hex::encode(receipt.h),
                    nu: hex::encode(receipt.nu),
                    node: hex::encode(receipt.node),
                    tin: receipt.tin,
                    tout: receipt.tout,
                    sig: hex::encode(receipt.sig),
                };
                println!("{}", serde_json::to_string_pretty(&out).unwrap());
            }
            #[cfg(not(feature = "pott"))]
            {
                // Só erro quando o CLI é construído sem a feature — nunca em
                // tempo de compilação (para que o bin compile nos dois modos).
                let _ = (secret, address, nonce);
                return Err(
                    "receipt requer a feature `pott` (rebuild: --features pott)".to_string(),
                );
            }
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(msg) => {
            if !msg.is_empty() {
                eprintln!("erro: {msg}");
            }
            ExitCode::FAILURE
        }
    }
}