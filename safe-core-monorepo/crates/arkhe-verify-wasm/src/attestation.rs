//! §2.1 — `verify_attestation`: o pipeline completo.
//!
//! Compõe as quatro verificações anteriores numa só afirmação:
//!
//! 1. **`sha256`** — `SHA-256(payload)` confere com o digest declarado.
//! 2. **`signature`** — um signatário confiável assinou o *subject*.
//! 3. **`inclusion`** — o payload está na árvore Merkle RFC 6962 sob a raiz
//!    declarada, na posição declarada.
//! 4. **`quorum`** — ao menos `quorum_threshold` witnesses distintos e
//!    confiáveis assinaram o mesmo *subject*.
//!
//! # O subject
//!
//! O que é assinado não é o payload cru, e sim um *subject* que amarra os
//! quatro campos relevantes:
//!
//! ```text
//! "arkhe-attestation/v1" ∥ SHA-256(payload) ∥ raiz Merkle ∥ leaf_index ∥ tree_size
//! ```
//!
//! Cada um dos cinco campos é de tamanho fixo (domínio 19 bytes, digest 32,
//! raiz 32, índices 8 e 8), então a concatenação é livre de ambiguidade sem
//! precisar de prefixos de comprimento: nenhum rearranjo de bytes de campos
//! distintos produz a mesma sequência.
//!
//! Duas decisões deliberadas:
//!
//! - O subject usa o digest **calculado a partir do payload**, não o digest
//!   declarado no JSON. Se os dois divergirem, a etapa `sha256` já falha; usar
//!   o calculado garante que, quando a assinatura verifica, ela cobre os bytes
//!   reais e não um digest que o atacante escolheu.
//! - Assinar o *subject* e não o payload faz a assinatura cobrir também a
//!   **posição** e a **raiz**: de outro modo a mesma assinatura valeria para
//!   uma prova de inclusão diferente, e a atestação não diria onde o artifact
//!   está no log.
//!
//! # Formato do JSON
//!
//! ```json
//! {
//!   "payload_b64": "aGVsbG8=",
//!   "payload_sha256_hex": "2cf2...e0c",
//!   "merkle_leaf_index": 3,
//!   "merkle_tree_size": 8,
//!   "merkle_proof_hex": "aabb...",
//!   "merkle_root_hex": "ccdd...",
//!   "signer_public_key_hex": "1f8f...c2",
//!   "signature_hex": "9a1b...",
//!   "witnesses": [{ "public_key_hex": "...", "signature_hex": "..." }],
//!   "quorum_threshold": 2
//! }
//! ```
//!
//! `witnesses` é opcional na desserialização (default `[]`), o que só é útil
//! para que uma atestação sem quórum seja **avaliada** e reporte `quorum:
//! false` em vez de falhar na desserialização.

use serde::{Deserialize, Serialize};

use crate::encoding::{decode_base64, decode_fixed_hex, decode_hex, TrustRoot, WitnessJson};
use crate::error::VerifyError;
use crate::hash::{sha256_bytes, sha256_hex, verify_sha256_inner, SHA256_LEN};
use crate::merkle::verify_inclusion_inner;
use crate::quorum::verify_witness_quorum_inner;
use crate::signature::verify_signature_inner;

/// Separador de domínio do subject da atestação.
pub const ATTESTATION_DOMAIN: &[u8] = b"arkhe-attestation/v1";

/// A atestação, como o JavaScript a envia.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attestation {
    /// O payload, em base64 standard.
    pub payload_b64: String,
    /// O digest SHA-256 que o atestador **declara** para o payload, em hex.
    pub payload_sha256_hex: String,
    /// Posição do payload na árvore (0-based).
    pub merkle_leaf_index: u64,
    /// Número de folhas da árvore a que a raiz pertence.
    pub merkle_tree_size: u64,
    /// A prova de inclusão: digests irmãos concatenados, em hex.
    pub merkle_proof_hex: String,
    /// A raiz Merkle RFC 6962, em hex.
    pub merkle_root_hex: String,
    /// A chave pública Ed25519 do signatário, em hex.
    pub signer_public_key_hex: String,
    /// A assinatura Ed25519 do *subject*, em hex.
    pub signature_hex: String,
    /// As testemunhas que atestam o mesmo *subject*.
    #[serde(default)]
    pub witnesses: Vec<WitnessJson>,
    /// Quantas testemunhas distintas são exigidas.
    pub quorum_threshold: u32,
}

