//! Integração com o substrate 924-POTT-INTERPLANETARY-TRANSPORT (`arkhe-pott`).
//!
//! Gera um *receipt* de custódia (prova-de-trânsito) para um endereço Bitcoin:
//! o digest do endereço é ancorado em TAI e assinado por BIP-340 sob a chave
//! que gerou o endereço, produzindo evidência temporal auditável.
//!
//! Este módulo só compila com a feature `pott` ativa.

use arkhe_pott::receipt::{signing_key_from_secret, xonly_pubkey};
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::{BitcoinError, Result};
use crate::PrivateKey;

/// Digest do endereço (payload genérico → SHA-256, regra PoTT §5.2).
fn address_digest(address: &str) -> [u8; 32] {
    let mut h = [0u8; 32];
    h.copy_from_slice(&Sha256::digest(address.as_bytes()));
    h
}

/// Tempo atual em segundos TAI (epoch CUC 1958-01-01), ancorado por PoTT.
fn now_tai() -> u64 {
    let posix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    arkhe_pott::time::tai_from_posix_i64_to_u64(posix)
}

/// Gera um receipt PoTT assinado para o endereço `address`.
///
/// * `h` = SHA-256 do endereço (payload genérico).
/// * `nu` = nonce de 16 bytes fornecido pelo chamador (mintado uma vez por instância).
/// * `node` = NodeId BIP-340 (x-only) derivado de `private_key`.
/// * Assinatura BIP-340 sobre o CBOR canônico (keys 0–5) — [`arkhe_pott::Receipt`].
///
/// O receipt retornado é auto-verificado antes de sair da função.
pub fn create_pott_receipt_for_address(
    private_key: &PrivateKey,
    address: &str,
    nonce: &[u8; 16],
) -> Result<arkhe_pott::Receipt> {
    let h: [u8; 32] = address_digest(address);
    let nu: [u8; 16] = *nonce;

    // NodeId = chave pública x-only (32 bytes).
    let signing_key = signing_key_from_secret(&private_key.secret);
    let node = xonly_pubkey(&signing_key);

    let tin = now_tai();
    let tout = tin + 60; // janela de trânsito de 1 min (parâmetro J do paper).

    // Cadeia de custódia de um hop: origem assina o endereço.
    let mut chain = arkhe_pott::Chain::new(h, nu);
    chain
        .append_signed(&private_key.secret, node, tin, tout)
        .map_err(|e| BitcoinError::PottError(format!("{e:?}")))?;

    let receipt = chain
        .receipts
        .last()
        .copied()
        .ok_or_else(|| BitcoinError::PottError("empty chain".to_owned()))?;

    // Auto-verificação constitucional (529-…-KERNEL-API): nunca confie numa
    // assinatura que você próprio não verificou.
    if !receipt.verify_schnorr(&node) {
        return Err(BitcoinError::PottError(
            "signature did not verify".to_owned(),
        ));
    }

    tracing::info!(address_hash = %hex_of(&h), node = %hex_of(&node), "pott receipt issued");

    Ok(receipt)
}

/// Representação hex curta de depuração.
fn hex_of(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Network;

    #[test]
    fn test_create_pott_receipt_for_address() {
        let key = crate::key::PrivateKey::generate();
        let address = key.to_p2wpkh_address(Network::Testnet).unwrap();
        let nonce = [0x42u8; 16];

        let receipt = create_pott_receipt_for_address(&key, &address, &nonce)
            .expect("pott receipt");
        // O receipt guarda o digest do endereço.
        assert_eq!(receipt.h, address_digest(&address));
        assert_eq!(receipt.nu, nonce);
        assert!(receipt.tout > receipt.tin);

        // O receipt atravessa o wire canônico e é re-verificado.
        let bytes = receipt.to_wire();
        let decoded = arkhe_pott::Receipt::from_bytes(&bytes)
            .expect("decode canonical CBOR");
        assert_eq!(decoded, receipt);

        // Assinatura verifica sob o NodeId derivado.
        let node = key.pott_node_id().unwrap();
        assert!(decode_verify(&decoded, &node));
    }

    fn decode_verify(r: &arkhe_pott::Receipt, node: &[u8; 32]) -> bool {
        r.verify_schnorr(node)
    }
}