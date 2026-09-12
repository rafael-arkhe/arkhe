//! Validação e parsing de endereços Bitcoin.

use std::str::FromStr;

use bitcoin::Address as BitcoinAddress;

use crate::error::{BitcoinError, Result};
use crate::network::Network;

/// Verifica se uma string é um endereço Bitcoin válido **para a rede fornecida**.
///
/// Retorna `false` para strings inválidas ou para redes diferentes.
pub fn validate_address(address: &str, network: Network) -> bool {
    BitcoinAddress::from_str(address)
        .map(|addr| addr.is_valid_for_network(network.to_bitcoin_network()))
        .unwrap_or(false)
}

/// Converte uma string para o tipo `Address` do crate `bitcoin`, exigindo que
/// a rede do endereço coincida com `network`.
pub fn parse_address(address: &str, network: Network) -> Result<bitcoin::Address> {
    let addr = BitcoinAddress::from_str(address)
        .map_err(|_| BitcoinError::InvalidAddress(address.to_owned()))?;
    if !addr.is_valid_for_network(network.to_bitcoin_network()) {
        return Err(BitcoinError::NetworkMismatch {
            expected: format!("{network:?}"),
            actual: describe_network(&addr),
        });
    }
    Ok(addr.assume_checked())
}

/// Nome da rede de um endereço (para mensagens de erro; o enum não expõe
/// diretamente o campo `network`).
fn describe_network(addr: &bitcoin::Address<bitcoin::address::NetworkUnchecked>) -> String {
    [
        (bitcoin::Network::Bitcoin, "Bitcoin"),
        (bitcoin::Network::Testnet, "Testnet"),
        (bitcoin::Network::Regtest, "Regtest"),
        (bitcoin::Network::Signet, "Signet"),
    ]
    .into_iter()
    .find(|(n, _)| addr.is_valid_for_network(*n))
    .map(|(_, name)| name)
    .unwrap_or("Unknown/Invalid")
    .to_owned()
}

/// Gera um endereço P2WPKH aleatório (util para testes e doações).
pub fn generate_dummy_address(network: Network) -> String {
    let sk = crate::key::PrivateKey::generate();
    sk.to_p2wpkh_address(network)
        .unwrap_or_else(|_| String::from("invalid key"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_address() {
        // Vetor da doc oficial (mainnet P2WPKH).
        let valid = "bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq";
        assert!(validate_address(valid, Network::Bitcoin));
        // Último caractere adulterado → checksum bech32 inválido.
        let invalid = "bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdX";
        assert!(!validate_address(invalid, Network::Bitcoin));
        // Rede errada → inválido para a rede pedida.
        assert!(!validate_address(valid, Network::Testnet));
    }

    #[test]
    fn test_parse_address_network_mismatch() {
        let addr = generate_dummy_address(Network::Testnet);
        let parsed = parse_address(&addr, Network::Testnet).unwrap();
        assert_eq!(parsed.to_string(), addr);
        assert!(matches!(
            parse_address(&addr, Network::Bitcoin),
            Err(BitcoinError::NetworkMismatch { .. })
        ));
    }

    #[test]
    fn test_generate_dummy_address() {
        assert!(generate_dummy_address(Network::Bitcoin).starts_with("bc1"));
    }
}