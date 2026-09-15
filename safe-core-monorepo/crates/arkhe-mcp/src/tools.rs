//! As ferramentas do servidor: o que cada uma recebe, o que devolve, e a que
//! função **real** do `arkhe-verify` ela delega.
//!
//! # Por que este módulo não conhece o `rmcp`
//!
//! As funções daqui são Rust comum: recebem os argumentos já desserializados e
//! devolvem um [`ToolOutcome`]. O acoplamento com o protocolo MCP fica inteiro
//! em [`crate::server`]. A consequência prática é que os testes deste módulo
//! exercitam as ferramentas **diretamente**, sem um cliente MCP vivo.
//!
//! # As codificações são as do core
//!
//! Nada de novo é inventado aqui: **base64** para o que é arbitrário (payload,
//! folha, mensagem) e **hex** para o que é de tamanho fixo ou curto (digest,
//! assinatura, chave, raiz, prova) — exatamente a convenção documentada em
//! `arkhe_verify::encoding`, e os decodificadores usados são os de lá
//! ([`decode_base64`], [`decode_hex`]), não uma segunda implementação.
//!
//! O trust root é o único campo com uma diferença de forma: o core o interpreta
//! a partir de um JSON (um array de chaves em hex), e aqui ele chega como um
//! `Vec<String>` — que é serializado de volta àquela forma antes de
//! [`arkhe_verify::TrustRoot::parse`] rodar. A forma do core é preservada; o que
//! muda é só não obrigar quem chama a escapar JSON dentro de JSON.

use serde::Serialize;
use serde_json::{json, Value};

use arkhe_verify::encoding::{decode_base64, decode_hex};
use arkhe_verify::TrustRoot;

/// Os argumentos de `arkhe_verify_sha256`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, schemars::JsonSchema)]
pub struct Sha256Args {
    /// O payload, em base64 standard (com padding).
    pub payload_b64: String,
    /// O digest SHA-256 declarado para o payload, em hex (aceita `0x` e
    /// maiúsculas/minúsculas).
    pub expected_hex: String,
}

/// Os argumentos de `arkhe_verify_signature`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, schemars::JsonSchema)]
pub struct SignatureArgs {
    /// A mensagem assinada, em base64 standard.
    pub message_b64: String,
    /// A assinatura Ed25519 da mensagem, em hex.
    pub signature_hex: String,
    /// A chave pública Ed25519 do signatário, em hex (32 bytes).
    pub public_key_hex: String,
    /// As chaves públicas em que a verificação confia, em hex. Um array vazio é
    /// válido e não confia em nada.
    pub trust_root: Vec<String>,
}

/// Os argumentos de `arkhe_verify_inclusion`.
///
/// Os nomes dos campos são os da função delegada
/// ([`arkhe_verify::verify_inclusion`] recebe `leaf_index` e `tree_size`). No
/// documento de atestação os mesmos valores aparecem como `merkle_leaf_index` e
/// `merkle_tree_size` — são os dois vocabulários do core para os mesmos dois
/// números, e esta ferramenta usa o da função que chama.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, schemars::JsonSchema)]
pub struct InclusionArgs {
    /// A folha original, em base64 — os bytes que foram incluídos na árvore (o
    /// prefixo `0x00` do RFC 6962 é aplicado dentro do core).
    pub leaf_b64: String,
    /// A posição da folha na árvore (0-based).
    pub leaf_index: u64,
    /// O número de folhas da árvore a que `root_hex` pertence.
    pub tree_size: u64,
    /// A prova de inclusão: os digests irmãos concatenados, em hex (n × 32
    /// bytes).
    pub proof_hex: String,
    /// A raiz Merkle RFC 6962 conhecida, em hex.
    pub root_hex: String,
}

/// Os argumentos de `arkhe_verify_attestation`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, schemars::JsonSchema)]
pub struct AttestationArgs {
    /// O documento de atestação, como JSON. O formato **é** a interface do
    /// pipeline do core: os quatro estágios leem os campos daqui, e é o mesmo
    /// documento que a página do `arkhe-verify-wasm` envia.
    pub attestation_json: String,
    /// As chaves públicas em que a verificação confia, em hex (32 bytes cada).
    pub trust_root: Vec<String>,
}

