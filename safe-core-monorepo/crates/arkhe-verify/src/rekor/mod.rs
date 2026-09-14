//! O modelo de dados no formato Rekor/CT.
//!
//! Estes tipos são a **entrada** das verificações de inclusão e de quórum: o
//! `verify_inclusion` do core consome `(folha, leaf_index, tree_size, prova,
//! raiz)`, e o `verify_witness_quorum` consome `(subject, witnesses, trust
//! root, limiar)`. O que esta camada faz é montar esses argumentos a partir do
//! que o Rekor publica — sem reimplementar nenhuma das duas verificações.
//!
//! # Procedência: descrição da API, não uma instância real
//!
//! **Nada aqui foi verificado contra uma instância Rekor real.** Os campos e os
//! nomes seguem a descrição pública da API do Rekor
//! (`GET /api/v1/log/entries?logIndex=N`, `GET /api/v1/log/proof`) e a
//! convenção de *signed notes* do Certificate Transparency no que diz respeito
//! ao checkpoint. Ver a seção "Não verificado" do README: em particular, o
//! formato do checkpoint assinado (a linha `— <nome> <blob>`, com o `key id` de
//! 4 bytes concatenado à assinatura dentro do mesmo base64) é uma leitura da
//! convenção, e o modo de falha é seguro — um formato diferente produz um
//! `key id` que não está no [`WitnessKeyring`], logo
//! [`RekorError::UnknownWitnessKey`], não um veredito errado.
//!
//! # Rede
//!
//! **Nada neste módulo faz rede.** Os tipos são dados puros; quem busca é
//! [`client::RekorClient`], e só ele. Todo o resto desta crate é puro sobre
//! dados já obtidos, e é por isso que os testes conseguem exercitar tudo sem
//! tocar a rede.

pub mod client;
pub mod note;

pub use client::{RekorClient, DEFAULT_BASE_URL};
pub use note::{
    parse_signed_checkpoint, Checkpoint, SignedCheckpoint, WitnessKeyring, WitnessSignature,
    KEY_ID_LEN,
};

use std::collections::BTreeMap;

use arkhe_verify_wasm::encoding::{decode_base64, decode_hex};
use serde::{Deserialize, Serialize};

use crate::error::RekorError;
use crate::report::InclusionReport;

/// Uma entrada do log de transparência, como o Rekor a publica.
///
/// `body` é o conteúdo **em base64** (é assim que o Rekor o entrega; a
/// decodificação é [`LogEntry::leaf_bytes`]), e é o `body` decodificado — não
/// um digest — que é a folha da árvore Merkle, conforme o RFC 6962.
///
/// A resposta de `GET /api/v1/log/entries?logIndex=N` é, na verdade, **um mapa
/// de UUID para entrada** com um único elemento; o desembrulho é feito em
/// [`client::RekorClient::log_entry`]. O UUID em si não é modelado aqui porque
/// nenhum passo da verificação o usa.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    /// A posição da entrada no log.
    pub log_index: u64,
    /// O `body` em base64 — a folha da árvore, depois de decodificado.
    pub body: String,
    /// Quando a entrada foi integrada, em segundos desde a época Unix.
    pub integrated_time: u64,
    /// O identificador do log, em hex.
    #[serde(rename = "logID")]
    pub log_id: String,
    /// A verificação que o log anexou à entrada, quando presente.
    #[serde(default)]
    pub verification: Option<LogEntryVerification>,
}

/// O bloco `verification` de uma [`LogEntry`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntryVerification {
    /// A prova de inclusão da entrada.
    #[serde(default)]
    pub inclusion_proof: Option<InclusionProof>,
    /// A assinatura do log sobre a entrada (`signedEntryTimestamp`), em base64.
    ///
    /// Não é verificada por esta crate: verificar a SET exigiria a chave do log
    /// e o formato exato do envelope assinado, que o core não modela. Fica
    /// registrado em vez de silenciado.
    #[serde(default)]
    pub signed_entry_timestamp: Option<String>,
}

/// Uma prova de inclusão no formato do Rekor.
///
/// Os hashes irmãos vêm **em hex**, um por elemento; o core do
/// `arkhe-verify-wasm` consome a concatenação crua dos digests (n × 32 bytes),
/// que é o que [`InclusionProof::proof_bytes`] produz.
///
/// Note a assimetria de codificação dentro do próprio Rekor: aqui `root_hash` é
/// hex, enquanto o `root_hash` de um [`Checkpoint`] é base64 (convenção de
/// *signed note*). Não é um erro de transcrição — são dois formatos da mesma
/// API, e misturá-los produziria uma comparação entre codificações diferentes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InclusionProof {
    /// A posição da folha provada.
    pub log_index: u64,
    /// A raiz da árvore, em hex.
    pub root_hash: String,
    /// O número de folhas da árvore a que a raiz pertence.
    pub tree_size: u64,
    /// Os digests irmãos, em hex, do mais baixo ao mais alto.
    #[serde(default)]
    pub hashes: Vec<String>,
    /// O checkpoint assinado, quando o log o anexa à prova.
    #[serde(default)]
    pub checkpoint: Option<String>,
}

