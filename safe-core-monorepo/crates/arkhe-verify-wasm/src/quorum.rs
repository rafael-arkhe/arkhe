//! §2.1 — `verify_witness_quorum`: quórum de testemunhas.

use std::collections::BTreeSet;

use crate::encoding::{decode_fixed_hex, encode_hex, WitnessJson, PUBLIC_KEY_HEX_LEN};
use crate::error::VerifyError;
use crate::signature::verify_with_key;

/// Quórum mínimo aceito.
///
/// Um "quórum de 1" não é quórum: é uma única testemunha, e aceitá-lo
/// tornaria a função indistinguível de [`crate::signature::verify_signature_inner`].
/// Por isso o limiar é validado, em vez de aceito, e um limiar abaixo deste
/// valor falha com [`VerifyError::QuorumBelowMinimum`] mesmo que haja
/// witnesses válidos suficientes.
pub const QUORUM_MIN: u32 = 2;

/// Núcleo compartilhado: devolve o conjunto das chaves de witness distintas
/// que assinaram `subject` validamente e estão no trust root.
fn valid_witness_key_set(
    subject: &[u8],
    witnesses: &[WitnessJson],
    trust_root: &crate::encoding::TrustRoot,
) -> BTreeSet<[u8; PUBLIC_KEY_HEX_LEN]> {
    let mut valid: BTreeSet<[u8; PUBLIC_KEY_HEX_LEN]> = BTreeSet::new();

    for witness in witnesses {
        let Ok(key) = decode_fixed_hex::<PUBLIC_KEY_HEX_LEN>(&witness.public_key_hex) else {
            continue;
        };
        if !trust_root.contains(&key) {
            continue;
        }
        let Ok(signature) = crate::encoding::decode_hex(&witness.signature_hex) else {
            continue;
        };
        if verify_with_key(subject, &signature, &key).is_ok() {
            valid.insert(key);
        }
    }

    valid
}

/// Conta quantos witnesses **distintos** produziram uma assinatura válida
/// sobre `subject` com uma chave presente no trust root.
///
/// Duas propriedades que fazem a contagem valer como quórum:
///
/// 1. A contagem é sobre **chaves distintas** (um `BTreeSet`), não sobre
///    entradas da lista. A mesma testemunha listada duas vezes conta uma vez
///    só — caso contrário uma única chave forjaria um quórum de 2.
/// 2. Se uma chave aparece mais de uma vez, ela conta se **qualquer** uma das
///    cópias tiver assinatura válida; uma primeira cópia inválida seguida de
///    uma válida não é descartada.
///
/// Uma entrada individual malformada (hex inválido, chave fora do trust root,
/// assinatura inválida) simplesmente não conta: a função não falha por causa
/// dela, porque uma lista de witnesses é heterogênea por natureza e o que
/// importa é o total de válidos. O que falha é o JSON da lista inteira, que é
/// [`VerifyError::InvalidJson`].
pub fn count_valid_witnesses(
    subject: &[u8],
    witnesses: &[WitnessJson],
    trust_root: &crate::encoding::TrustRoot,
) -> u32 {
    valid_witness_key_set(subject, witnesses, trust_root).len() as u32
}

/// Confere que ao menos `threshold` witnesses distintos e confiáveis
/// assinaram `subject`.
pub fn verify_witness_quorum_inner(
    subject: &[u8],
    witnesses: &[WitnessJson],
    trust_root: &crate::encoding::TrustRoot,
    threshold: u32,
) -> Result<(), VerifyError> {
    if threshold < QUORUM_MIN {
        return Err(VerifyError::QuorumBelowMinimum {
            threshold,
            minimum: QUORUM_MIN,
        });
    }

    let valid = count_valid_witnesses(subject, witnesses, trust_root);
    if valid < threshold {
        return Err(VerifyError::QuorumNotMet {
            valid,
            required: threshold,
        });
    }
    Ok(())
}

