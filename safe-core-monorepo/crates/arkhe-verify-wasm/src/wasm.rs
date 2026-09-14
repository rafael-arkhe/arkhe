//! §2.1 — a superfície `#[wasm_bindgen]`, que é o que o navegador enxerga.
//!
//! Cada função aqui é uma casca fina: converte as entradas amigáveis do
//! JavaScript (`String`, `&[u8]`, `u32`) para as funções de verificação
//! tipadas dos outros módulos, e converte o [`Result`] de volta em um valor
//! que o JavaScript consome sem precisar tratar exceções — `bool`, ou uma
//! `String` com o relatório JSON no caso de [`verify_attestation`].
//!
//! Nenhuma destas funções lança: uma entrada malformada devolve `false` (ou
//! um relatório com `ok: false` e a causa), nunca uma exceção do lado do JS.
//! Isso é deliberado — um verificador client-side que lança em cima de bytes
//! não confiáveis transfere para o chamador a tarefa de distinguir "entrada
//! inválida" de "bug no verificador", e no navegador isso vira
//! `Uncaught (in promise)`.

use wasm_bindgen::prelude::*;

use crate::attestation::{
    attestation_subject_hex as subject_hex_inner, verify_attestation_inner, AttestationReport,
};
use crate::encoding::{TrustRoot, WitnessJson};
use crate::hash::{sha256_hex as sha256_hex_inner, verify_sha256_inner};
use crate::merkle::{merkle_root_from_leaves, verify_inclusion_inner};
use crate::quorum::verify_witness_quorum_inner;
use crate::signature::verify_signature_inner;

/// §2.1 — confere que `SHA-256(data)` é igual a `expected_hex`.
///
/// `expected_hex` aceita maiúsculas/minúsculas e o prefixo `0x` opcional.
/// Devolve `false` para um `expected_hex` malformado (não é hex, ou não tem
/// 32 bytes), em vez de lançar.
#[wasm_bindgen]
pub fn verify_sha256(data: &[u8], expected_hex: &str) -> bool {
    verify_sha256_inner(data, expected_hex).is_ok()
}

/// §2.1 — o SHA-256 de `data`, em hex minúsculo.
///
/// Exposto para que o chamador JavaScript consiga comparar digests sem
/// depender de outra implementação de SHA-256 no lado do JS.
#[wasm_bindgen]
pub fn sha256_hex(data: &[u8]) -> String {
    sha256_hex_inner(data)
}

/// §2.1 — confere uma assinatura Ed25519 e que a chave está no trust root.
///
/// - `message`: os bytes assinados.
/// - `signature`: a assinatura Ed25519 (64 bytes).
/// - `public_key_hex`: a chave pública do signatário (32 bytes, hex).
/// - `trust_root_json`: um array JSON de chaves públicas confiáveis em hex,
///   ex.: `["1f8f...c2","a30b...77"]`.
///
/// Devolve `false` se a chave não estiver no trust root, mesmo que a
/// assinatura seja criptograficamente válida, e também se o trust root não
/// for um JSON válido — um trust root que não dá para interpretar não confia
/// em nada.
#[wasm_bindgen]
pub fn verify_signature(
    message: &[u8],
    signature: &[u8],
    public_key_hex: &str,
    trust_root_json: &str,
) -> bool {
    let Ok(trust_root) = TrustRoot::parse(trust_root_json) else {
        return false;
    };
    verify_signature_inner(message, signature, public_key_hex, &trust_root).is_ok()
}

/// §2.1 — confere uma prova de inclusão Merkle RFC 6962 (§2.1.1).
///
/// - `leaf`: a folha cuja inclusão se quer provar (os bytes originais, não o
///   hash da folha — o prefixo `0x00` é aplicado internamente conforme o RFC).
/// - `leaf_index`: posição da folha, 0-based.
/// - `tree_size`: número de folhas da árvore a que `root_hex` pertence.
/// - `proof`: os digests irmãos concatenados (n × 32 bytes).
/// - `root_hex`: a raiz Merkle (32 bytes, hex).
///
/// Devolve `false` se a prova não reconstruir exatamente `root_hex`.
#[wasm_bindgen]
pub fn verify_inclusion(
    leaf: &[u8],
    leaf_index: u64,
    tree_size: u64,
    proof: &[u8],
    root_hex: &str,
) -> bool {
    verify_inclusion_inner(leaf, leaf_index, tree_size, proof, root_hex).is_ok()
}

