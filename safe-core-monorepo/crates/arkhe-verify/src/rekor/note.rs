//! O checkpoint assinado (uma *signed note*, na convenção do Certificate
//! Transparency) e a conversão dele para o que o core consome.
//!
//! # O formato
//!
//! ```text
//! <origem> - <id curto>
//! <tree_size>
//! <raiz, em base64>
//! <linha em branco>
//! — <nome do witness> <base64(key id de 4 bytes ∥ assinatura)>
//! — <nome do witness> <base64(key id de 4 bytes ∥ assinatura)>
//! ```
//!
//! Duas coisas a registrar, porque não foram verificadas contra material real
//! (ver o README):
//!
//! 1. **O que é assinado** é o corpo da note — as três linhas de conteúdo,
//!    incluindo o `\n` que termina a última — e não o documento inteiro.
//!    [`Checkpoint::note_body`] guarda exatamente esses bytes, e é o `subject`
//!    que vai para o `verify_witness_quorum` do core.
//! 2. **O `key id` viaja concatenado à assinatura**, os dois dentro do mesmo
//!    base64: os 4 primeiros bytes são o `key id`, o resto é a assinatura. É a
//!    leitura da convenção do pacote `note` do Go.
//!
//! O modo de falha da leitura 2 é seguro: um formato diferente produz um
//! `key id` que não está no [`WitnessKeyring`], e portanto
//! [`RekorError::UnknownWitnessKey`] — não um veredito errado.
//!
//! # O `key id` não é a chave
//!
//! Uma note identifica o witness pelo `key id` de 4 bytes, não pela chave
//! pública. Resolver um no outro é responsabilidade de quem chama, via
//! [`WitnessKeyring`]: em CT o de-para vive numa lista de chaves distribuída
//! fora da note, e inventar uma resolução aqui seria inventar uma raiz de
//! confiança.

use std::collections::BTreeMap;

use arkhe_verify_wasm::encoding::{
    decode_base64, decode_fixed_hex, encode_hex, PUBLIC_KEY_HEX_LEN,
};
use arkhe_verify_wasm::{TrustRoot, WitnessJson};
use serde::Serialize;

use crate::error::RekorError;
use crate::report::QuorumReport;

/// Tamanho do `key id` de um witness, em bytes.
pub const KEY_ID_LEN: usize = 4;

/// O separador entre o corpo da note e as linhas de assinatura.
const BODY_SEPARATOR: &str = "\n\n";

/// O prefixo de uma linha de assinatura: travessão em dash seguido de espaço.
const SIGNATURE_PREFIX: &str = "\u{2014} ";

/// O checkpoint de um log de transparência: origem, tamanho e raiz.
///
/// Serializa, mas **não** desserializa: um `Checkpoint` só nasce de
/// [`parse_signed_checkpoint`], onde a raiz é lida do mesmo texto que as
/// assinaturas cobrem. Permitir construí-lo por desserialização permitiria um
/// `Checkpoint` cujo campo `root_hash` não corresponde ao do
/// [`Checkpoint::note_body`] — um estado inconsistente que o parser existe para
/// tornar impossível.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Checkpoint {
    /// A primeira linha: a origem (ex.: `rekor.sigstore.dev - 26057336`).
    pub origin: String,
    /// A segunda linha: o número de folhas da árvore.
    pub tree_size: u64,
    /// A terceira linha: a raiz, **em base64** (convenção de *signed note*).
    ///
    /// Note que o `root_hash` de uma [`crate::rekor::InclusionProof`] é hex.
    /// São dois formatos da mesma API.
    pub root_hash: String,
    /// O texto exato que as assinaturas cobrem — o `subject` do quórum.
    ///
    /// Inclui o `\n` final da última linha do corpo, e **não** inclui a linha
    /// em branco nem as linhas de assinatura.
    pub note_body: String,
}

impl Checkpoint {
    /// A raiz, decodificada do base64 da note.
    pub fn root_hash_bytes(&self) -> Result<Vec<u8>, RekorError> {
        decode_base64(&self.root_hash).map_err(crate::rekor::encoding_error)
    }
}

/// A assinatura de um witness sobre o corpo de uma note.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WitnessSignature {
    /// O `key id` do witness: os 4 bytes que identificam a chave na note.
    pub key_id: [u8; KEY_ID_LEN],
    /// Os bytes da assinatura (o que sobra depois dos 4 bytes de `key id`).
    pub signature: Vec<u8>,
}

