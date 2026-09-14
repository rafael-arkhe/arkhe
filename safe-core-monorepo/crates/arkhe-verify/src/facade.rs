//! A fachada nativa: as cinco verificações, com tipos nativos e relatórios
//! estruturados.
//!
//! **Um core, duas cascas.** Cada função daqui é uma casca fina sobre o *mesmo*
//! core que a casca `#[wasm_bindgen]` chama, em `arkhe-verify-wasm` — as funções
//! `*_inner`. Nenhuma verificação é reimplementada neste crate, e nenhuma
//! decisão criptográfica é tomada aqui: o que muda entre as duas cascas é só a
//! forma da entrada e da saída.
//!
//! | Verificação | Esta casca (nativa) | Casca wasm | Core (uma só) |
//! |:---|:---|:---|:---|
//! | SHA-256 | [`verify_sha256`] → [`Sha256Report`] | `wasm::verify_sha256` → `bool` | `hash::verify_sha256_inner` |
//! | assinatura | [`verify_signature`] → [`SignatureReport`] | `wasm::verify_signature` → `bool` | `signature::verify_signature_inner` |
//! | inclusão | [`verify_inclusion`] → [`InclusionReport`] | `wasm::verify_inclusion` → `bool` | `merkle::verify_inclusion_inner` |
//! | quórum | [`verify_witness_quorum`] → [`QuorumReport`] | `wasm::verify_witness_quorum` → `bool` | `quorum::verify_witness_quorum_inner` |
//! | pipeline | [`verify_attestation`] → `AttestationReport` | `wasm::verify_attestation` → `String` | `attestation::verify_attestation_inner` |
//!
//! A coluna do core é literal: são essas as funções chamadas abaixo, pelo
//! caminho `arkhe_verify_wasm::*` (o crate é dependência deste como `rlib`).
//!
//! # O que a casca nativa acrescenta
//!
//! - **Tipos, não strings.** O trust root é um [`TrustRoot`] já interpretado, e
//!   não um JSON. Consequência que vale registrar: um trust root malformado
//!   **não tem como chegar** a [`verify_signature`] nem a
//!   [`verify_witness_quorum`] — não existe valor de [`TrustRoot`] malformado.
//!   Na casca wasm isso precisa ser tratado como caso (e é: lá um JSON inválido
//!   rejeita tudo); aqui o tipo elimina o caso.
//! - **Relatórios.** Ver a nota em [`crate::report`]: o veredito vem com os
//!   dados contra os quais foi produzido, inclusive `valid_witnesses` no
//!   quórum, que um `bool` não carrega.
//!
//! Nenhuma função daqui falha de forma abrupta: entrada malformada vira um
//! relatório com `ok: false` e a causa, exatamente como na casca wasm.

use arkhe_verify_wasm::encoding::{decode_fixed_hex, PUBLIC_KEY_HEX_LEN};
use arkhe_verify_wasm::{
    attestation, count_valid_witnesses, sha256_hex, verify_inclusion_inner, verify_sha256_inner,
    verify_signature_inner, verify_witness_quorum_inner, AttestationReport, TrustRoot,
    VerifyError, WitnessJson,
};

use crate::report::{InclusionReport, QuorumReport, Sha256Report, SignatureReport};

/// Confere que `SHA-256(data)` é igual a `expected_hex`.
///
/// `expected_hex` aceita maiúsculas/minúsculas e o prefixo `0x` opcional. Um
/// `expected_hex` malformado produz `ok: false` com a causa — o digest
/// calculado é reportado de qualquer forma, porque ele existe mesmo quando o
/// declarado não é hex.
pub fn verify_sha256(data: &[u8], expected_hex: &str) -> Sha256Report {
    let outcome = verify_sha256_inner(data, expected_hex);
    Sha256Report {
        ok: outcome.is_ok(),
        expected_hex: expected_hex.to_string(),
        computed_hex: sha256_hex(data),
        error: outcome.err().map(|err| err.to_string()),
    }
}

