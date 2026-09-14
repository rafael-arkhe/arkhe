//! Decodificação das entradas vindas do JavaScript, e o formato do trust root.
//!
//! # Convenções de codificação
//!
//! A API é exposta ao JavaScript como `String`/`Vec<u8>`/`bool`, então cada
//! tipo binário precisa de uma codificação textual combinada:
//!
//! - **hex** para valores de tamanho fixo e curto: chaves públicas (32
//!   bytes), assinaturas (64), raízes Merkle (32), digests (32) e provas de
//!   inclusão (n × 32).
//! - **base64** (standard, com padding) para o payload, que é arbitrário e
//!   pode ser binário.
//!
//! Ambos os decodificadores aceitam o prefixo `0x` opcional no hex e são
//! insensíveis a maiúsculas/minúsculas.

use std::collections::BTreeSet;

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use serde::{Deserialize, Serialize};

use crate::error::VerifyError;

/// Uma chave pública Ed25519, como o JavaScript a envia em hex.
pub const PUBLIC_KEY_HEX_LEN: usize = 32;

/// Decodifica hex de tamanho fixo, exigindo exatamente `N` bytes.
///
/// O tamanho é verificado aqui, e não no ponto de uso, para que uma chave
/// truncada falhe com [`VerifyError::WrongLength`] em vez de silenciosamente
/// virar outra chave.
pub fn decode_fixed_hex<const N: usize>(value: &str) -> Result<[u8; N], VerifyError> {
    let trimmed = value.strip_prefix("0x").unwrap_or(value);
    let bytes = hex::decode(trimmed).map_err(|err| VerifyError::InvalidHex(err.to_string()))?;
    let actual = bytes.len();
    let fixed: [u8; N] = bytes
        .try_into()
        .map_err(|_| VerifyError::WrongLength {
            expected: N,
            actual,
        })?;
    Ok(fixed)
}

/// Decodifica hex de tamanho variável (provas de inclusão, assinaturas).
pub fn decode_hex(value: &str) -> Result<Vec<u8>, VerifyError> {
    let trimmed = value.strip_prefix("0x").unwrap_or(value);
    hex::decode(trimmed).map_err(|err| VerifyError::InvalidHex(err.to_string()))
}

/// Codifica bytes como hex minúsculo.
pub fn encode_hex(bytes: &[u8]) -> String {
    hex::encode(bytes)
}

/// Decodifica o payload em base64 standard (com padding).
pub fn decode_base64(value: &str) -> Result<Vec<u8>, VerifyError> {
    BASE64
        .decode(value)
        .map_err(|err| VerifyError::InvalidBase64(err.to_string()))
}

/// O conjunto de chaves públicas em que a verificação confia.
///
/// Serializado como um array JSON de strings hex:
///
/// ```json
/// ["1f8f...c2", "a30b...77"]
/// ```
///
/// Um array vazio é aceito e não confia em nada — toda verificação de
/// assinatura falha com [`VerifyError::UntrustedKey`], que é o resultado
/// correto para "nenhuma chave é confiável".
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TrustRoot {
    keys: BTreeSet<[u8; PUBLIC_KEY_HEX_LEN]>,
}

impl TrustRoot {
    /// Interpreta o trust root a partir do JSON.
    pub fn parse(json: &str) -> Result<Self, VerifyError> {
        let raw: Vec<String> =
            serde_json::from_str(json).map_err(|err| VerifyError::InvalidTrustRoot(err.to_string()))?;

        let mut keys = BTreeSet::new();
        for entry in raw {
            let key = decode_fixed_hex::<PUBLIC_KEY_HEX_LEN>(&entry).map_err(|err| {
                VerifyError::InvalidTrustRoot(format!("chave `{entry}`: {err}"))
            })?;
            keys.insert(key);
        }
        Ok(Self { keys })
    }

    /// `true` se `key` está no trust root.
    pub fn contains(&self, key: &[u8; PUBLIC_KEY_HEX_LEN]) -> bool {
        self.keys.contains(key)
    }