/// O que uma ferramenta devolve, antes de virar resposta MCP.
///
/// Os dois casos são a distinção central deste crate: ver a seção "Duas
/// espécies de 'não passou'" na documentação do [`crate`]. Um
/// [`ToolOutcome::Report`] **é** o relatório do `arkhe-verify` serializado sem
/// tradução — nenhum campo é renomeado, achatado ou reinterpretado por esta
/// crate, para que a forma que o agente vê seja a mesma que a casca nativa
/// documenta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolOutcome {
    /// A verificação **rodou** e produziu um relatório do `arkhe-verify`.
    ///
    /// Isto inclui os vereditos negativos: uma prova que não fecha é um
    /// resultado — `ok: false`, com a causa e os dados no relatório.
    Report(Value),
    /// A verificação **não rodou**: os argumentos não puderam ser
    /// interpretados, e não existe relatório para algo que não foi verificado.
    Invalid(Value),
}

impl ToolOutcome {
    /// `true` se a verificação rodou, independentemente do veredito.
    ///
    /// É `false` só para [`ToolOutcome::Invalid`] — um relatório reprovado
    /// devolve `true`, porque um veredito negativo é um resultado.
    pub fn ran(&self) -> bool {
        matches!(self, Self::Report(_))
    }

    /// O JSON devolvido, com o relatório ou com a causa da rejeição.
    pub fn value(&self) -> &Value {
        match self {
            Self::Report(value) | Self::Invalid(value) => value,
        }
    }
}

/// Confere que `SHA-256(payload)` é o digest declarado.
///
/// Delega a [`arkhe_verify::verify_sha256`]. Um `expected_hex` malformado não é
/// rejeitado aqui: a função do core o reporta como veredito reprovado e devolve
/// o digest calculado de qualquer forma, porque o digest existe mesmo quando o
/// declarado não é hex. Rejeitado aqui é só o `payload_b64` que não decodifica —
/// sem os bytes não há o que digerir.
pub fn verify_sha256(args: Sha256Args) -> ToolOutcome {
    let payload = match decode_base64(&args.payload_b64) {
        Ok(payload) => payload,
        Err(err) => return invalid("payload_b64", &err.to_string()),
    };

    report(&arkhe_verify::verify_sha256(&payload, &args.expected_hex))
}

/// Confere uma assinatura Ed25519 **e** que a chave está no trust root.
///
/// Delega a [`arkhe_verify::verify_signature`]. O trust root é interpretado por
/// [`TrustRoot::parse`] — o parser do core —, então um trust root malformado
/// (chave que não é hex, ou que não tem 32 bytes) é uma rejeição de entrada: a
/// fachada nativa recebe um trust root já tipado, e um valor malformado não tem
/// como chegar a uma verificação.
pub fn verify_signature(args: SignatureArgs) -> ToolOutcome {
    let message = match decode_base64(&args.message_b64) {
        Ok(message) => message,
        Err(err) => return invalid("message_b64", &err.to_string()),
    };

    let signature = match decode_hex(&args.signature_hex) {
        Ok(signature) => signature,
        Err(err) => return invalid("signature_hex", &err.to_string()),
    };

    // O core lê o trust root de um JSON; o `Vec<String>` recebido é
    // serializado para exatamente aquela forma antes de `parse`.
    let trust_root_json = match serde_json::to_string(&args.trust_root) {
        Ok(json) => json,
        Err(err) => return invalid("trust_root", &err.to_string()),
    };

    let trust_root = match TrustRoot::parse(&trust_root_json) {
        Ok(trust_root) => trust_root,
        Err(err) => return invalid("trust_root", &err.to_string()),
    };

    report(&arkhe_verify::verify_signature(
        &message,
        &signature,
        &args.public_key_hex,
        &trust_root,
    ))
}

/// Confere uma prova de inclusão Merkle RFC 6962 contra uma raiz conhecida.
///
/// Delega a [`arkhe_verify::verify_inclusion`]. A prova é a concatenação crua
/// dos digests irmãos em hex; a folha vai em base64, porque é arbitrária.
pub fn verify_inclusion(args: InclusionArgs) -> ToolOutcome {
    let leaf = match decode_base64(&args.leaf_b64) {
        Ok(leaf) => leaf,
        Err(err) => return invalid("leaf_b64", &err.to_string()),
    };

    let proof = match decode_hex(&args.proof_hex) {
        Ok(proof) => proof,
        Err(err) => return invalid("proof_hex", &err.to_string()),
    };

    report(&arkhe_verify::verify_inclusion(
        &leaf,
        args.leaf_index,
        args.tree_size,
        &proof,
        &args.root_hex,
    ))
}