impl WitnessSignature {
    /// O `key id` em hex — a forma usada nas chaves do [`WitnessKeyring`] e nas
    /// mensagens de erro.
    pub fn key_id_hex(&self) -> String {
        encode_hex(&self.key_id)
    }
}

/// Um checkpoint com as assinaturas dos witnesses.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SignedCheckpoint {
    /// O checkpoint assinado.
    pub checkpoint: Checkpoint,
    /// As assinaturas dos witnesses, na ordem em que aparecem na note.
    pub witnesses: Vec<WitnessSignature>,
}

impl SignedCheckpoint {
    /// Quantos witnesses assinaram.
    ///
    /// É a contagem de **assinaturas**, não de witnesses distintos: a note
    /// pode, em tese, trazer a mesma chave duas vezes. Quem deduplica chaves
    /// distintas é o `verify_witness_quorum` do core, que conta chaves — e é
    /// por isso que a contagem bruta daqui não é um quórum.
    pub fn witness_count(&self) -> usize {
        self.witnesses.len()
    }

    /// A raiz do checkpoint, decodificada do base64.
    pub fn root_hash_bytes(&self) -> Result<Vec<u8>, RekorError> {
        self.checkpoint.root_hash_bytes()
    }

    /// Monta os argumentos do `verify_witness_quorum` do core: o `subject` (o
    /// corpo da note) e os witnesses no formato que ele consome.
    ///
    /// Falha em [`RekorError::UnknownWitnessKey`] se o `key id` de alguma
    /// assinatura não estiver no keyring — sem saber qual é a chave não há como
    /// verificar a assinatura, e pular a assinatura silenciosamente rebaixaria o
    /// quórum sem avisar.
    pub fn quorum_input(
        &self,
        keyring: &WitnessKeyring,
    ) -> Result<(Vec<u8>, Vec<WitnessJson>), RekorError> {
        let subject = self.checkpoint.note_body.clone().into_bytes();

        let mut witnesses = Vec::with_capacity(self.witnesses.len());
        for witness in &self.witnesses {
            let public_key_hex = keyring.public_key_hex(&witness.key_id).ok_or_else(|| {
                RekorError::UnknownWitnessKey {
                    key_id: witness.key_id_hex(),
                }
            })?;
            witnesses.push(WitnessJson {
                public_key_hex: public_key_hex.to_string(),
                signature_hex: encode_hex(&witness.signature),
            });
        }
        Ok((subject, witnesses))
    }

    /// Verifica o quórum dos witnesses sobre este checkpoint.
    ///
    /// Delega a [`crate::verify_witness_quorum`] — o mesmo core que a casca wasm
    /// chama, com as mesmas garantias (quórum mínimo de 2, contagem por chave
    /// distinta, chave precisa estar no trust root).
    pub fn quorum_report(
        &self,
        keyring: &WitnessKeyring,
        trust_root: &TrustRoot,
        threshold: u32,
    ) -> Result<QuorumReport, RekorError> {
        let (subject, witnesses) = self.quorum_input(keyring)?;
        Ok(crate::verify_witness_quorum(
            &subject,
            &witnesses,
            trust_root,
            threshold,
        ))
    }
}

/// O de-para entre o `key id` de 4 bytes de um witness e a sua chave pública
/// Ed25519.
///
/// A chave pública **não** está na note; este mapa é a raiz de confiança de
/// quem chama, e é deliberadamente explícita. O `key id` é indexado em **hex**
/// (8 caracteres), pela mesma convenção do resto da crate: hex para valores de
/// tamanho fixo e curto. A chave pública é o hex de 32 bytes que o trust root
/// do core já usa.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WitnessKeyring {
    keys: BTreeMap<[u8; KEY_ID_LEN], String>,
}

impl WitnessKeyring {
    /// Um keyring vazio — que não resolve nenhum witness.
    pub fn new() -> Self {
        Self::default()
    }