/// §2.1 — confere que ao menos `threshold` witnesses **distintos** e
/// confiáveis assinaram `subject`.
///
/// - `subject`: os bytes que as testemunhas assinaram (em geral o retorno de
///   [`attestation_subject_hex`]).
/// - `witnesses_json`: `[{"public_key_hex": "...", "signature_hex": "..."}]`.
/// - `trust_root_json`: como em [`verify_signature`].
/// - `threshold`: quantas testemunhas distintas são exigidas. Precisa ser ao
///   menos 2 — quórum de 1 é rejeitado mesmo havendo testemunha válida.
///
/// A mesma testemunha listada duas vezes conta **uma vez**: a contagem é
/// sobre chaves distintas, não sobre entradas da lista.
#[wasm_bindgen]
pub fn verify_witness_quorum(
    subject: &[u8],
    witnesses_json: &str,
    trust_root_json: &str,
    threshold: u32,
) -> bool {
    let Ok(trust_root) = TrustRoot::parse(trust_root_json) else {
        return false;
    };
    let Ok(witnesses) = serde_json::from_str::<Vec<WitnessJson>>(witnesses_json) else {
        return false;
    };
    verify_witness_quorum_inner(subject, &witnesses, &trust_root, threshold).is_ok()
}

/// §2.1 — o pipeline completo, compondo as quatro verificações.
///
/// Devolve uma `String` com o relatório JSON:
///
/// ```json
/// {"ok":true,"sha256":true,"signature":true,"inclusion":true,"quorum":true,"error":null}
/// ```
///
/// Os quatro estágios são avaliados e reportados **independentemente**, então
/// uma atestação que falha só no quórum aparece com `"quorum":false` e os
/// outros três `true`. `error` traz a causa do primeiro estágio que falhou, em
/// ordem de pipeline.
///
/// Um `trust_root_json` inválido rejeita a atestação inteira (todos os
/// estágios `false`): sem saber em quem confiar, não há o que verificar.
#[wasm_bindgen]
pub fn verify_attestation(attestation_json: &str, trust_root_json: &str) -> String {
    let trust_root = match TrustRoot::parse(trust_root_json) {
        Ok(trust_root) => trust_root,
        Err(err) => {
            return render_report(&AttestationReport::rejected(format!(
                "trust root inválido: {err}"
            )))
        }
    };
    render_report(&verify_attestation_inner(attestation_json, &trust_root))
}

/// A raiz Merkle RFC 6962 de uma lista de folhas, em hex.
///
/// - `leaves_hex_json`: um array JSON de folhas em hex, ex.: `["616263"]`.
///
/// A árvore vazia devolve o SHA-256 da string vazia, conforme o RFC 6962
/// §2.1. Devolve a string vazia se a entrada for inválida — uma raiz válida
/// tem sempre 64 caracteres hex, então a string vazia nunca é ambígua.
#[wasm_bindgen]
pub fn merkle_root_hex(leaves_hex_json: &str) -> String {
    let Ok(leaves_hex) = serde_json::from_str::<Vec<String>>(leaves_hex_json) else {
        return String::new();
    };
    let mut leaves = Vec::with_capacity(leaves_hex.len());
    for leaf_hex in &leaves_hex {
        let Ok(leaf) = crate::encoding::decode_hex(leaf_hex) else {
            return String::new();
        };
        leaves.push(leaf);
    }
    crate::encoding::encode_hex(&merkle_root_from_leaves(&leaves))
}

/// O *subject* de uma atestação, em hex — os bytes exatos que precisam ser
/// assinados pelo signatário e pelas testemunhas.
///
/// Exposto porque o signatário e o verificador precisam concordar byte a byte
/// sobre o que está sendo assinado, e reconstruir essa concatenação no
/// JavaScript seria uma segunda implementação — ou seja, uma segunda chance
/// de divergir. Devolve a string vazia se a atestação for inválida.
#[wasm_bindgen]
pub fn attestation_subject_hex(attestation_json: &str) -> String {
    subject_hex_inner(attestation_json)
}