    /// Número de chaves confiáveis.
    pub fn len(&self) -> usize {
        self.keys.len()
    }

    /// `true` se o trust root não confia em nenhuma chave.
    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }
}

/// Um witness, como o JavaScript o envia.
///
/// O witness atesta o mesmo *subject* que o atestador primário: o que o
/// distingue é a chave, e é por isso que o quórum conta **chaves distintas**
/// e não entradas da lista — caso contrário a mesma testemunha repetida
/// duas vezes satisfaria sozinha um quórum de 2.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WitnessJson {
    /// Chave pública Ed25519 do witness, em hex.
    pub public_key_hex: String,
    /// Assinatura Ed25519 do *subject*, em hex.
    pub signature_hex: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_hex_round_trips_and_accepts_prefix_and_case() {
        let bytes = [0xabu8; 32];
        let encoded = encode_hex(&bytes);
        assert_eq!(decode_fixed_hex::<32>(&encoded).expect("decodifica"), bytes);
        assert_eq!(
            decode_fixed_hex::<32>(&encoded.to_uppercase()).expect("decodifica"),
            bytes
        );
        assert_eq!(
            decode_fixed_hex::<32>(&format!("0x{encoded}")).expect("decodifica"),
            bytes
        );
    }

    #[test]
    fn fixed_hex_rejects_wrong_length() {
        let short = encode_hex(&[0u8; 31]);
        assert_eq!(
            decode_fixed_hex::<32>(&short),
            Err(VerifyError::WrongLength {
                expected: 32,
                actual: 31
            })
        );
    }

    #[test]
    fn fixed_hex_rejects_non_hex() {
        assert!(matches!(
            decode_fixed_hex::<32>("zzzz"),
            Err(VerifyError::InvalidHex(_))
        ));
        // Comprimento ímpar também é hex inválido.
        assert!(matches!(
            decode_fixed_hex::<32>("abc"),
            Err(VerifyError::InvalidHex(_))
        ));
    }

    #[test]
    fn trust_root_parses_dedupes_and_rejects_malformed() {
        let key_a = encode_hex(&[1u8; 32]);
        let key_b = encode_hex(&[2u8; 32]);

        let root = TrustRoot::parse(&format!("[\"{key_a}\",\"{key_b}\",\"{key_a}\"]")).expect("parse");
        assert_eq!(root.len(), 2, "duplicata não deve contar duas vezes");
        assert!(root.contains(&[1u8; 32]));
        assert!(root.contains(&[2u8; 32]));
        assert!(!root.contains(&[3u8; 32]));

        assert!(TrustRoot::parse("[]").expect("parse").is_empty());
        assert!(matches!(
            TrustRoot::parse("{\"não\":\"é array\"}"),
            Err(VerifyError::InvalidTrustRoot(_))
        ));
        assert!(matches!(
            TrustRoot::parse("[\"curta\"]"),
            Err(VerifyError::InvalidTrustRoot(_))
        ));
        assert!(matches!(
            TrustRoot::parse("não é json"),
            Err(VerifyError::InvalidTrustRoot(_))
        ));
    }

    #[test]
    fn base64_decodes_standard_payloads() {
        assert_eq!(decode_base64("aGVsbG8=").expect("decodifica"), b"hello");
        assert_eq!(decode_base64("").expect("decodifica"), b"");
        assert!(matches!(
            decode_base64("!!!não é base64!!!"),
            Err(VerifyError::InvalidBase64(_))
        ));
    }

    #[test]
    fn witness_json_requires_both_fields() {
        let ok: WitnessJson =
            serde_json::from_str(r#"{"public_key_hex":"aa","signature_hex":"bb"}"#).expect("parse");
        assert_eq!(ok.public_key_hex, "aa");
        assert!(serde_json::from_str::<WitnessJson>(r#"{"public_key_hex":"aa"}"#).is_err());
    }
}
