//! Exemplo: gerar chave e endereços Bitcoin (testnet) e, opcionalmente, um
//! receipt PoTT — execute com `--features pott` para a parte 3.
//!
//! ```text
//! cargo run -p arkhe-bitcoin --example generate_address
//! cargo run -p arkhe-bitcoin --example generate_address --features pott
//! ```

use arkhe_bitcoin::{PrivateKey, Network};
use hex::encode;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let network = Network::Testnet;

    // 1. Gerar chave privada.
    let sk = PrivateKey::generate();
    println!("Chave privada (hex): {}", encode(sk.secret));
    println!("Chave privada (WIF): {}", sk.to_wif(network)?);

    // 2. Endereços.
    let p2pkh = sk.to_p2pkh_address(network)?;
    let p2wpkh = sk.to_p2wpkh_address(network)?;
    let p2tr = sk.to_p2tr_address(network)?;

    println!("Endereço P2PKH (legacy):   {p2pkh}");
    println!("Endereço P2WPKH (SegWit):  {p2wpkh}");
    println!("Endereço P2TR (Taproot):   {p2tr}");

    // 3. Validar os endereços na rede alvo.
    for (name, addr) in [
        ("P2PKH", &p2pkh),
        ("P2WPKH", &p2wpkh),
        ("P2TR", &p2tr),
    ] {
        let ok = arkhe_bitcoin::validate_address(addr, network);
        println!("Validação {name}: {ok}");
    }

    // 4. Opcional: receipt PoTT (feature `pott`).
    #[cfg(feature = "pott")]
    {
        let nonce = [42u8; 16];
        let receipt = arkhe_bitcoin::pott_integration::create_pott_receipt_for_address(
            &sk,
            &p2wpkh,
            &nonce,
        )?;
        println!("Receipt PoTT: h={:02x}, tin={}, tout={}", receipt.h[0], receipt.tin, receipt.tout);
    }

    Ok(())
}