/// Serializa o relatório, sem pânico.
///
/// `AttestationReport` é um struct de `bool`s e um `Option<String>`, então a
/// serialização não tem como falhar; ainda assim o erro é tratado em vez de
/// ignorado, porque a alternativa (`unwrap`) seria um pânico em potencial
/// dentro do wasm, onde ele vira um `RuntimeError` opaco.
fn render_report(report: &AttestationReport) -> String {
    match serde_json::to_string(report) {
        Ok(json) => json,
        Err(err) => format!(
            "{{\"ok\":false,\"sha256\":false,\"signature\":false,\"inclusion\":false,\"quorum\":false,\"error\":\"falha ao serializar o relatório: {}\"}}",
            err.to_string().replace('"', "'")
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encoding::encode_hex;
    use ed25519_dalek::{Signer, SigningKey};

    fn signing_key(seed: u8) -> SigningKey {
        SigningKey::from_bytes(&[seed; 32])
    }

    fn public_hex(seed: u8) -> String {
        encode_hex(signing_key(seed).verifying_key().as_bytes())
    }

    #[test]
    fn sha256_wrapper_matches_the_known_vector() {
        assert!(verify_sha256(
            b"abc",
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        ));
        assert!(!verify_sha256(b"abcd", "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"));
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn sha256_wrapper_rejects_malformed_expected_digests_without_panicking() {
        assert!(!verify_sha256(b"abc", "não é hex"));
        assert!(!verify_sha256(b"abc", ""));
        assert!(!verify_sha256(b"abc", &"ab".repeat(33)));
    }

    #[test]
    fn signature_wrapper_accepts_a_trusted_key_and_rejects_an_untrusted_one() {
        let message = b"wasm surface";
        let signature = signing_key(1).sign(message).to_bytes();
        let trust_root = format!("[\"{}\"]", public_hex(1));

        assert!(verify_signature(message, &signature, &public_hex(1), &trust_root));
        assert!(!verify_signature(message, &signature, &public_hex(2), &trust_root));
        // Trust root malformado não confia em nada, em vez de lançar.
        assert!(!verify_signature(message, &signature, &public_hex(1), "não é json"));
    }

    #[test]
    fn quorum_wrapper_enforces_two_and_dedupes_repeated_witnesses() {
        let subject = b"quorum subject";
        let witnesses: Vec<WitnessJson> = [1u8, 2]
            .iter()
            .map(|&seed| WitnessJson {
                public_key_hex: public_hex(seed),
                signature_hex: encode_hex(&signing_key(seed).sign(subject).to_bytes()),
            })
            .collect();
        let witnesses_json = serde_json::to_string(&witnesses).expect("serializa");
        let trust_root = format!("[\"{}\",\"{}\"]", public_hex(1), public_hex(2));

        assert!(verify_witness_quorum(subject, &witnesses_json, &trust_root, 2));
        // Quórum de 1 é rejeitado mesmo com 2 witnesses válidos.
        assert!(!verify_witness_quorum(subject, &witnesses_json, &trust_root, 1));
        // Uma testemunha só não basta para 2.
        let one = serde_json::to_string(&witnesses[..1]).expect("serializa");
        assert!(!verify_witness_quorum(subject, &one, &trust_root, 2));
        // A mesma testemunha repetida não forja um quórum de 2.
        let repeated = vec![witnesses[0].clone(), witnesses[0].clone()];
        let repeated_json = serde_json::to_string(&repeated).expect("serializa");
        assert!(!verify_witness_quorum(subject, &repeated_json, &trust_root, 2));
        // JSON de witnesses inválido: false, não pânico.
        assert!(!verify_witness_quorum(subject, "{não é json}", &trust_root, 2));
    }

    #[test]
    fn merkle_root_wrapper_handles_empty_valid_and_invalid_input() {
        assert_eq!(
            merkle_root_hex("[]"),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        // Uma folha "abc": SHA-256(0x00 ∥ "abc"), conferido em Python.
        assert_eq!(
            merkle_root_hex("[\"616263\"]"),
            "609f6e36d2405585188d5cfd761f407c7cc46a7d3f314c88270469dde315fcd1"
        );
        // Uma folha vazia: SHA-256(0x00), o primeiro vetor do RFC 6962.
        assert_eq!(
            merkle_root_hex("[\"\"]"),
            "6e340b9cffb37a989ca544e6bb780a2c78901d3fb33738768511a30617afa01d"
        );
        assert_eq!(merkle_root_hex("não é json"), "");
        assert_eq!(merkle_root_hex("[\"zz\"]"), "");
    }

    #[test]
    fn the_wasm_attestation_entry_point_returns_a_parseable_report() {
        // Sem fixture completa: o que se verifica aqui é o contrato de
        // entrada/saída da casca wasm — sempre uma String JSON, nunca pânico.
        let report = verify_attestation("entrada inválida", "[]");
        let value: serde_json::Value = serde_json::from_str(&report).expect("JSON válido");
        assert_eq!(value["ok"], false);
        assert!(value["error"].is_string());
    }

    #[test]
    fn an_invalid_trust_root_rejects_the_whole_attestation() {
        let report = verify_attestation("{}", "não é json");
        let value: serde_json::Value = serde_json::from_str(&report).expect("JSON válido");
        assert_eq!(value["ok"], false);
        assert_eq!(value["sha256"], false);
        assert!(value["error"]
            .as_str()
            .expect("string")
            .contains("trust root inválido"));
    }

    #[test]
    fn the_subject_wrapper_is_empty_for_invalid_input() {
        assert_eq!(attestation_subject_hex("não é json"), "");
    }
}