/// Confere uma assinatura Ed25519 **e** que a chave está no trust root.
///
/// Os dois fatos são reportados em campos separados
/// ([`SignatureReport::ok`] e [`SignatureReport::trusted`]): uma assinatura
/// válida por uma chave não confiável é rejeitada, e o relatório diz que foi a
/// confiança que falhou, não a criptografia.
pub fn verify_signature(
    message: &[u8],
    signature: &[u8],
    public_key_hex: &str,
    trust_root: &TrustRoot,
) -> SignatureReport {
    // A checagem de confiança é feita aqui *também* para poder reportá-la; o
    // veredito continua vindo do core, que faz a mesma checagem antes de
    // qualquer trabalho criptográfico.
    let trusted = decode_fixed_hex::<PUBLIC_KEY_HEX_LEN>(public_key_hex)
        .map(|key| trust_root.contains(&key))
        .unwrap_or(false);

    let outcome = verify_signature_inner(message, signature, public_key_hex, trust_root);
    SignatureReport {
        ok: outcome.is_ok(),
        public_key_hex: public_key_hex.to_string(),
        trusted,
        error: outcome.err().map(|err| err.to_string()),
    }
}

/// Confere uma prova de inclusão Merkle RFC 6962 contra uma raiz conhecida.
///
/// `proof` é a concatenação crua dos digests irmãos (n × 32 bytes); `leaf` são
/// os bytes originais da folha (o prefixo `0x00` é aplicado dentro do core,
/// conforme o RFC); `tree_size` é o número de folhas da árvore a que
/// `root_hex` pertence.
///
/// Para o caso Rekor, [`crate::rekor::LogEntry::inclusion_report`] monta esses
/// quatro argumentos a partir de uma entrada de log.
pub fn verify_inclusion(
    leaf: &[u8],
    leaf_index: u64,
    tree_size: u64,
    proof: &[u8],
    root_hex: &str,
) -> InclusionReport {
    let outcome = verify_inclusion_inner(leaf, leaf_index, tree_size, proof, root_hex);
    InclusionReport {
        ok: outcome.is_ok(),
        leaf_index,
        tree_size,
        root_hex: root_hex.to_string(),
        error: outcome.err().map(|err| err.to_string()),
    }
}

/// Confere que ao menos `threshold` witnesses **distintos** e confiáveis
/// assinaram `subject`.
///
/// Precisa de `threshold >= 2`: quórum de 1 é rejeitado mesmo havendo
/// testemunha válida, e o relatório traz o erro do core explicando o mínimo.
/// Os witnesses duplicados contam uma vez só — a contagem é sobre chaves
/// distintas, não sobre entradas da lista.
///
/// Para o caso Rekor, [`crate::rekor::SignedCheckpoint::quorum_report`] monta
/// `subject` e `witnesses` a partir de um checkpoint assinado.
pub fn verify_witness_quorum(
    subject: &[u8],
    witnesses: &[WitnessJson],
    trust_root: &TrustRoot,
    threshold: u32,
) -> QuorumReport {
    // Contado antes do veredito, e reportado mesmo quando o limiar pedido é
    // inválido: é a informação que o `bool` da casca wasm descarta.
    let valid_witnesses = count_valid_witnesses(subject, witnesses, trust_root);
    let outcome = verify_witness_quorum_inner(subject, witnesses, trust_root, threshold);
    QuorumReport {
        ok: outcome.is_ok(),
        valid_witnesses,
        required: threshold,
        error: outcome.err().map(|err| err.to_string()),
    }
}

/// O pipeline completo — as quatro verificações compostas — a partir do JSON da
/// atestação.
///
/// Devolve o [`AttestationReport`] do core, sem tradução: os quatro estágios
/// são avaliados e reportados **independentemente**, então uma atestação que
/// falha só no quórum aparece com `quorum: false` e os outros três `true`.
///
/// Este é o único ponto em que a entrada continua sendo uma `&str` JSON, e por
/// um motivo: o formato do JSON **é** a interface do pipeline (é o mesmo
/// documento que a página envia), e reinterpretá-lo em tipos nativos criaria um
/// segundo parser para o mesmo contrato — exatamente a divergência que o
/// formato único existe para evitar. O trust root, esse sim, é tipado.
pub fn verify_attestation(attestation_json: &str, trust_root: &TrustRoot) -> AttestationReport {
    attestation::verify_attestation_inner(attestation_json, trust_root)
}

/// Os bytes exatos do *subject* de uma atestação — o que o signatário e as
/// testemunhas assinam.
///
/// É o análogo nativo de `wasm::attestation_subject_hex`: existe para que um
/// signatário **nativo** monte os bytes exatos sem reimplementar a
/// concatenação. Uma segunda implementação seria uma segunda chance de
/// divergir, e o que se assina não é o payload cru, e sim
/// `"arkhe-attestation/v1" ∥ SHA-256(payload) ∥ raiz Merkle ∥ leaf_index ∥
/// tree_size`.
pub fn attestation_subject(attestation_json: &str) -> Result<Vec<u8>, VerifyError> {
    attestation::attestation_subject_of(attestation_json).map(|(subject, _)| subject)
}