/// Lista de chaves públicas (em hex) que satisfazem o quórum, em ordem
/// determinística — útil para auditar *quem* atestou, não só quantos.
pub fn valid_witness_keys(
    subject: &[u8],
    witnesses: &[WitnessJson],
    trust_root: &crate::encoding::TrustRoot,
) -> Vec<String> {
    valid_witness_key_set(subject, witnesses, trust_root)
        .iter()
        .map(|key| encode_hex(key))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encoding::TrustRoot;
    use ed25519_dalek::{Signer, SigningKey};

    const SUBJECT: &[u8] = b"arkhe-os: subject under quorum";

    fn signing_key(seed: u8) -> SigningKey {
        SigningKey::from_bytes(&[seed; 32])
    }

    fn public_hex(seed: u8) -> String {
        encode_hex(signing_key(seed).verifying_key().as_bytes())
    }

    fn witness(seed: u8, message: &[u8]) -> WitnessJson {
        WitnessJson {
            public_key_hex: public_hex(seed),
            signature_hex: encode_hex(&signing_key(seed).sign(message).to_bytes()),
        }
    }

    fn trust_root(seeds: &[u8]) -> TrustRoot {
        let keys: Vec<String> = seeds.iter().map(|&seed| format!("\"{}\"", public_hex(seed))).collect();
        TrustRoot::parse(&format!("[{}]", keys.join(","))).expect("parse")
    }

    #[test]
    fn two_valid_witnesses_satisfy_a_quorum_of_two() {
        let witnesses = vec![witness(1, SUBJECT), witness(2, SUBJECT)];
        let root = trust_root(&[1, 2]);
        assert_eq!(count_valid_witnesses(SUBJECT, &witnesses, &root), 2);
        assert_eq!(
            verify_witness_quorum_inner(SUBJECT, &witnesses, &root, 2),
            Ok(())
        );
    }

    #[test]
    fn a_single_valid_witness_does_not_satisfy_a_quorum_of_two() {
        let witnesses = vec![witness(1, SUBJECT)];
        let root = trust_root(&[1, 2]);
        assert_eq!(count_valid_witnesses(SUBJECT, &witnesses, &root), 1);
        assert_eq!(
            verify_witness_quorum_inner(SUBJECT, &witnesses, &root, 2),
            Err(VerifyError::QuorumNotMet {
                valid: 1,
                required: 2
            })
        );
    }

    #[test]
    fn the_same_witness_listed_twice_is_not_a_quorum() {
        // A mesma chave, duas entradas, duas assinaturas válidas idênticas:
        // continua sendo uma testemunha.
        let one = witness(1, SUBJECT);
        let witnesses = vec![one.clone(), one];
        let root = trust_root(&[1]);
        assert_eq!(count_valid_witnesses(SUBJECT, &witnesses, &root), 1);
        assert!(verify_witness_quorum_inner(SUBJECT, &witnesses, &root, 2).is_err());
    }

    #[test]
    fn a_duplicated_key_still_counts_if_any_copy_is_valid() {
        // Primeira cópia com assinatura quebrada, segunda válida: a chave
        // conta (uma vez).
        let mut broken = witness(1, SUBJECT);
        broken.signature_hex = encode_hex(&[0u8; 64]);
        let witnesses = vec![broken, witness(1, SUBJECT)];
        let root = trust_root(&[1]);
        assert_eq!(count_valid_witnesses(SUBJECT, &witnesses, &root), 1);
    }

    #[test]
    fn witnesses_outside_the_trust_root_do_not_count() {
        let witnesses = vec![witness(1, SUBJECT), witness(3, SUBJECT)];
        let root = trust_root(&[1, 2]); // a chave 3 não é confiável
        assert_eq!(count_valid_witnesses(SUBJECT, &witnesses, &root), 1);
        assert!(verify_witness_quorum_inner(SUBJECT, &witnesses, &root, 2).is_err());
    }

    #[test]
    fn witnesses_signing_a_different_subject_do_not_count() {
        let witnesses = vec![
            witness(1, SUBJECT),
            witness(2, b"outro subject completamente diferente"),
        ];
        let root = trust_root(&[1, 2]);
        assert_eq!(count_valid_witnesses(SUBJECT, &witnesses, &root), 1);
        assert!(verify_witness_quorum_inner(SUBJECT, &witnesses, &root, 2).is_err());
    }

    #[test]
    fn a_threshold_below_the_minimum_is_rejected_even_with_enough_witnesses() {
        let witnesses = vec![witness(1, SUBJECT), witness(2, SUBJECT)];
        let root = trust_root(&[1, 2]);
        // Há 2 witnesses válidos, mas quórum de 1 não é quórum.
        assert_eq!(
            verify_witness_quorum_inner(SUBJECT, &witnesses, &root, 1),
            Err(VerifyError::QuorumBelowMinimum {
                threshold: 1,
                minimum: 2
            })
        );
        assert_eq!(
            verify_witness_quorum_inner(SUBJECT, &witnesses, &root, 0),
            Err(VerifyError::QuorumBelowMinimum {
                threshold: 0,
                minimum: 2
            })
        );
    }

    #[test]
    fn all_six_witnesses_satisfy_a_quorum_of_five() {
        let witnesses: Vec<WitnessJson> = (1..=6).map(|seed| witness(seed, SUBJECT)).collect();
        let root = trust_root(&[1, 2, 3, 4, 5, 6]);
        assert_eq!(count_valid_witnesses(SUBJECT, &witnesses, &root), 6);
        assert_eq!(
            verify_witness_quorum_inner(SUBJECT, &witnesses, &root, 5),
            Ok(())
        );
    }

    #[test]
    fn malformed_entries_are_skipped_rather_than_aborting_the_count() {
        let witnesses = vec![
            WitnessJson {
                public_key_hex: "não é hex".to_string(),
                signature_hex: "também não".to_string(),
            },
            WitnessJson {
                public_key_hex: public_hex(3), // bem formada, fora do trust root
                signature_hex: encode_hex(&[0u8; 64]),
            },
            WitnessJson {
                public_key_hex: public_hex(1),
                signature_hex: "não é hex".to_string(),
            },
            witness(2, SUBJECT),
        ];
        let root = trust_root(&[1, 2, 3]);
        assert_eq!(count_valid_witnesses(SUBJECT, &witnesses, &root), 1);
    }

    #[test]
    fn an_empty_witness_list_never_reaches_quorum() {
        let root = trust_root(&[1, 2]);
        assert_eq!(count_valid_witnesses(SUBJECT, &[], &root), 0);
        assert!(verify_witness_quorum_inner(SUBJECT, &[], &root, 2).is_err());
    }

    #[test]
    fn valid_witness_keys_reports_who_attested() {
        let witnesses = vec![witness(1, SUBJECT), witness(2, SUBJECT), witness(3, SUBJECT)];
        let root = trust_root(&[1, 2, 3]);
        let keys = valid_witness_keys(SUBJECT, &witnesses, &root);
        assert_eq!(keys.len(), 3);
        assert!(keys.contains(&public_hex(1)));
        assert!(keys.contains(&public_hex(2)));
        assert!(keys.contains(&public_hex(3)));
    }
}