/// O pipeline completo — as quatro verificações compostas — sobre um documento
/// de atestação.
///
/// Delega a [`arkhe_verify::verify_attestation`], que é a função do core para o
/// pipeline: ela avalia os quatro estágios **independentemente** e reporta cada
/// um, então uma atestação que falha só no quórum volta com `quorum: false` e os
/// outros três `true`.
///
/// É por aqui que o quórum de witnesses é verificado: o documento traz os
/// witnesses e o `quorum_threshold`, e o pipeline chama o quórum do core. Não há
/// ferramenta separada de quórum — ver "Por que o quórum não tem ferramenta
/// própria" na documentação do [`crate`] para a escolha e o que ela custa.
///
/// Um documento que não é JSON **não** é rejeitado aqui: o core o reporta como
/// atestação reprovada nos quatro estágios mais a causa, porque o formato JSON é
/// a interface do pipeline e a fachada nativa não o reinterpreta.
pub fn verify_attestation(args: AttestationArgs) -> ToolOutcome {
    let trust_root_json = match serde_json::to_string(&args.trust_root) {
        Ok(json) => json,
        Err(err) => return invalid("trust_root", &err.to_string()),
    };

    let trust_root = match TrustRoot::parse(&trust_root_json) {
        Ok(trust_root) => trust_root,
        Err(err) => return invalid("trust_root", &err.to_string()),
    };

    report(&arkhe_verify::verify_attestation(
        &args.attestation_json,
        &trust_root,
    ))
}

/// Serializa um relatório do `arkhe-verify` sem traduzir nada.
///
/// A falha de serialização é inalcançável para os quatro relatórios concretos
/// (todos são `Serialize` derivado de campos simples), mas não é `unwrap()`: se
/// ela acontecesse, seria uma entrada que não produziu relatório — o mesmo caso
/// de [`ToolOutcome::Invalid`].
fn report<T: Serialize>(report: &T) -> ToolOutcome {
    match serde_json::to_value(report) {
        Ok(value) => ToolOutcome::Report(value),
        Err(err) => invalid("report", &err.to_string()),
    }
}