#[cfg(test)]
mod tests {
    use super::*;
    use arkhe_verify_wasm::encoding::encode_hex;
    use arkhe_verify_wasm::merkle_root_from_leaves;
    use ed25519_dalek::{Signer, SigningKey};

    fn signing_key(seed: u8) -> SigningKey {
        SigningKey::from_bytes(&[seed; 32])
    }

    fn public_hex(seed: u8) -> String {
        encode_hex(signing_key(seed).verifying_key().as_bytes())
    }

    fn trust_root(seeds: &[u8]) -> TrustRoot {
        let keys: Vec<String> = seeds
            .iter()
            .map(|&seed| format!("\"{}\"", public_hex(seed)))
            .collect();
        TrustRoot::parse(&format!("[{}]", keys.join(","))).expect("parse")
    }

    fn witness(seed: u8, message: &[u8]) -> WitnessJson {
        WitnessJson {
            public_key_hex: public_hex(seed),
            signature_hex: encode_hex(&signing_key(seed).sign(message).to_bytes()),
        }
    }

    /// Constrói uma prova de inclusão real de uma árvore de 5 folhas, com a
    /// `ct-merkle` — o mesmo pin que o core usa.
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
        let root = encode_hex(&merkle_root_from_leaves(&leaves));
        (leaves[2].clone(), 2, 5, proof, root)
    }

    // --- sha256 ------------------------------------------------------------

    #[test]
    fn sha256_reports_the_computed_digest_alongside_the_verdict() {
        let report = verify_sha256(
            b"abc",
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        );
        assert!(report.ok);
        assert_eq!(
            report.computed_hex,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert!(report.error.is_none());
    }

    #[test]
    fn sha256_reports_the_computed_digest_even_when_the_expected_one_is_malformed() {
        let report = verify_sha256(b"abc", "não é hex");
        assert!(!report.ok);
        assert_eq!(
            report.computed_hex,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
            "o digest calculado existe mesmo quando o declarado é inválido"
        );
        assert!(report.error.expect("causa").contains("hex"));
    }

    // --- signature ---------------------------------------------------------

    #[test]
    fn signature_accepts_a_trusted_key() {
        let message = b"subject";
        let signature = signing_key(1).sign(message).to_bytes();
        let report = verify_signature(message, &signature, &public_hex(1), &trust_root(&[1, 2]));

        assert!(report.ok);
        assert!(report.trusted);
        assert!(report.error.is_none());
    }

    #[test]
    fn signature_separates_untrusted_from_cryptographically_invalid() {
        let message = b"subject";
        let signature = signing_key(9).sign(message).to_bytes();

        // Assinatura correta, chave fora do trust root.
        let untrusted = verify_signature(message, &signature, &public_hex(9), &trust_root(&[1, 2]));
        assert!(!untrusted.ok);
        assert!(!untrusted.trusted, "a chave não está no trust root");
        assert!(untrusted.error.expect("causa").contains("trust root"));

        // Chave confiável, assinatura de outra mensagem.
        let bad = verify_signature(
            b"outra mensagem",
            &signature,
            &public_hex(9),
            &trust_root(&[9]),
        );
        assert!(!bad.ok);
        assert!(bad.trusted, "a chave está no trust root");
        assert!(bad.error.expect("causa").contains("assinatura"));
    }

    #[test]
    fn an_empty_trust_root_trusts_nothing() {
        let message = b"subject";
        let signature = signing_key(1).sign(message).to_bytes();
        let report = verify_signature(message, &signature, &public_hex(1), &TrustRoot::default());
        assert!(!report.ok);
        assert!(!report.trusted);
    }

    // --- inclusion ---------------------------------------------------------

    #[test]
    fn inclusion_verifies_a_real_proof() {
        let (leaf, leaf_index, tree_size, proof, root) = inclusion_fixture();
        let report = verify_inclusion(&leaf, leaf_index, tree_size, &proof, &root);

        assert!(report.ok, "erro: {:?}", report.error);
        assert_eq!(report.leaf_index, 2);
        assert_eq!(report.tree_size, 5);
        assert_eq!(report.root_hex, root);
    }

    #[test]
    fn inclusion_rejects_a_tampered_proof_and_echoes_the_inputs() {
        let (leaf, leaf_index, tree_size, mut proof, root) = inclusion_fixture();
        proof[0] ^= 0x01;
        let report = verify_inclusion(&leaf, leaf_index, tree_size, &proof, &root);

        assert!(!report.ok);
        assert_eq!(report.leaf_index, leaf_index, "os dados entram no relatório");
        assert_eq!(report.tree_size, tree_size);
        assert_eq!(report.root_hex, root);
        assert!(report.error.is_some());
    }

    #[test]
    fn inclusion_rejects_a_wrong_root() {
        let (leaf, leaf_index, tree_size, proof, _root) = inclusion_fixture();
        let other = encode_hex(&merkle_root_from_leaves(&[b"outra".to_vec()]));
        assert!(!verify_inclusion(&leaf, leaf_index, tree_size, &proof, &other).ok);
    }

    // --- quorum ------------------------------------------------------------

    const SUBJECT: &[u8] = b"arkhe-os: subject under quorum";

    #[test]
    fn quorum_reports_the_number_of_valid_witnesses() {
        let witnesses = vec![witness(1, SUBJECT), witness(2, SUBJECT)];
        let report = verify_witness_quorum(SUBJECT, &witnesses, &trust_root(&[1, 2]), 2);

        assert!(report.ok);
        assert_eq!(report.valid_witnesses, 2);
        assert_eq!(report.required, 2);
        assert!(report.error.is_none());
    }

    #[test]
    fn quorum_reports_one_valid_witness_where_the_wasm_shell_could_only_report_false() {
        let witnesses = vec![witness(1, SUBJECT)];
        let report = verify_witness_quorum(SUBJECT, &witnesses, &trust_root(&[1, 2]), 2);

        assert!(!report.ok);
        assert_eq!(
            report.valid_witnesses, 1,
            "a contagem sobrevive ao veredito — é o que o `bool` da casca wasm perde"
        );
        assert!(report.error.expect("causa").contains("quórum"));
    }

    #[test]
    fn quorum_counts_distinct_keys_not_list_entries() {
        let one = witness(1, SUBJECT);
        let report = verify_witness_quorum(
            SUBJECT,
            &[one.clone(), one],
            &trust_root(&[1]),
            2,
        );
        assert!(!report.ok);
        assert_eq!(report.valid_witnesses, 1);
    }

    #[test]
    fn a_threshold_below_the_minimum_is_rejected_but_still_counted() {
        let witnesses = vec![witness(1, SUBJECT), witness(2, SUBJECT)];
        let report = verify_witness_quorum(SUBJECT, &witnesses, &trust_root(&[1, 2]), 1);

        assert!(!report.ok);
        assert_eq!(report.valid_witnesses, 2, "havia 2 witnesses válidos");
        assert_eq!(report.required, 1);
        assert!(report.error.expect("causa").contains("mínimo"));
    }

    // --- attestation -------------------------------------------------------

    #[test]
    fn a_malformed_attestation_is_reported_rather_than_panicking() {
        let report = verify_attestation("isto não é json", &TrustRoot::default());
        assert!(!report.ok);
        assert_eq!(
            (
                report.sha256,
                report.signature,
                report.inclusion,
                report.quorum
            ),
            (false, false, false, false)
        );
        assert!(report.error.expect("causa").contains("JSON inválido"));
    }

    #[test]
    fn the_native_shell_hands_the_pipeline_the_core_unchanged() {
        // O mesmo documento, a mesma resposta esperada das duas cascas: o
        // contrato do pipeline é o JSON, e a casca nativa não o reinterpreta.
        let report = verify_attestation("{}", &trust_root(&[1]));
        assert!(!report.ok);
        assert!(report.error.is_some());
    }

    #[test]
    fn the_subject_helper_matches_the_core_convention() {
        assert!(attestation_subject("não é json").is_err());

        let (leaf, leaf_index, tree_size, _proof, root) = inclusion_fixture();
        let json = serde_json::json!({
            "payload_b64": base64_encode(&leaf),
            "payload_sha256_hex": encode_hex(&arkhe_verify_wasm::sha256_bytes(&leaf)),
            "merkle_leaf_index": leaf_index,
            "merkle_tree_size": tree_size,
            "merkle_proof_hex": "",
            "merkle_root_hex": root,
            "signer_public_key_hex": public_hex(1),
            "signature_hex": "",
            "quorum_threshold": 2,
        })
        .to_string();

        let subject = attestation_subject(&json).expect("subject");
        assert!(subject.starts_with(b"arkhe-attestation/v1"));
        assert_eq!(subject.len(), "arkhe-attestation/v1".len() + 32 + 32 + 8 + 8);
    }

    fn base64_encode(bytes: &[u8]) -> String {
        use base64::engine::general_purpose::STANDARD;
        use base64::Engine;
        STANDARD.encode(bytes)
    }
}