impl InclusionProof {
    /// A prova no formato que o core consome: os digests irmãos concatenados.
    ///
    /// Falha em [`RekorError::Encoding`] se algum elemento não for hex.
    pub fn proof_bytes(&self) -> Result<Vec<u8>, RekorError> {
        let mut proof = Vec::with_capacity(self.hashes.len() * 32);
        for hash in &self.hashes {
            proof.extend_from_slice(&decode_hex(hash).map_err(encoding_error)?);
        }
        Ok(proof)
    }
}

/// A prova de consistência que `GET /api/v1/log/proof` devolve.
///
/// **Não é verificada por esta crate.** O core do `arkhe-verify-wasm` cobre
/// hash, assinatura Ed25519, inclusão RFC 6962 e quórum — não cobre prova de
/// consistência entre dois tamanhos de árvore. Modelar o tipo e devolver os
/// bytes é o que dá para fazer sem duplicar (nem inventar) lógica de
/// verificação; verificar consistência exigiria um verificador novo no core,
/// não uma segunda implementação aqui.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsistencyProof {
    /// A raiz da árvore, em hex.
    pub root_hash: String,
    /// Os digests da prova, em hex.
    #[serde(default)]
    pub hashes: Vec<String>,
}

impl LogEntry {
    /// A folha da árvore: o `body` decodificado de base64.
    pub fn leaf_bytes(&self) -> Result<Vec<u8>, RekorError> {
        decode_base64(&self.body).map_err(encoding_error)
    }

    /// A prova de inclusão anexada à entrada, se houver.
    pub fn inclusion_proof(&self) -> Option<&InclusionProof> {
        self.verification
            .as_ref()
            .and_then(|verification| verification.inclusion_proof.as_ref())
    }

    /// Verifica a inclusão desta entrada na árvore Merkle do log.
    ///
    /// Monta os argumentos a partir do `body` e da prova anexada, e delega a
    /// [`crate::verify_inclusion`] — o mesmo core que a casca wasm chama.
    ///
    /// `Err` (e não `ok: false`) quando **não há** o que verificar: entrada sem
    /// `inclusionProof`, ou `body`/`hashes` que não decodificam. Isso é o que
    /// *impede* a verificação de acontecer, e não um veredito — ver a nota em
    /// [`crate::error`].
    pub fn inclusion_report(&self) -> Result<InclusionReport, RekorError> {
        let proof = self.inclusion_proof().ok_or_else(|| RekorError::MalformedLogEntry {
            reason: format!(
                "a entrada do logIndex {} não traz inclusionProof",
                self.log_index
            ),
        })?;
        let leaf = self.leaf_bytes()?;
        let proof_bytes = proof.proof_bytes()?;

        Ok(crate::verify_inclusion(
            &leaf,
            proof.log_index,
            proof.tree_size,
            &proof_bytes,
            &proof.root_hash,
        ))
    }
}

/// A resposta de `GET /api/v1/log/entries?logIndex=N`: um mapa de UUID para
/// entrada.
pub type LogEntriesResponse = BTreeMap<String, LogEntry>;