    /// Interpreta um keyring de um objeto JSON `{"<key id hex>": "<chave hex>"}`.
    ///
    /// Valida os dois lados: um `key id` que não seja 4 bytes, ou uma chave que
    /// não seja 32 bytes, falha aqui em vez de virar uma falha confusa na hora
    /// de verificar.
    pub fn from_json(json: &str) -> Result<Self, RekorError> {
        let raw: BTreeMap<String, String> =
            serde_json::from_str(json).map_err(|err| RekorError::UnexpectedResponse {
                reason: format!("o keyring não é um objeto JSON de strings: {err}"),
            })?;

        let mut keyring = Self::new();
        for (key_id_hex, public_key_hex) in raw {
            keyring.insert(&key_id_hex, public_key_hex)?;
        }
        Ok(keyring)
    }

    /// Registra a chave pública Ed25519 de um witness.
    pub fn insert(
        &mut self,
        key_id_hex: &str,
        public_key_hex: impl Into<String>,
    ) -> Result<(), RekorError> {
        let key_id = decode_fixed_hex::<KEY_ID_LEN>(key_id_hex)
            .map_err(|err| RekorError::Encoding(format!("key id `{key_id_hex}`: {err}")))?;

        let public_key_hex = public_key_hex.into();
        decode_fixed_hex::<PUBLIC_KEY_HEX_LEN>(&public_key_hex)
            .map_err(|err| RekorError::Encoding(format!("chave `{public_key_hex}`: {err}")))?;

        self.keys.insert(key_id, public_key_hex);
        Ok(())
    }

    /// A chave pública registrada para este `key id`, se houver.
    pub fn public_key_hex(&self, key_id: &[u8; KEY_ID_LEN]) -> Option<&str> {
        self.keys.get(key_id).map(String::as_str)
    }

    /// Quantas chaves o keyring conhece.
    pub fn len(&self) -> usize {
        self.keys.len()
    }

    /// `true` se o keyring não conhece nenhuma chave.
    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }
}

/// Interpreta uma note assinada.
///
/// Ver a nota de formato no topo do módulo para as duas leituras da convenção
/// que isto assume.
pub fn parse_signed_checkpoint(note: &str) -> Result<SignedCheckpoint, RekorError> {
    let separator = note.find(BODY_SEPARATOR).ok_or_else(|| {
        RekorError::MalformedCheckpoint {
            reason: "sem a linha em branco que separa o corpo das assinaturas".to_string(),
        }
    })?;

    // O corpo inclui o `\n` que termina a sua última linha — é o que as
    // assinaturas cobrem.
    let note_body = &note[..separator + 1];
    let signatures = &note[separator + BODY_SEPARATOR.len()..];

    let mut lines = note_body.trim_end_matches('\n').split('\n');
    let origin = lines.next().unwrap_or_default().to_string();
    let tree_size_line = lines.next().ok_or_else(|| RekorError::MalformedCheckpoint {
        reason: "o corpo não tem a linha do tree_size".to_string(),
    })?;
    let root_hash = lines.next().ok_or_else(|| RekorError::MalformedCheckpoint {
        reason: "o corpo não tem a linha da raiz".to_string(),
    })?;
    if lines.next().is_some() {
        return Err(RekorError::MalformedCheckpoint {
            reason: "o corpo tem mais de três linhas".to_string(),
        });
    }

    let tree_size = tree_size_line
        .trim()
        .parse::<u64>()
        .map_err(|err| RekorError::MalformedCheckpoint {
            reason: format!("tree_size `{tree_size_line}` não é um número: {err}"),
        })?;

    let mut witnesses = Vec::new();
    for line in signatures.lines() {
        let line = line.trim_end_matches('\r');
        if line.trim().is_empty() {
            continue;
        }
        witnesses.push(parse_signature_line(line)?);
    }
    if witnesses.is_empty() {
        return Err(RekorError::MalformedCheckpoint {
            reason: "nenhuma linha de assinatura".to_string(),
        });
    }

    Ok(SignedCheckpoint {
        checkpoint: Checkpoint {
            origin,
            tree_size,
            root_hash: root_hash.to_string(),
            note_body: note_body.to_string(),
        },
        witnesses,
    })
}

