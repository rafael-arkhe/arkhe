//! `arkhe-verify` — §2.3 do plano Arkhe OS: a **casca nativa** sobre o core de
//! verificação de `arkhe-verify-wasm`, com o modelo de dados no formato Rekor/CT
//! e um adaptador HTTP para uma instância Rekor.
//!
//! # Um core, duas cascas
//!
//! A verificação **não** é reimplementada aqui. Ela vive no core de
//! `arkhe-verify-wasm` — que declara `crate-type = ["cdylib", "rlib"]`
//! justamente para ter duas cascas —, e este crate depende dele como `rlib`:
//!
//! ```text
//!                        arkhe-verify-wasm
//!                        ├── hash::verify_sha256_inner
//!                        ├── signature::verify_signature_inner
//!                        ├── merkle::verify_inclusion_inner
//!                        ├── quorum::verify_witness_quorum_inner
//!                        └── attestation::verify_attestation_inner
//!                                 │                │
//!          casca #[wasm_bindgen] ─┘                └─ casca nativa
//!          (wasm.rs: bool / String JSON)              (facade.rs: relatórios)
//! ```
//!
//! As duas cascas chamam as **mesmas** funções `*_inner`. O que muda é só a
//! forma da entrada e da saída: a casca wasm traduz de `String`/`&[u8]`/`u32` e
//! devolve `bool` (ou uma `String` JSON), porque é o que o JavaScript consome;
//! esta casca recebe tipos nativos e devolve relatórios estruturados
//! ([`report`]).
//!
//! O ponto exato do reuso é [`facade`], onde cada uma das cinco funções chama
//! `arkhe_verify_wasm::*_inner` — a mesma chamada que o `wasm.rs` faz. Não há
//! uma segunda implementação de SHA-256, de Ed25519, do RFC 6962 nem da
//! contagem de quórum nesta crate; e nem de hex ou base64, que também vêm do
//! core (`arkhe_verify_wasm::encoding`).
//!
//! # O que esta crate acrescenta
//!
//! 1. **A fachada nativa** ([`facade`]): as cinco verificações com tipos
//!    nativos e relatórios estruturados.
//! 2. **O modelo Rekor/CT** ([`rekor`]): [`LogEntry`], [`InclusionProof`],
//!    [`Checkpoint`]/[`SignedCheckpoint`] com as assinaturas de witness — os
//!    tipos que montam os argumentos de `verify_inclusion` e
//!    `verify_witness_quorum` a partir do que o log publica.
//! 3. **O adaptador HTTP** ([`RekorClient`]): as duas chamadas ao log, contra
//!    uma instância configurável.
//! 4. **A verificação de um modelo GGUF** ([`gguf`]): o cabeçalho lido de
//!    bytes, o digest SHA-256 do modelo para conferência contra um esperado, e
//!    a ligação a uma atestação existente — que delega o pipeline a
//!    [`verify_attestation`]. A API inteira é sobre `&[u8]`: não há `Path` nem
//!    I/O, porque o core compila para wasm e quem lê o arquivo é quem chama.
//!
//! # O que esta crate **não** faz
//!
//! - Não verifica `signedEntryTimestamp` (a assinatura do log sobre a entrada).
//! - Não verifica **prova de consistência** entre tamanhos de árvore: o core
//!   não tem um verificador de consistência, e escrever um aqui seria duplicar
//!   lógica de verificação fora do core. [`RekorClient::consistency_proof`]
//!   busca os bytes; verificá-los não está implementado.
//! - Não faz chamada de rede em nenhum teste. O adaptador é exercitado contra
//!   um servidor local (`mockito`), como o `arkhe-orcid` faz.
//! - Não instala `sigstore`. Ver a seção do README sobre a rota opcional e o
//!   pin `0.13` com `features = ["wasm"]`.
//! - Não valida o **conteúdo** de um GGUF além do cabeçalho: [`gguf`] lê 24
//!   bytes de estrutura e o digest dos bytes, e não os pares chave-valor, os
//!   descritores de tensor nem o bloco de dados. Ver a nota "o que este módulo
//!   não prova" em [`gguf`].
//!
//! # Rede
//!
//! Só [`RekorClient`] toca a rede, e só os dois métodos dele. Todo o resto é
//! puro sobre dados já obtidos — inclusive todas as verificações.