/// Uma rejeição de entrada, na forma que o agente lê.
///
/// O campo `stage` existe para que uma rejeição não seja confundida com um
/// veredito: os relatórios do `arkhe-verify` também têm `ok`, então
/// `{"ok": false, "stage": "input"}` diz que a verificação **não rodou**,
/// enquanto `{"ok": false}` com os campos do relatório diz que ela rodou e
/// reprovou.
fn invalid(field: &str, cause: &str) -> ToolOutcome {
    ToolOutcome::Invalid(json!({
        "ok": false,
        "stage": "input",
        "field": field,
        "error": cause,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    use arkhe_verify::encoding::encode_hex;
    use ed25519_dalek::{Signer, SigningKey};

    // --- fixtures -----------------------------------------------------------

    fn signing_key(seed: u8) -> SigningKey {
        SigningKey::from_bytes(&[seed; 32])
    }

    fn public_hex(seed: u8) -> String {
        encode_hex(signing_key(seed).verifying_key().as_bytes())
    }

    fn base64_encode(bytes: &[u8]) -> String {
        use base64::engine::general_purpose::STANDARD;
        use base64::Engine;
        STANDARD.encode(bytes)
    }

    /// O digest da fixture, calculado com o `sha2` do teste — **não** pelo
    /// `arkhe-verify`.
    ///
    /// Não é preciosismo: montar a atestação com a mesma função que a verifica
    /// faria a fixture confirmar o que a própria implementação produz. O
    /// `arkhe-verify` não reexporta `sha256_bytes` na raiz (o core o tem, a
    /// fachada não), então o caminho independente também é o mais direto.
    fn sha256_hex(bytes: &[u8]) -> String {
        use sha2::{Digest, Sha256};
        encode_hex(&Sha256::digest(bytes))
    }

    // --- leitura do valor devolvido -----------------------------------------
    //
    // Os `Value` são indexados por nome de campo e os acessores abaixo falham
    // alto quando o campo não tem o tipo esperado: se o `arkhe-verify` mudasse a
    // forma de um relatório, o teste diria *isso*, em vez de comparar `Null`
    // silenciosamente.

    fn boolean(value: &Value, field: &str) -> bool {
        value[field]
            .as_bool()
            .expect("campo booleano ausente no valor")
    }

    fn text<'a>(value: &'a Value, field: &str) -> &'a str {
        value[field]
            .as_str()
            .expect("campo textual ausente no valor")
    }

    /// Uma prova de inclusão **de verdade** de uma árvore de 5 folhas, com a
    /// mesma `ct-merkle` que o core usa.
    fn inclusion_fixture() -> (Vec<u8>, u64, u64, Vec<u8>, String) {
        use ct_merkle::mem_backed_tree::MemoryBackedTree;
        use sha2::Sha256;

        let leaves: Vec<Vec<u8>> = (0..5)
            .map(|index| format!("artifact-{index}").into_bytes())
            .collect();
        let mut tree = MemoryBackedTree::<Sha256, Vec<u8>>::new();
        for leaf in &leaves {
            tree.push(leaf.clone());
        }
        let proof = tree.prove_inclusion(2).as_bytes().to_vec();
        let root = encode_hex(&arkhe_verify::merkle_root_from_leaves(&leaves));
        (leaves[2].clone(), 2, 5, proof, root)
    }

    /// Uma atestação que passa nos quatro estágios.
    ///
    /// O *subject* é montado pelo próprio core
    /// ([`arkhe_verify::attestation_subject`]), assinado por chaves
    /// determinísticas, e depois a atestação é remontada com as assinaturas. O
    /// mesmo caminho que o `arkhe-verify` usa nos testes dele: o core define o
    /// que se assina, e o teste assina exatamente aquilo.
    fn passing_attestation() -> (String, Vec<String>) {
        let (leaf, leaf_index, tree_size, proof, root) = inclusion_fixture();

        let doc = |signature_hex: &str, witnesses: Value| {
            json!({
                "payload_b64": base64_encode(&leaf),
                "payload_sha256_hex": sha256_hex(&leaf),
                "merkle_leaf_index": leaf_index,
                "merkle_tree_size": tree_size,
                "merkle_proof_hex": encode_hex(&proof),
                "merkle_root_hex": root,
                "signer_public_key_hex": public_hex(1),
                "signature_hex": signature_hex,
                "witnesses": witnesses,
                "quorum_threshold": 2,
            })
            .to_string()
        };

        let subject = arkhe_verify::attestation_subject(&doc("", json!([]))).expect("subject");
        let signer_signature = encode_hex(&signing_key(1).sign(&subject).to_bytes());
        let witnesses = json!([
            {
                "public_key_hex": public_hex(2),
                "signature_hex": encode_hex(&signing_key(2).sign(&subject).to_bytes()),
            },
            {
                "public_key_hex": public_hex(3),
                "signature_hex": encode_hex(&signing_key(3).sign(&subject).to_bytes()),
            },
        ]);

        (
            doc(&signer_signature, witnesses),
            vec![public_hex(1), public_hex(2), public_hex(3)],
        )
    }

    // --- sha256 -------------------------------------------------------------

    #[test]
    fn sha256_verifies_a_known_answer_vector() {
        // SHA-256("abc"), o vetor clássico de FIPS 180-4.
        let outcome = verify_sha256(Sha256Args {
            payload_b64: base64_encode(b"abc"),
            expected_hex: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
                .to_string(),
        });

        assert!(outcome.ran(), "a verificação rodou");
        let value = outcome.value();
        assert!(boolean(value, "ok"));
        assert_eq!(
            text(value, "computed_hex"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert!(value["error"].is_null());
    }

    #[test]
    fn a_wrong_digest_is_a_report_and_not_a_rejection() {
        let outcome = verify_sha256(Sha256Args {
            payload_b64: base64_encode(b"abc"),
            expected_hex: "00".repeat(32),
        });

        assert!(
            outcome.ran(),
            "um veredito negativo é um resultado: a verificação rodou"
        );
        let value = outcome.value();
        assert!(!boolean(value, "ok"));
        assert!(
            value.get("stage").is_none(),
            "sem `stage`: isto é um relatório, não uma rejeição de entrada"
        );
        assert!(
            value["computed_hex"].is_string(),
            "o digest calculado vem junto do veredito"
        );
    }

    #[test]
    fn a_payload_that_is_not_base64_is_rejected_before_any_verification() {
        let outcome = verify_sha256(Sha256Args {
            payload_b64: "!!! isto não é base64 !!!".to_string(),
            expected_hex: "00".repeat(32),
        });

        assert!(!outcome.ran(), "não há verificação sem os bytes");
        let value = outcome.value();
        assert!(!boolean(value, "ok"));
        assert_eq!(text(value, "stage"), "input");
        assert_eq!(text(value, "field"), "payload_b64");
        assert!(!text(value, "error").is_empty());
    }

    // --- signature ----------------------------------------------------------

    #[test]
    fn signature_accepts_a_trusted_key() {
        let message = b"arkhe-os: subject under signature";
        let outcome = verify_signature(SignatureArgs {
            message_b64: base64_encode(message),
            signature_hex: encode_hex(&signing_key(1).sign(message).to_bytes()),
            public_key_hex: public_hex(1),
            trust_root: vec![public_hex(1), public_hex(2)],
        });

        assert!(outcome.ran());
        let value = outcome.value();
        assert!(boolean(value, "ok"), "erro: {}", value["error"]);
        assert!(boolean(value, "trusted"));
    }

    #[test]
    fn signature_reports_an_untrusted_key_without_rejecting_the_call() {
        let message = b"arkhe-os: subject under signature";
        let outcome = verify_signature(SignatureArgs {
            message_b64: base64_encode(message),
            signature_hex: encode_hex(&signing_key(9).sign(message).to_bytes()),
            public_key_hex: public_hex(9),
            trust_root: vec![public_hex(1)],
        });

        assert!(
            outcome.ran(),
            "a assinatura confere; quem falhou foi a confiança"
        );
        let value = outcome.value();
        assert!(!boolean(value, "ok"));
        assert!(
            !boolean(value, "trusted"),
            "o relatório separa `trusted` do veredito — é o que o booleano da casca wasm perde"
        );
        assert!(text(value, "error").contains("trust root"));
    }

    #[test]
    fn a_malformed_trust_root_is_rejected_before_any_verification() {
        let message = b"x";
        let outcome = verify_signature(SignatureArgs {
            message_b64: base64_encode(message),
            signature_hex: encode_hex(&signing_key(1).sign(message).to_bytes()),
            public_key_hex: public_hex(1),
            // Uma chave de 3 bytes: o `TrustRoot` do core exige 32.
            trust_root: vec!["abc".to_string()],
        });

        assert!(!outcome.ran());
        let value = outcome.value();
        assert_eq!(text(value, "stage"), "input");
        assert_eq!(text(value, "field"), "trust_root");
    }

    // --- inclusion ----------------------------------------------------------

    #[test]
    fn inclusion_verifies_a_real_proof() {
        let (leaf, leaf_index, tree_size, proof, root) = inclusion_fixture();
        let outcome = verify_inclusion(InclusionArgs {
            leaf_b64: base64_encode(&leaf),
            leaf_index,
            tree_size,
            proof_hex: encode_hex(&proof),
            root_hex: root.clone(),
        });

        assert!(outcome.ran());
        let value = outcome.value();
        assert!(boolean(value, "ok"), "erro: {}", value["error"]);
        assert_eq!(value["leaf_index"].as_u64(), Some(leaf_index));
        assert_eq!(value["tree_size"].as_u64(), Some(tree_size));
        assert_eq!(text(value, "root_hex"), root);
    }

    #[test]
    fn inclusion_reports_a_tampered_proof_as_a_negative_verdict() {
        let (leaf, leaf_index, tree_size, mut proof, root) = inclusion_fixture();
        proof[0] ^= 0x01;

        let outcome = verify_inclusion(InclusionArgs {
            leaf_b64: base64_encode(&leaf),
            leaf_index,
            tree_size,
            proof_hex: encode_hex(&proof),
            root_hex: root.clone(),
        });

        assert!(outcome.ran(), "a prova foi verificada; ela é que não fecha");
        let value = outcome.value();
        assert!(!boolean(value, "ok"));
        assert_eq!(
            text(value, "root_hex"),
            root,
            "o relatório ecoa a raiz pedida"
        );
        assert!(value["error"].is_string());
    }

    #[test]
    fn an_odd_length_proof_hex_is_rejected() {
        let (leaf, leaf_index, tree_size, _proof, root) = inclusion_fixture();
        let outcome = verify_inclusion(InclusionArgs {
            leaf_b64: base64_encode(&leaf),
            leaf_index,
            tree_size,
            proof_hex: "abc".to_string(),
            root_hex: root,
        });

        assert!(!outcome.ran());
        assert_eq!(text(outcome.value(), "field"), "proof_hex");
    }

    // --- attestation --------------------------------------------------------

    #[test]
    fn attestation_runs_the_whole_pipeline_and_all_four_stages_pass() {
        let (attestation_json, trust_root) = passing_attestation();
        let outcome = verify_attestation(AttestationArgs {
            attestation_json,
            trust_root,
        });

        assert!(outcome.ran());
        let value = outcome.value();
        assert!(boolean(value, "ok"), "erro: {}", value["error"]);
        assert!(boolean(value, "sha256"));
        assert!(boolean(value, "signature"));
        assert!(boolean(value, "inclusion"));
        assert!(boolean(value, "quorum"), "o quórum roda dentro do pipeline");
    }

    #[test]
    fn attestation_without_witnesses_fails_only_the_quorum_stage() {
        let (attestation_json, trust_root) = passing_attestation();
        let mut stripped: Value = serde_json::from_str(&attestation_json).expect("parse");
        stripped["witnesses"] = json!([]);

        let outcome = verify_attestation(AttestationArgs {
            attestation_json: stripped.to_string(),
            trust_root,
        });

        let value = outcome.value();
        assert!(
            !boolean(value, "ok"),
            "sem quórum o pipeline reprova — mas só ele"
        );
        assert!(boolean(value, "sha256"));
        assert!(boolean(value, "signature"));
        assert!(boolean(value, "inclusion"));
        assert!(!boolean(value, "quorum"));
    }

    #[test]
    fn a_malformed_attestation_document_is_the_cores_report_not_this_crates() {
        // O caso que a fronteira não intercepta, de propósito: o formato JSON do
        // documento **é** a interface do pipeline, e quem o interpreta é o core.
        let outcome = verify_attestation(AttestationArgs {
            attestation_json: "isto não é json".to_string(),
            trust_root: vec![public_hex(1)],
        });

        assert!(
            outcome.ran(),
            "o core reporta a atestação malformada; ele não a rejeita na fronteira"
        );
        let value = outcome.value();
        assert!(!boolean(value, "ok"));
        assert!(!boolean(value, "sha256"));
        assert!(!boolean(value, "signature"));
        assert!(!boolean(value, "inclusion"));
        assert!(!boolean(value, "quorum"));
        assert!(text(value, "error").contains("JSON"));
    }

    #[test]
    fn an_unknown_field_in_the_attestation_is_not_a_rejection() {
        // Um campo a mais não impede o pipeline de rodar: a desserialização do
        // core é permissiva quanto a campos desconhecidos, e o relatório diz o
        // que de fato bateu.
        let (attestation_json, trust_root) = passing_attestation();
        let mut doc: Value = serde_json::from_str(&attestation_json).expect("parse");
        doc["campo_que_ninguem_le"] = json!("ignorado");

        let outcome = verify_attestation(AttestationArgs {
            attestation_json: doc.to_string(),
            trust_root,
        });

        assert!(outcome.ran());
        assert!(boolean(outcome.value(), "ok"));
    }

    // --- a forma da rejeição ------------------------------------------------

    #[test]
    fn a_rejection_is_distinguishable_from_a_negative_verdict() {
        let rejected = verify_sha256(Sha256Args {
            payload_b64: "não é base64".to_string(),
            expected_hex: "00".to_string(),
        });
        let negative = verify_sha256(Sha256Args {
            payload_b64: base64_encode(b"abc"),
            expected_hex: "00".repeat(32),
        });

        assert_eq!(text(rejected.value(), "stage"), "input");
        assert!(negative.value().get("stage").is_none());
        assert_eq!(
            boolean(rejected.value(), "ok"),
            boolean(negative.value(), "ok")
        );
        assert!(!rejected.ran());
        assert!(negative.ran());
    }
}