/// `— <nome> <base64(key id ∥ assinatura)>`.
///
/// O nome pode conter espaços, então o blob é o **último** token — daí o
/// `rsplit_once`.
fn parse_signature_line(line: &str) -> Result<WitnessSignature, RekorError> {
    let rest = line
        .strip_prefix(SIGNATURE_PREFIX)
        .ok_or_else(|| RekorError::MalformedCheckpoint {
            reason: format!("linha de assinatura sem o prefixo `{SIGNATURE_PREFIX}`: `{line}`"),
        })?;

    let (name, blob) = rest.rsplit_once(' ').ok_or_else(|| RekorError::MalformedCheckpoint {
        reason: format!("linha de assinatura sem nome e blob separados: `{line}`"),
    })?;
    if name.trim().is_empty() {
        return Err(RekorError::MalformedCheckpoint {
            reason: format!("linha de assinatura sem nome: `{line}`"),
        });
    }

    let decoded = decode_base64(blob).map_err(crate::rekor::encoding_error)?;
    if decoded.len() <= KEY_ID_LEN {
        return Err(RekorError::MalformedCheckpoint {
            reason: format!(
                "o blob de `{name}` tem {} bytes: curto demais para {KEY_ID_LEN} de key id mais assinatura",
                decoded.len()
            ),
        });
    }

    let (key_id, signature) = decoded.split_at(KEY_ID_LEN);
    let mut fixed = [0u8; KEY_ID_LEN];
    fixed.copy_from_slice(key_id);

    Ok(WitnessSignature {
        key_id: fixed,
        signature: signature.to_vec(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::engine::general_purpose::STANDARD as BASE64;
    use base64::Engine;
    use ed25519_dalek::{Signer, SigningKey};

    const ORIGIN: &str = "rekor.sigstore.dev - 26057336";

    fn signing_key(seed: u8) -> SigningKey {
        SigningKey::from_bytes(&[seed; 32])
    }

    fn public_hex(seed: u8) -> String {
        encode_hex(signing_key(seed).verifying_key().as_bytes())
    }

    fn key_id(seed: u8) -> [u8; KEY_ID_LEN] {
        [seed, seed, seed, seed]
    }

    /// Uma note no formato documentado, com as assinaturas dos seeds dados.
    fn signed_note(root: &[u8], tree_size: u64, seeds: &[u8]) -> String {
        let body = format!("{ORIGIN}\n{tree_size}\n{}\n", BASE64.encode(root));
        let mut note = String::from(&body);
        note.push('\n');
        for &seed in seeds {
            let signature = signing_key(seed).sign(body.as_bytes()).to_bytes();
            let mut blob = key_id(seed).to_vec();
            blob.extend_from_slice(&signature);
            note.push_str(&format!(
                "{SIGNATURE_PREFIX}witness-{seed} {}\n",
                BASE64.encode(&blob)
            ));
        }
        note
    }

    fn trust_root(seeds: &[u8]) -> TrustRoot {
        let keys: Vec<String> = seeds
            .iter()
            .map(|&seed| format!("\"{}\"", public_hex(seed)))
            .collect();
        TrustRoot::parse(&format!("[{}]", keys.join(","))).expect("parse")
    }

    fn keyring(seeds: &[u8]) -> WitnessKeyring {
        let mut keyring = WitnessKeyring::new();
        for &seed in seeds {
            keyring
                .insert(&encode_hex(&key_id(seed)), public_hex(seed))
                .expect("insere");
        }
        keyring
    }

    // --- parsing -----------------------------------------------------------

    #[test]
    fn a_signed_note_parses_into_its_parts() {
        let root = [0x11u8; 32];
        let signed = parse_signed_checkpoint(&signed_note(&root, 8, &[1, 2])).expect("parse");

        assert_eq!(signed.checkpoint.origin, ORIGIN);
        assert_eq!(signed.checkpoint.tree_size, 8);
        assert_eq!(signed.checkpoint.root_hash, BASE64.encode(root));
        assert_eq!(signed.root_hash_bytes().expect("decodifica"), root);
        assert_eq!(signed.witness_count(), 2);
        assert_eq!(signed.witnesses[0].key_id, key_id(1));
        assert_eq!(signed.witnesses[0].key_id_hex(), encode_hex(&key_id(1)));
        assert_eq!(signed.witnesses[1].key_id, key_id(2));
    }

    #[test]
    fn the_signed_subject_is_the_body_including_its_trailing_newline() {
        let signed = parse_signed_checkpoint(&signed_note(&[0u8; 32], 1, &[1])).expect("parse");

        let expected = format!("{ORIGIN}\n1\n{}\n", BASE64.encode([0u8; 32]));
        assert_eq!(signed.checkpoint.note_body, expected);
        assert!(signed.checkpoint.note_body.ends_with('\n'));
        assert!(!signed.checkpoint.note_body.contains(SIGNATURE_PREFIX));
    }

    #[test]
    fn a_note_without_a_blank_line_is_rejected() {
        assert!(matches!(
            parse_signed_checkpoint("sem separador nenhum"),
            Err(RekorError::MalformedCheckpoint { .. })
        ));
    }

    #[test]
    fn a_note_without_signatures_is_rejected() {
        let note = format!("{ORIGIN}\n8\n{}\n\n", BASE64.encode([0u8; 32]));
        assert!(matches!(
            parse_signed_checkpoint(&note),
            Err(RekorError::MalformedCheckpoint { .. })
        ));
    }

    #[test]
    fn a_note_with_a_non_numeric_tree_size_is_rejected() {
        let note = format!(
            "{ORIGIN}\noito\n{}\n\n{SIGNATURE_PREFIX}w {}\n",
            BASE64.encode([0u8; 32]),
            BASE64.encode([0u8; 68])
        );
        assert!(matches!(
            parse_signed_checkpoint(&note),
            Err(RekorError::MalformedCheckpoint { .. })
        ));
    }

    #[test]
    fn a_body_with_four_lines_is_rejected() {
        let note = format!(
            "{ORIGIN}\n8\n{}\nquarta\n\n{SIGNATURE_PREFIX}w {}\n",
            BASE64.encode([0u8; 32]),
            BASE64.encode([0u8; 68])
        );
        assert!(matches!(
            parse_signed_checkpoint(&note),
            Err(RekorError::MalformedCheckpoint { .. })
        ));
    }

    #[test]
    fn a_signature_line_without_the_dash_prefix_is_rejected() {
        let note = format!(
            "{ORIGIN}\n8\n{}\n\nsem travessao {}\n",
            BASE64.encode([0u8; 32]),
            BASE64.encode([0u8; 68])
        );
        assert!(matches!(
            parse_signed_checkpoint(&note),
            Err(RekorError::MalformedCheckpoint { .. })
        ));
    }

    #[test]
    fn a_signature_blob_too_short_for_a_key_id_is_rejected() {
        let note = format!(
            "{ORIGIN}\n8\n{}\n\n{SIGNATURE_PREFIX}w {}\n",
            BASE64.encode([0u8; 32]),
            BASE64.encode([0u8; KEY_ID_LEN])
        );
        assert!(matches!(
            parse_signed_checkpoint(&note),
            Err(RekorError::MalformedCheckpoint { .. })
        ));
    }

    #[test]
    fn a_blob_that_is_not_base64_is_an_encoding_error() {
        let note = format!(
            "{ORIGIN}\n8\n{}\n\n{SIGNATURE_PREFIX}w !!!não é base64!!!\n",
            BASE64.encode([0u8; 32])
        );
        assert!(matches!(
            parse_signed_checkpoint(&note),
            Err(RekorError::Encoding(_))
        ));
    }

    // --- keyring -----------------------------------------------------------

    #[test]
    fn a_keyring_parses_validates_and_resolves() {
        let json = format!(
            r#"{{"{}":"{}","{}":"{}"}}"#,
            encode_hex(&key_id(1)),
            public_hex(1),
            encode_hex(&key_id(2)),
            public_hex(2)
        );
        let keyring = WitnessKeyring::from_json(&json).expect("parse");
        assert_eq!(keyring.len(), 2);
        assert!(!keyring.is_empty());
        assert_eq!(keyring.public_key_hex(&key_id(1)), Some(public_hex(1).as_str()));
        assert_eq!(keyring.public_key_hex(&key_id(9)), None);
    }

    #[test]
    fn a_keyring_rejects_a_short_key_id_or_a_short_key() {
        assert!(matches!(
            WitnessKeyring::from_json(r#"{"aabb":"00"}"#),
            Err(RekorError::Encoding(_))
        ));
        assert!(matches!(
            WitnessKeyring::from_json(&format!(r#"{{"{}":"00"}}"#, encode_hex(&key_id(1)))),
            Err(RekorError::Encoding(_))
        ));
        assert!(matches!(
            WitnessKeyring::from_json("não é json"),
            Err(RekorError::UnexpectedResponse { .. })
        ));
    }

    #[test]
    fn an_empty_keyring_resolves_nothing() {
        let keyring = WitnessKeyring::new();
        assert!(keyring.is_empty());
        assert_eq!(keyring.len(), 0);
        assert_eq!(keyring.public_key_hex(&key_id(1)), None);
    }

    // --- quórum ------------------------------------------------------------

    #[test]
    fn a_checkpoint_with_two_witnesses_satisfies_a_quorum_of_two() {
        let signed = parse_signed_checkpoint(&signed_note(&[0x22; 32], 8, &[1, 2])).expect("parse");
        let report = signed
            .quorum_report(&keyring(&[1, 2]), &trust_root(&[1, 2]), 2)
            .expect("monta");

        assert!(report.ok, "erro: {:?}", report.error);
        assert_eq!(report.valid_witnesses, 2);
        assert_eq!(report.required, 2);
    }

    #[test]
    fn a_checkpoint_with_one_witness_does_not_satisfy_a_quorum_of_two() {
        let signed = parse_signed_checkpoint(&signed_note(&[0x22; 32], 8, &[1])).expect("parse");
        let report = signed
            .quorum_report(&keyring(&[1, 2]), &trust_root(&[1, 2]), 2)
            .expect("monta");

        assert!(!report.ok);
        assert_eq!(report.valid_witnesses, 1);
        assert!(report.error.expect("causa").contains("quórum"));
    }

    #[test]
    fn a_witness_outside_the_trust_root_does_not_count() {
        let signed = parse_signed_checkpoint(&signed_note(&[0x22; 32], 8, &[1, 2])).expect("parse");
        // O keyring conhece as duas chaves, mas o trust root só confia na 1.
        let report = signed
            .quorum_report(&keyring(&[1, 2]), &trust_root(&[1]), 2)
            .expect("monta");

        assert!(!report.ok);
        assert_eq!(report.valid_witnesses, 1);
    }

    #[test]
    fn a_witness_whose_key_id_is_unknown_cannot_be_verified() {
        let signed = parse_signed_checkpoint(&signed_note(&[0x22; 32], 8, &[1, 2])).expect("parse");
        let report = signed.quorum_report(&keyring(&[1]), &trust_root(&[1, 2]), 2);

        assert_eq!(
            report,
            Err(RekorError::UnknownWitnessKey {
                key_id: encode_hex(&key_id(2))
            }),
            "sem saber qual é a chave, a assinatura não é verificada nem pulada"
        );
    }

    #[test]
    fn a_signature_over_a_different_body_does_not_count() {
        // As duas notes têm corpos diferentes, então a assinatura da primeira
        // não vale para o corpo da segunda, mesmo com a mesma chave.
        let signed = parse_signed_checkpoint(&signed_note(&[0x22; 32], 8, &[1, 2])).expect("parse");
        let mut tampered = signed.clone();
        tampered.checkpoint.note_body = format!("{ORIGIN}\n9\n{}\n", BASE64.encode([0x22u8; 32]));

        let report = tampered
            .quorum_report(&keyring(&[1, 2]), &trust_root(&[1, 2]), 2)
            .expect("monta");
        assert!(!report.ok);
        assert_eq!(report.valid_witnesses, 0);
    }

    #[test]
    fn a_threshold_below_the_minimum_is_rejected_by_the_core() {
        let signed = parse_signed_checkpoint(&signed_note(&[0x22; 32], 8, &[1, 2])).expect("parse");
        let report = signed
            .quorum_report(&keyring(&[1, 2]), &trust_root(&[1, 2]), 1)
            .expect("monta");

        assert!(!report.ok);
        assert_eq!(report.valid_witnesses, 2, "a contagem sobrevive ao limiar inválido");
        assert!(report.error.expect("causa").contains("mínimo"));
    }

    #[test]
    fn the_quorum_input_is_the_body_and_the_hex_encoded_signatures() {
        let signed = parse_signed_checkpoint(&signed_note(&[0x22; 32], 8, &[1])).expect("parse");
        let (subject, witnesses) = signed.quorum_input(&keyring(&[1])).expect("monta");

        assert_eq!(subject, signed.checkpoint.note_body.as_bytes());
        assert_eq!(witnesses.len(), 1);
        assert_eq!(witnesses[0].public_key_hex, public_hex(1));
        assert_eq!(witnesses[0].signature_hex, encode_hex(&signed.witnesses[0].signature));
    }
}