/// Converte uma falha de decodificação do core no erro desta crate.
pub(crate) fn encoding_error(err: arkhe_verify_wasm::VerifyError) -> RekorError {
    RekorError::Encoding(err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use arkhe_verify_wasm::encoding::encode_hex;

    fn entry_with_proof(hashes: Vec<String>) -> LogEntry {
        LogEntry {
            log_index: 42,
            body: "aGVsbG8=".to_string(),
            integrated_time: 1_700_000_000,
            log_id: "c0d23d6a".to_string(),
            verification: Some(LogEntryVerification {
                inclusion_proof: Some(InclusionProof {
                    log_index: 42,
                    root_hash: encode_hex(&[0xAB; 32]),
                    tree_size: 100,
                    hashes,
                    checkpoint: None,
                }),
                signed_entry_timestamp: None,
            }),
        }
    }

    #[test]
    fn a_log_entry_round_trips_through_the_rekor_json_shape() {
        let json = serde_json::json!({
            "logIndex": 42,
            "body": "aGVsbG8=",
            "integratedTime": 1_700_000_000u64,
            "logID": "c0d23d6a",
            "verification": {
                "inclusionProof": {
                    "logIndex": 42,
                    "rootHash": "abab",
                    "treeSize": 100,
                    "hashes": ["cd", "ef"],
                    "checkpoint": "rekor.sigstore.dev - 42\n100\ncm9vdA==\n"
                },
                "signedEntryTimestamp": "c2ln"
            }
        })
        .to_string();

        let entry: LogEntry = serde_json::from_str(&json).expect("desserializa");
        assert_eq!(entry.log_index, 42);
        assert_eq!(entry.body, "aGVsbG8=");
        assert_eq!(entry.integrated_time, 1_700_000_000);
        assert_eq!(entry.log_id, "c0d23d6a");

        let proof = entry.inclusion_proof().expect("a prova está anexada");
        assert_eq!(proof.log_index, 42);
        assert_eq!(proof.tree_size, 100);
        assert_eq!(proof.hashes, vec!["cd", "ef"]);
        assert!(proof.checkpoint.is_some());
        assert_eq!(
            entry
                .verification
                .as_ref()
                .and_then(|v| v.signed_entry_timestamp.as_deref()),
            Some("c2ln")
        );

        // E o round-trip de volta preserva os nomes camelCase do Rekor.
        let restored: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&entry).expect("serializa")).expect("parse");
        assert_eq!(restored["logIndex"], 42);
        assert_eq!(restored["logID"], "c0d23d6a");
        assert_eq!(restored["integratedTime"], 1_700_000_000u64);
        assert_eq!(restored["verification"]["inclusionProof"]["rootHash"], "abab");
    }

    #[test]
    fn the_leaf_is_the_decoded_body() {
        let entry = entry_with_proof(Vec::new());
        assert_eq!(entry.leaf_bytes().expect("decodifica"), b"hello");
    }

    #[test]
    fn a_body_that_is_not_base64_is_an_encoding_error() {
        let mut entry = entry_with_proof(Vec::new());
        entry.body = "!!!não é base64!!!".to_string();
        assert!(matches!(
            entry.leaf_bytes(),
            Err(RekorError::Encoding(_))
        ));
    }

    #[test]
    fn the_proof_bytes_are_the_concatenated_sibling_digests() {
        let proof = InclusionProof {
            log_index: 0,
            root_hash: String::new(),
            tree_size: 2,
            hashes: vec![encode_hex(&[0x01; 32]), encode_hex(&[0x02; 32])],
            checkpoint: None,
        };
        let bytes = proof.proof_bytes().expect("decodifica");
        assert_eq!(bytes.len(), 64);
        assert_eq!(&bytes[..32], &[0x01; 32]);
        assert_eq!(&bytes[32..], &[0x02; 32]);
    }

    #[test]
    fn a_non_hex_sibling_hash_is_an_encoding_error() {
        let proof = InclusionProof {
            log_index: 0,
            root_hash: String::new(),
            tree_size: 2,
            hashes: vec!["não é hex".to_string()],
            checkpoint: None,
        };
        assert!(matches!(proof.proof_bytes(), Err(RekorError::Encoding(_))));
    }

    #[test]
    fn an_entry_without_an_inclusion_proof_cannot_be_checked() {
        let mut entry = entry_with_proof(Vec::new());
        entry.verification = None;
        assert!(matches!(
            entry.inclusion_report(),
            Err(RekorError::MalformedLogEntry { .. })
        ));
    }

    #[test]
    fn an_entry_with_a_single_leaf_proof_verifies_end_to_end() {
        // Uma árvore de uma folha tem prova vazia e raiz conhecida — não precisa
        // do `ct-merkle` para montar, e exercita o caminho Rekor → core inteiro.
        let leaf = b"hello".to_vec();
        let root = arkhe_verify_wasm::merkle_root_from_leaves(std::slice::from_ref(&leaf));

        let entry = LogEntry {
            log_index: 0,
            body: "aGVsbG8=".to_string(),
            integrated_time: 1,
            log_id: "c0d23d6a".to_string(),
            verification: Some(LogEntryVerification {
                inclusion_proof: Some(InclusionProof {
                    log_index: 0,
                    root_hash: encode_hex(&root),
                    tree_size: 1,
                    hashes: Vec::new(),
                    checkpoint: None,
                }),
                signed_entry_timestamp: None,
            }),
        };

        let report = entry.inclusion_report().expect("verifica");
        assert!(report.ok, "erro: {:?}", report.error);
        assert_eq!(report.leaf_index, 0);
        assert_eq!(report.tree_size, 1);
    }
}
