//! Os relatórios estruturados que a casca nativa devolve.
//!
//! A casca `#[wasm_bindgen]` de `arkhe-verify-wasm` devolve `bool` (ou uma
//! `String` JSON) porque é o que o JavaScript consome. A casca nativa devolve
//! estes tipos: o mesmo veredito, com os **dados contra os quais** o veredito
//! foi produzido.
//!
//! A diferença não é cosmética. Um `false` obriga quem chama a refazer a
//! comparação — recalcular o digest, recontar os witnesses — para descobrir *o
//! que* não bateu; e no caso do quórum, o número de witnesses válidos não é
//! recuperável a partir do `bool` de forma nenhuma. Aqui ele vem no relatório.
//!
//! Nenhum destes tipos falha: uma entrada malformada produz um relatório com
//! `ok: false` e a causa em `error`, nunca um `Err`. O `Err` existe apenas em
//! [`crate::RekorError`], para o que **impede** a verificação de acontecer
//! (rede, resposta, formato).

use serde::Serialize;

/// O veredito de `verify_sha256`: o digest calculado e o declarado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Sha256Report {
    /// `true` se `SHA-256(data)` é igual ao digest declarado.
    pub ok: bool,
    /// O digest que o chamador declarou, como veio.
    pub expected_hex: String,
    /// O digest efetivamente calculado de `data`, em hex minúsculo.
    pub computed_hex: String,
    /// A causa da falha, ou `None` se passou.
    pub error: Option<String>,
}

/// O veredito de `verify_signature`: se a assinatura confere **e** se a chave
/// está no trust root.
///
/// Os dois campos separados são o ponto: uma assinatura criptograficamente
/// válida por uma chave fora do trust root é uma falha, e o relatório diz que
/// foi *isso* — não que a assinatura é inválida, o que seria falso.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SignatureReport {
    /// `true` somente se a assinatura confere e a chave é confiável.
    pub ok: bool,
    /// A chave pública apresentada, como veio.
    pub public_key_hex: String,
    /// `true` se a chave apresentada está no trust root, independentemente de a
    /// assinatura conferir.
    pub trusted: bool,
    /// A causa da falha, ou `None` se passou.
    pub error: Option<String>,
}

/// O veredito de `verify_inclusion`: a prova contra a raiz declarada.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InclusionReport {
    /// `true` se a prova reconstrói a raiz declarada.
    pub ok: bool,
    /// A posição da folha verificada (0-based).
    pub leaf_index: u64,
    /// O número de folhas da árvore a que a raiz pertence.
    pub tree_size: u64,
    /// A raiz declarada, como veio.
    pub root_hex: String,
    /// A causa da falha, ou `None` se passou.
    pub error: Option<String>,
}

/// O veredito de `verify_witness_quorum`: quantos witnesses distintos e
/// confiáveis assinaram, e quantos eram exigidos.
///
/// `valid_witnesses` é reportado mesmo quando `ok` é `true` (é a margem do
/// quórum) e mesmo quando o limiar pedido é inválido — nos dois casos é
/// informação que o `bool` da casca wasm descarta.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct QuorumReport {
    /// `true` se ao menos `required` witnesses distintos e confiáveis
    /// assinaram.
    pub ok: bool,
    /// Quantos witnesses distintos, confiáveis e com assinatura válida.
    pub valid_witnesses: u32,
    /// O limiar pedido pelo chamador.
    pub required: u32,
    /// A causa da falha, ou `None` se passou.
    pub error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_reports_serialize_to_the_documented_shape() {
        let report = Sha256Report {
            ok: true,
            expected_hex: "ab".to_string(),
            computed_hex: "ab".to_string(),
            error: None,
        };
        let value: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&report).expect("serializa"))
                .expect("parse");
        assert_eq!(value["ok"], true);
        assert_eq!(value["expected_hex"], "ab");
        assert!(value["error"].is_null());
    }

    #[test]
    fn a_failed_report_carries_the_stage_and_the_cause() {
        let report = SignatureReport {
            ok: false,
            public_key_hex: "1f8f".to_string(),
            trusted: false,
            error: Some("chave pública apresentada não está no trust root".to_string()),
        };
        let value: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&report).expect("serializa"))
                .expect("parse");
        assert_eq!(value["ok"], false);
        assert_eq!(value["trusted"], false);
        assert!(value["error"].as_str().expect("string").contains("trust root"));
    }
}