#![deny(unsafe_code)]
#![warn(missing_docs)]

/// Verificação de calibração — decisões com probabilidade e incerteza.
///
/// O `verify` é *fail-closed*: uma decisão que não satisfaça os limiares, ou
/// que venha sem metadados de calibração, é rejeitada. Uma decisão
/// **vendor-tested** é aceita pelo `verify` e **só** rejeitada pelo
/// `verify_independent` — a diferença é intencional e está fixada nos testes do
/// módulo (`calibrated_decision_accepts_valid` afirma explicitamente o `is_ok`).
pub mod calibration;
pub mod error;
pub mod facade;
pub mod gguf;
pub mod rekor;
pub mod report;

pub use calibration::{CalibratedDecision, CalibrationError, CalibrationMetadata, EvaluationType};
pub use error::RekorError;
pub use facade::{
    attestation_subject, verify_attestation, verify_inclusion, verify_sha256, verify_signature,
    verify_witness_quorum,
};
pub use gguf::{
    model_digest, parse_header, verify_model_attestation, verify_model_digest,
    GgufAttestationReport, GgufHeaderReport, GgufModelReport, GGUF_MAGIC, HEADER_LEN,
    SUPPORTED_VERSIONS,
};
pub use rekor::{
    parse_signed_checkpoint, Checkpoint, ConsistencyProof, InclusionProof, LogEntriesResponse,
    LogEntry, LogEntryVerification, RekorClient, SignedCheckpoint, WitnessKeyring,
    WitnessSignature, DEFAULT_BASE_URL, KEY_ID_LEN,
};
pub use report::{InclusionReport, QuorumReport, Sha256Report, SignatureReport};

/// O relatório do pipeline de atestação, do core.
///
/// Reexportado do core em vez de redefinido: é o mesmo tipo que a casca wasm
/// serializa para o JavaScript, e ter um segundo tipo com a mesma forma criaria
/// a chance de os dois discordarem.
pub use arkhe_verify_wasm::AttestationReport;

/// O conjunto de chaves públicas em que a verificação confia, do core.
///
/// Ver a nota de [`facade`]: como o tipo só existe já interpretado, um trust
/// root malformado não tem como chegar a uma verificação.
pub use arkhe_verify_wasm::TrustRoot;

/// Um witness, como o `verify_witness_quorum` do core o consome.
pub use arkhe_verify_wasm::WitnessJson;

/// A falha de interpretação de uma entrada de verificação, do core.
pub use arkhe_verify_wasm::VerifyError;

/// As codificações de fio do core — hex para valores de tamanho fixo e curto
/// (chaves, assinaturas, raízes, digests, provas) e base64 para o payload.
///
/// Reexportadas para que quem usa a casca nativa não precise de uma **segunda**
/// dependência (`arkhe-verify-wasm`) só para montar uma entrada: a fachada
/// reexporta os tipos, e os codificadores vêm com eles. É a mesma convenção da
/// casca wasm; ver o módulo `encoding` do core.
pub use arkhe_verify_wasm::encoding;

/// A raiz Merkle RFC 6962 de uma lista de folhas, do core.
///
/// É um auxiliar de **integração** (quem produz uma entrada precisa saber a
/// raiz), não uma verificação — mas vem do mesmo core pelos mesmos motivos de
/// [`encoding`]. A árvore vazia devolve `SHA-256("")`, como manda o RFC 6962
/// §2.1.
pub use arkhe_verify_wasm::merkle_root_from_leaves;