/// O resultado do pipeline, estágio por estágio.
///
/// Os quatro estágios são avaliados de forma **independente** e reportados
/// separadamente, mesmo quando um anterior já falhou: um verificador
/// client-side precisa dizer *o que* não bateu, e um curto-circuito no
/// primeiro erro esconderia que, por exemplo, o quórum estava correto e só a
/// prova de inclusão não.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AttestationReport {
    /// `true` somente se **todos** os quatro estágios passaram.
    pub ok: bool,
    /// Estágio 1: o digest declarado confere com o payload.
    pub sha256: bool,
    /// Estágio 2: a assinatura do signatário é válida e a chave é confiável.
    pub signature: bool,
    /// Estágio 3: a prova de inclusão reconstrói a raiz declarada.
    pub inclusion: bool,
    /// Estágio 4: há quórum de witnesses distintos e confiáveis.
    pub quorum: bool,
    /// O primeiro estágio que falhou, em ordem de pipeline — `None` se todos
    /// passaram. Para a entrada malformada, descreve o erro de interpretação.
    pub error: Option<String>,
}

impl AttestationReport {
    /// Um relatório em que tudo falhou, com a causa.
    pub fn rejected(error: impl Into<String>) -> Self {
        Self {
            ok: false,
            sha256: false,
            signature: false,
            inclusion: false,
            quorum: false,
            error: Some(error.into()),
        }
    }
}

/// Constrói o *subject* assinado.
///
/// Ver a nota de formato no topo do módulo: todos os campos têm tamanho fixo,
/// então a concatenação é unívoca.
pub fn attestation_subject(
    payload_digest: &[u8; SHA256_LEN],
    merkle_root: &[u8; SHA256_LEN],
    leaf_index: u64,
    tree_size: u64,
) -> Vec<u8> {
    let mut buf = Vec::with_capacity(ATTESTATION_DOMAIN.len() + SHA256_LEN * 2 + 16);
    buf.extend_from_slice(ATTESTATION_DOMAIN);
    buf.extend_from_slice(payload_digest);
    buf.extend_from_slice(merkle_root);
    buf.extend_from_slice(&leaf_index.to_be_bytes());
    buf.extend_from_slice(&tree_size.to_be_bytes());
    buf
}

/// O subject de uma atestação, em hex — para que um signatário JavaScript
/// consiga montar exatamente os bytes que precisam ser assinados.
///
/// Devolve `Err` se o payload não for base64 válido ou a raiz não for hex de
/// 32 bytes.
pub fn attestation_subject_of(attestation_json: &str) -> Result<(Vec<u8>, Attestation), VerifyError> {
    let attestation: Attestation = serde_json::from_str(attestation_json)
        .map_err(|err| VerifyError::InvalidJson(err.to_string()))?;
    let payload = decode_base64(&attestation.payload_b64)?;
    let root = decode_fixed_hex::<SHA256_LEN>(&attestation.merkle_root_hex)?;
    let digest = sha256_bytes(&payload);
    let subject = attestation_subject(
        &digest,
        &root,
        attestation.merkle_leaf_index,
        attestation.merkle_tree_size,
    );
    Ok((subject, attestation))
}

/// O subject de uma atestação, em hex (string vazia se a entrada for
/// inválida).
pub fn attestation_subject_hex(attestation_json: &str) -> String {
    match attestation_subject_of(attestation_json) {
        Ok((subject, _)) => crate::encoding::encode_hex(&subject),
        Err(_) => String::new(),
    }
}

/// Roda o pipeline completo e devolve o relatório.
///
/// Nunca falha de forma abrupta: uma entrada malformada produz um relatório
/// com `ok: false` e a causa em `error`.
pub fn verify_attestation_inner(attestation_json: &str, trust_root: &TrustRoot) -> AttestationReport {
    let attestation: Attestation = match serde_json::from_str(attestation_json) {
        Ok(attestation) => attestation,
        Err(err) => return AttestationReport::rejected(format!("entrada: JSON inválido: {err}")),
    };

    // --- Estágio 1: sha256 ------------------------------------------------
    let payload = decode_base64(&attestation.payload_b64);
    let sha256_ok = match &payload {
        Ok(payload) => verify_sha256_inner(payload, &attestation.payload_sha256_hex).is_ok(),
        Err(_) => false,
    };

    // O subject usa o digest calculado do payload real; sem payload não há
    // subject, e as etapas que dependem dele ficam `false`.
    let computed_digest = payload.as_ref().ok().map(|payload| sha256_bytes(payload));
    let merkle_root = decode_fixed_hex::<SHA256_LEN>(&attestation.merkle_root_hex).ok();

    let subject = match (computed_digest, merkle_root) {
        (Some(digest), Some(root)) => Some(attestation_subject(
            &digest,
            &root,
            attestation.merkle_leaf_index,
            attestation.merkle_tree_size,
        )),
        _ => None,
    };

    // --- Estágio 2: assinatura do signatário ------------------------------
    let signature_ok = match (&subject, decode_hex(&attestation.signature_hex)) {
        (Some(subject), Ok(signature)) => verify_signature_inner(
            subject,
            &signature,
            &attestation.signer_public_key_hex,
            trust_root,
        )
        .is_ok(),
        _ => false,
    };

    // --- Estágio 3: inclusão Merkle ---------------------------------------
    let inclusion_ok = match (&payload, decode_hex(&attestation.merkle_proof_hex)) {
        (Ok(payload), Ok(proof)) => verify_inclusion_inner(
            payload,
            attestation.merkle_leaf_index,
            attestation.merkle_tree_size,
            &proof,
            &attestation.merkle_root_hex,
        )
        .is_ok(),
        _ => false,
    };

    // --- Estágio 4: quórum de witnesses -----------------------------------
    let quorum_ok = match &subject {
        Some(subject) => verify_witness_quorum_inner(
            subject,
            &attestation.witnesses,
            trust_root,
            attestation.quorum_threshold,
        )
        .is_ok(),
        None => false,
    };

    let ok = sha256_ok && signature_ok && inclusion_ok && quorum_ok;

    // A causa reportada é a do primeiro estágio que falhou, na ordem do
    // pipeline — é o que o usuário precisa corrigir primeiro.
    let error = if ok {
        None
    } else if payload.is_err() {
        Some(format!("payload: base64 inválido: {}", payload.err().map(|err| err.to_string()).unwrap_or_default()))
    } else if merkle_root.is_none() {
        Some(format!("merkle_root_hex inválido: `{}`", attestation.merkle_root_hex))
    } else if !sha256_ok {
        Some(format!(
            "sha256: digest declarado `{}` não confere com o payload (calculado: {})",
            attestation.payload_sha256_hex,
            computed_digest.map(|digest| sha256_hex(&digest)).unwrap_or_default()
        ))
    } else if !signature_ok {
        Some("signature: assinatura do signatário inválida ou chave fora do trust root".to_string())
    } else if !inclusion_ok {
        Some("inclusion: a prova não reconstrói a raiz declarada".to_string())
    } else {
        Some(format!(
            "quorum: {} witnesses distintos e confiáveis, {} exigidos (mínimo {})",
            subject
                .as_deref()
                .map(|subject| crate::quorum::count_valid_witnesses(
                    subject,
                    &attestation.witnesses,
                    trust_root
                ))
                .unwrap_or(0),
            attestation.quorum_threshold,
            crate::quorum::QUORUM_MIN
        ))
    };

    AttestationReport {
        ok,
        sha256: sha256_ok,
        signature: signature_ok,
        inclusion: inclusion_ok,
        quorum: quorum_ok,
        error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encoding::encode_hex;
    use crate::merkle::merkle_root_from_leaves;
    use ct_merkle::mem_backed_tree::MemoryBackedTree;
    use ed25519_dalek::{Signer, SigningKey};
    use sha2::Sha256;

    fn signing_key(seed: u8) -> SigningKey {
        SigningKey::from_bytes(&[seed; 32])
    }

    fn public_hex(seed: u8) -> String {
        encode_hex(signing_key(seed).verifying_key().as_bytes())
    }

    struct Fixture {
        json: String,
        trust_root: String,
    }

    /// Constrói uma atestação **válida** de ponta a ponta: 5 folhas, a folha 2
    /// atestada, assinada pelo seed 1, com 2 witnesses (seeds 2 e 3).
    fn valid_fixture() -> Fixture {
        let leaves: Vec<Vec<u8>> = (0..5)
            .map(|i| format!("artifact-{i}").into_bytes())
            .collect();
        let payload = leaves[2].clone();
        let leaf_index = 2u64;

        let mut tree = MemoryBackedTree::<Sha256, Vec<u8>>::new();
        for leaf in &leaves {
            tree.push(leaf.clone());
        }
        let tree_size = tree.len();
        let root = merkle_root_from_leaves(&leaves);
        let proof = tree.prove_inclusion(leaf_index as usize).as_bytes().to_vec();

        let digest = sha256_bytes(&payload);
        let subject = attestation_subject(&digest, &root, leaf_index, tree_size);

        let signature = signing_key(1).sign(&subject).to_bytes();
        let witnesses: Vec<WitnessJson> = [2u8, 3]
            .iter()
            .map(|&seed| WitnessJson {
                public_key_hex: public_hex(seed),
                signature_hex: encode_hex(&signing_key(seed).sign(&subject).to_bytes()),
            })
            .collect();

        let json = serde_json::json!({
            "payload_b64": base64_encode(&payload),
            "payload_sha256_hex": encode_hex(&digest),
            "merkle_leaf_index": leaf_index,
            "merkle_tree_size": tree_size,
            "merkle_proof_hex": encode_hex(&proof),
            "merkle_root_hex": encode_hex(&root),
            "signer_public_key_hex": public_hex(1),
            "signature_hex": encode_hex(&signature),
            "witnesses": witnesses,
            "quorum_threshold": 2,
        })
        .to_string();

        let trust_root = serde_json::json!([public_hex(1), public_hex(2), public_hex(3)]).to_string();

        Fixture { json, trust_root }
    }

    fn base64_encode(bytes: &[u8]) -> String {
        use base64::engine::general_purpose::STANDARD;
        use base64::Engine;
        STANDARD.encode(bytes)
    }

    fn trust_root_of(fixture: &Fixture) -> TrustRoot {
        TrustRoot::parse(&fixture.trust_root).expect("parse")
    }

    /// Reescreve um campo do JSON da atestação.
    fn with_field(json: &str, field: &str, value: serde_json::Value) -> String {
        let mut value_root: serde_json::Value = serde_json::from_str(json).expect("parse");
        value_root[field] = value;
        value_root.to_string()
    }

    // --- 1: caminho feliz --------------------------------------------------

    #[test]
    fn a_fully_valid_attestation_passes_every_stage() {
        let fixture = valid_fixture();
        let report = verify_attestation_inner(&fixture.json, &trust_root_of(&fixture));
        assert_eq!(
            report,
            AttestationReport {
                ok: true,
                sha256: true,
                signature: true,
                inclusion: true,
                quorum: true,
                error: None,
            }
        );
    }

    #[test]
    fn the_report_serializes_to_the_documented_json_shape() {
        let fixture = valid_fixture();
        let report = verify_attestation_inner(&fixture.json, &trust_root_of(&fixture));
        let value: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&report).expect("serializa"))
                .expect("parse");
        assert_eq!(value["ok"], true);
        assert_eq!(value["sha256"], true);
        assert_eq!(value["signature"], true);
        assert_eq!(value["inclusion"], true);
        assert_eq!(value["quorum"], true);
        assert!(value["error"].is_null());
    }

    // --- 2: estágio sha256 -------------------------------------------------

    #[test]
    fn a_declared_digest_that_does_not_match_the_payload_fails_only_the_sha256_stage() {
        let fixture = valid_fixture();
        // Só o digest **declarado** é reescrito; o payload não muda, então o
        // digest calculado — e portanto o subject assinado — continuam os
        // mesmos. É a isolação exata do estágio 1.
        let json = with_field(
            &fixture.json,
            "payload_sha256_hex",
            serde_json::Value::String("00".repeat(32)),
        );
        let report = verify_attestation_inner(&json, &trust_root_of(&fixture));
        assert_eq!(
            (report.sha256, report.signature, report.inclusion, report.quorum),
            (false, true, true, true)
        );
        assert!(report.error.expect("causa").contains("sha256"));
    }

    // --- 3: estágio signature ----------------------------------------------

    #[test]
    fn a_signature_from_an_untrusted_key_fails_only_the_signature_stage() {
        let fixture = valid_fixture();
        // Chave 9 assina corretamente, mas não está no trust root.
        let json = {
            let (subject, _) = attestation_subject_of(&fixture.json).expect("subject");
            let signature = signing_key(9).sign(&subject).to_bytes();
            let json = with_field(
                &fixture.json,
                "signer_public_key_hex",
                serde_json::Value::String(public_hex(9)),
            );
            with_field(
                &json,
                "signature_hex",
                serde_json::Value::String(encode_hex(&signature)),
            )
        };
        let report = verify_attestation_inner(&json, &trust_root_of(&fixture));
        assert!(!report.ok);
        assert!(report.sha256);
        assert!(!report.signature);
        assert!(report.inclusion);
        assert!(report.quorum, "o quórum não depende do signatário");
    }

    #[test]
    fn a_tampered_signature_fails_only_the_signature_stage() {
        let fixture = valid_fixture();
        let json = with_field(
            &fixture.json,
            "signature_hex",
            serde_json::Value::String(encode_hex(&[0u8; 64])),
        );
        let report = verify_attestation_inner(&json, &trust_root_of(&fixture));
        assert!(!report.ok);
        assert!(!report.signature);
        assert!(report.sha256 && report.inclusion && report.quorum);
    }

    // --- 4: estágio inclusion ---------------------------------------------

    #[test]
    fn a_tampered_proof_fails_only_the_inclusion_stage() {
        let fixture = valid_fixture();
        // A prova não entra no subject, então mexer nela isola o estágio 3.
        let mut proof = crate::encoding::decode_hex(
            serde_json::from_str::<serde_json::Value>(&fixture.json)
                .expect("parse")["merkle_proof_hex"]
                .as_str()
                .expect("string"),
        )
        .expect("hex");
        assert!(!proof.is_empty(), "a prova de 5 folhas não é vazia");
        proof[0] ^= 0x01;

        let json = with_field(
            &fixture.json,
            "merkle_proof_hex",
            serde_json::Value::String(encode_hex(&proof)),
        );
        let report = verify_attestation_inner(&json, &trust_root_of(&fixture));
        assert_eq!(
            (report.sha256, report.signature, report.inclusion, report.quorum),
            (true, true, false, true)
        );
        assert!(report.error.expect("causa").contains("inclusion"));
    }

    #[test]
    fn a_wrong_leaf_index_invalidates_the_proof_and_everything_that_signs_it() {
        let fixture = valid_fixture();
        // O índice entra no subject e na verificação da prova, então mexer
        // nele derruba inclusão, assinatura e quórum — e deixa `sha256`
        // intacto, que é o único estágio que não depende do índice.
        let json = with_field(&fixture.json, "merkle_leaf_index", serde_json::json!(4));
        let report = verify_attestation_inner(&json, &trust_root_of(&fixture));
        assert_eq!(
            (report.sha256, report.signature, report.inclusion, report.quorum),
            (true, false, false, false)
        );
    }

    #[test]
    fn a_payload_outside_the_tree_fails_the_inclusion_stage() {
        let fixture = valid_fixture();
        let json = with_field(
            &fixture.json,
            "payload_b64",
            serde_json::Value::String(base64_encode(b"artifact-forjado")),
        );
        let report = verify_attestation_inner(&json, &trust_root_of(&fixture));
        assert!(!report.ok);
        assert!(!report.sha256, "o digest declarado não bate com o novo payload");
        assert!(!report.inclusion);
    }

    #[test]
    fn a_proof_against_the_wrong_root_fails_the_inclusion_stage() {
        let fixture = valid_fixture();
        let other = encode_hex(&merkle_root_from_leaves(&[
            b"outra".to_vec(),
            b"arvore".to_vec(),
            b"inteira".to_vec(),
        ]));
        let json = with_field(
            &fixture.json,
            "merkle_root_hex",
            serde_json::Value::String(other),
        );
        let report = verify_attestation_inner(&json, &trust_root_of(&fixture));
        assert!(!report.ok);
        assert!(!report.inclusion);
    }

    // --- 5: estágio quorum -------------------------------------------------

    #[test]
    fn a_single_witness_fails_only_the_quorum_stage() {
        let fixture = valid_fixture();
        let json = {
            let mut root: serde_json::Value = serde_json::from_str(&fixture.json).expect("parse");
            let witnesses = root["witnesses"].as_array().expect("array");
            root["witnesses"] = serde_json::Value::Array(vec![witnesses[0].clone()]);
            root["quorum_threshold"] = serde_json::json!(2);
            root.to_string()
        };
        let report = verify_attestation_inner(&json, &trust_root_of(&fixture));
        assert!(!report.ok);
        assert!(report.sha256 && report.signature && report.inclusion);
        assert!(!report.quorum);
    }

    #[test]
    fn a_threshold_above_the_number_of_witnesses_fails_the_quorum_stage() {
        let fixture = valid_fixture();
        let json = with_field(&fixture.json, "quorum_threshold", serde_json::json!(3));
        let report = verify_attestation_inner(&json, &trust_root_of(&fixture));
        assert!(!report.ok);
        assert!(!report.quorum, "só há 2 witnesses");
    }

    #[test]
    fn a_threshold_below_the_minimum_fails_the_quorum_stage() {
        let fixture = valid_fixture();
        let json = with_field(&fixture.json, "quorum_threshold", serde_json::json!(1));
        let report = verify_attestation_inner(&json, &trust_root_of(&fixture));
        assert!(!report.ok);
        assert!(!report.quorum);
        assert!(report.signature && report.inclusion && report.sha256);
    }

    // --- 6: entradas malformadas ------------------------------------------

    #[test]
    fn malformed_json_is_reported_rather_than_panicking() {
        let report = verify_attestation_inner("isto não é json", &TrustRoot::default());
        assert!(!report.ok);
        assert_eq!(
            (report.sha256, report.signature, report.inclusion, report.quorum),
            (false, false, false, false)
        );
        assert!(report.error.expect("causa").contains("JSON inválido"));
    }

    #[test]
    fn a_missing_field_is_reported_as_invalid_json() {
        let fixture = valid_fixture();
        let mut root: serde_json::Value = serde_json::from_str(&fixture.json).expect("parse");
        root.as_object_mut().expect("objeto").remove("merkle_root_hex");
        let report = verify_attestation_inner(&root.to_string(), &trust_root_of(&fixture));
        assert!(!report.ok);
        assert!(report.error.expect("causa").contains("JSON inválido"));
    }

    #[test]
    fn an_invalid_payload_base64_is_reported() {
        let fixture = valid_fixture();
        let json = with_field(
            &fixture.json,
            "payload_b64",
            serde_json::Value::String("!!!".to_string()),
        );
        let report = verify_attestation_inner(&json, &trust_root_of(&fixture));
        assert!(!report.ok);
        assert!(!report.sha256 && !report.signature && !report.inclusion && !report.quorum);
        assert!(report.error.expect("causa").contains("base64"));
    }

    #[test]
    fn an_invalid_merkle_root_hex_is_reported() {
        let fixture = valid_fixture();
        let json = with_field(
            &fixture.json,
            "merkle_root_hex",
            serde_json::Value::String("não é hex".to_string()),
        );
        let report = verify_attestation_inner(&json, &trust_root_of(&fixture));
        assert!(!report.ok);
        assert!(!report.inclusion && !report.signature);
        assert!(report.error.expect("causa").contains("merkle_root_hex"));
    }

    #[test]
    fn an_empty_trust_root_rejects_signature_and_quorum_but_still_checks_the_log() {
        let fixture = valid_fixture();
        let report = verify_attestation_inner(&fixture.json, &TrustRoot::default());
        assert!(!report.ok);
        // Nada é confiável, mas a matemática do log continua conferível.
        assert!(report.sha256);
        assert!(report.inclusion);
        assert!(!report.signature);
        assert!(!report.quorum);
    }

    // --- 7: subject ---------------------------------------------------------

    #[test]
    fn the_subject_binds_payload_position_and_root() {
        let digest = [0xAAu8; 32];
        let root = [0xBBu8; 32];
        let base = attestation_subject(&digest, &root, 1, 8);

        assert!(base.starts_with(ATTESTATION_DOMAIN));
        assert_ne!(base, attestation_subject(&[0x01u8; 32], &root, 1, 8));
        assert_ne!(base, attestation_subject(&digest, &[0x01u8; 32], 1, 8));
        assert_ne!(base, attestation_subject(&digest, &root, 2, 8));
        assert_ne!(base, attestation_subject(&digest, &root, 1, 9));
        assert_eq!(base.len(), ATTESTATION_DOMAIN.len() + 32 + 32 + 8 + 8);
    }

    #[test]
    fn attestation_subject_hex_matches_the_bytes_that_were_signed() {
        let fixture = valid_fixture();
        let hex = attestation_subject_hex(&fixture.json);
        let (subject, _) = attestation_subject_of(&fixture.json).expect("subject");
        assert_eq!(hex, encode_hex(&subject));

        // E é de fato sobre esses bytes que a assinatura do fixture vale.
        let signature = decode_hex(&{
            let root: serde_json::Value = serde_json::from_str(&fixture.json).expect("parse");
            root["signature_hex"].as_str().expect("string").to_string()
        })
        .expect("hex");
        assert!(
            crate::signature::verify_with_key(&subject, &signature, &signing_key(1).verifying_key().to_bytes())
                .is_ok()
        );
    }

    #[test]
    fn attestation_subject_hex_is_empty_for_invalid_input() {
        assert_eq!(attestation_subject_hex("não é json"), "");
    }

    // --- 8: independência dos estágios ------------------------------------

    #[test]
    fn every_stage_can_fail_independently_without_masking_the_others() {
        let fixture = valid_fixture();
        let root = trust_root_of(&fixture);

        // Só o quórum quebrado — nem o subject nem a prova mudam.
        let json = with_field(&fixture.json, "quorum_threshold", serde_json::json!(3));
        let report = verify_attestation_inner(&json, &root);
        assert_eq!(
            (report.sha256, report.signature, report.inclusion, report.quorum),
            (true, true, true, false)
        );

        // Só a assinatura quebrada.
        let json = with_field(
            &fixture.json,
            "signature_hex",
            serde_json::Value::String(encode_hex(&[0u8; 64])),
        );
        let report = verify_attestation_inner(&json, &root);
        assert_eq!(
            (report.sha256, report.signature, report.inclusion, report.quorum),
            (true, false, true, true)
        );

        // Só a inclusão quebrada (prova adulterada, subject intacto).
        let mut proof = crate::encoding::decode_hex(
            serde_json::from_str::<serde_json::Value>(&fixture.json)
                .expect("parse")["merkle_proof_hex"]
                .as_str()
                .expect("string"),
        )
        .expect("hex");
        proof[0] ^= 0xFF;
        let json = with_field(
            &fixture.json,
            "merkle_proof_hex",
            serde_json::Value::String(encode_hex(&proof)),
        );
        let report = verify_attestation_inner(&json, &root);
        assert_eq!(
            (report.sha256, report.signature, report.inclusion, report.quorum),
            (true, true, false, true)
        );

        // Só o sha256 quebrado (digest declarado).
        let json = with_field(
            &fixture.json,
            "payload_sha256_hex",
            serde_json::Value::String("11".repeat(32)),
        );
        let report = verify_attestation_inner(&json, &root);
        assert_eq!(
            (report.sha256, report.signature, report.inclusion, report.quorum),
            (false, true, true, true)
        );
    }

    #[test]
    fn the_payload_length_does_not_change_the_subject_length() {
        let root = [0x11u8; 32];
        for len in [0usize, 1, 1024] {
            let payload = vec![0x42u8; len];
            let subject = attestation_subject(&sha256_bytes(&payload), &root, 0, 1);
            assert_eq!(subject.len(), ATTESTATION_DOMAIN.len() + 80);
        }
    }
}
