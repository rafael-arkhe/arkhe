//! Verificação de um modelo **GGUF**: o cabeçalho, o digest e a ligação a uma
//! atestação existente.
//!
//! É o caminho crítico do plano — dado um modelo em bytes, responder três
//! perguntas, cada uma com o seu próprio campo de veredito:
//!
//! 1. **O cabeçalho é estruturalmente válido?** ([`parse_header`]) — magic
//!    `GGUF`, versão interpretável, contagem de tensores e de pares
//!    chave-valor presentes.
//! 2. **Qual é o digest do modelo?** ([`model_digest`], [`verify_model_digest`])
//!    — SHA-256 sobre os bytes, para conferência contra um esperado.
//! 3. **Uma atestação existente liga este modelo ao log?**
//!    ([`verify_model_attestation`]) — o digest do modelo é o que o *subject*
//!    da atestação amarra, e o pipeline completo roda sobre ela.
//!
//! # Só bytes, nunca caminhos
//!
//! **Toda a API opera sobre `&[u8]`.** Não há um `Path`, nem `std::fs`, nem
//! qualquer I/O neste módulo — quem lê o arquivo é quem chama. Não é uma
//! preferência de estilo: o core de verificação vive em
//! `arkhe-verify-wasm`, compila para `wasm32-unknown-unknown`, e uma API que
//! recebesse um caminho não teria como funcionar do outro lado da mesma
//! fachada.
//!
//! # O que cada parte reusa
//!
//! Nada de criptografia, de decodificação ou de pipeline é reimplementado
//! aqui:
//!
//! | Parte | Reusa |
//! |:---|:---|
//! | conferência do digest | [`crate::facade::verify_sha256`] (`facade.rs:51`) |
//! | digest calculado | `arkhe_verify_wasm::sha256_hex` — a mesma primitiva que a fachada usa em `facade.rs:56` |
//! | pipeline da atestação | [`crate::facade::verify_attestation`] (`facade.rs:155`) |
//! | decodificação do payload | `arkhe_verify_wasm::encoding::decode_base64` (`encoding.rs:58` do core) |
//! | hex do magic inválido | `arkhe_verify_wasm::encoding::encode_hex` (`encoding.rs:53` do core) |
//!
//! O único código novo é o **leitor do cabeçalho** — 24 bytes — e a costura
//! entre as três perguntas.
//!
//! # O que este módulo **não** prova
//!
//! Vale ser explícito, porque "verificar um modelo GGUF" soa a mais do que
//! isto:
//!
//! - **O cabeçalho válido não é um modelo válido.** O leitor confere 24 bytes
//!   de estrutura: magic, versão, dois contadores. Ele **não** lê os pares
//!   chave-valor, não lê os descritores de tensor, não valida o alinhamento e
//!   não olha um byte do bloco de dados. Um arquivo com cabeçalho correto
//!   seguido de lixo passa em [`parse_header`] — e é por isso que o veredito
//!   completo de [`verify_model_digest`] não é o cabeçalho sozinho.
//! - **Quem identifica os bytes é o digest**, não o cabeçalho. O cabeçalho diz
//!   "isto se parece com um GGUF"; o digest diz *qual* arquivo é. A conferência
//!   contra um esperado é a afirmação forte; a leitura do cabeçalho é o
//!   diagnóstico que explica uma recusa.
//! - **Não há verificação de quantização, de tensor, nem de integridade dos
//!   pesos.** Nada disso está no core, e inventar aqui seria uma segunda
//!   implementação de verificação fora dele.
//!
//! # O leitor do cabeçalho segue o de referência
//!
//! O layout é o que `llama.cpp/ggml/include/gguf.h` documenta (magic de 4
//! bytes, versão `u32`, número de tensores, número de pares chave-valor) e o
//! que `llama.cpp/ggml/src/gguf.cpp` lê de fato. Duas decisões vêm de lá, e não
//! de invenção própria:
//!
//! - Os contadores são lidos como **`i64`** e um valor negativo é **recusado
//!   com causa** (`gguf.cpp` rejeita `n_tensors < 0`), em vez de ser
//!   reinterpretado como um `u64` gigante. O relatório traz `u64` só depois da
//!   validação.
//! - Uma versão com a metade alta preenchida (`0x03000000`) é diagnosticada
//!   como **endianness trocada**, que é exatamente o que o leitor de referência
//!   faz ao encontrar `version & 0x0000FFFF == 0`.
//!
//! Versões interpretadas: **2 e 3** ([`SUPPORTED_VERSIONS`]). A 1 é
//! explicitamente recusada pelo leitor de referência ("GGUFv1 is no longer
//! supported") e a 4+ está acima do que ele suporta; esta casca recusa as duas
//! com causa, em vez de tentar adivinhar um layout.

use serde::Serialize;

use arkhe_verify_wasm::encoding::{decode_base64, encode_hex};
use arkhe_verify_wasm::{sha256_hex, Attestation, AttestationReport, TrustRoot};

use crate::facade::{verify_attestation, verify_sha256};
use crate::report::Sha256Report;

/// O magic de um arquivo GGUF: `GGUF`, quatro bytes.
pub const GGUF_MAGIC: [u8; 4] = *b"GGUF";

/// O tamanho do cabeçalho: magic (4) + versão (4) + dois contadores (8 + 8).
pub const HEADER_LEN: usize = 24;

/// As versões de GGUF cujo cabeçalho esta casca interpreta.
///
/// São as que têm os contadores como inteiros de 64 bits nos deslocamentos 8 e
/// 16. Ver a nota de versões no topo do módulo: a 1 não é interpretada (o
/// leitor de referência a recusa) e nada acima da 3 é.
pub const SUPPORTED_VERSIONS: [u32; 2] = [2, 3];

/// O veredito de [`parse_header`]: o cabeçalho lido campo a campo.
///
/// Cada campo é um `Option` porque o leitor é **incremental**: um arquivo
/// truncado reporta exatamente os campos cujos bytes estavam presentes, e
/// `None` para o primeiro que faltou. É a mesma escolha de
/// [`crate::report::Sha256Report`], que reporta o digest calculado mesmo
/// quando o esperado é inválido — o dado que existe não é escondido pela
/// falha.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GgufHeaderReport {
    /// `true` somente se o cabeçalho inteiro é válido: magic correto, versão
    /// interpretável e os dois contadores presentes e não negativos.
    pub ok: bool,
    /// `true` se os quatro primeiros bytes são `GGUF`.
    pub magic_ok: bool,
    /// Quantos bytes o chamador entregou — a aritmética do truncamento fica
    /// visível aqui, sem precisar deduzir do erro.
    pub available_bytes: usize,
    /// A versão declarada, se os bytes dela estavam presentes.
    pub version: Option<u32>,
    /// O número de tensores, se presente e não negativo.
    pub tensor_count: Option<u64>,
    /// O número de pares chave-valor, se presente e não negativo.
    pub metadata_kv_count: Option<u64>,
    /// A causa da recusa, ou `None` se passou.
    pub error: Option<String>,
}

/// O veredito de [`verify_model_digest`]: o digest do modelo **e** o
/// cabeçalho, em campos separados.
///
/// Os dois fatos não se implicam: um arquivo pode ter o digest esperado e o
/// cabeçalho truncado (o caso do `arkhe.gguf` real, de 16 bytes). Quem chama
/// vê qual dos dois falhou em vez de um `false` só.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GgufModelReport {
    /// `true` somente se o cabeçalho é válido **e** o digest declarado confere.
    pub ok: bool,
    /// O cabeçalho lido dos mesmos bytes.
    pub header: GgufHeaderReport,
    /// O veredito de [`crate::facade::verify_sha256`] sobre os bytes do modelo
    /// e o digest declarado.
    pub sha256: Sha256Report,
}

/// O veredito de [`verify_model_attestation`]: a atestação liga *este* modelo?
///
/// Quatro fatos independentes, cada um no seu campo: o cabeçalho do modelo, a
/// conferência do digest declarado, a identidade dos bytes do payload e o
/// pipeline da atestação nas suas quatro etapas.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GgufAttestationReport {
    /// `true` somente se o cabeçalho é válido, o digest declarado confere, os
    /// bytes do payload **são** os do modelo e o pipeline da atestação passa
    /// inteiro.
    pub ok: bool,
    /// O cabeçalho lido dos bytes do modelo.
    pub header: GgufHeaderReport,
    /// O digest SHA-256 calculado dos bytes do modelo, em hex.
    ///
    /// Quando [`Self::link`] existe, ele é igual a `link.computed_hex` por
    /// construção — os dois vêm do mesmo cálculo —, e existe separado para que
    /// o digest apareça mesmo quando a atestação não é JSON.
    pub model_digest_hex: String,
    /// O digest que a atestação declara para o seu payload
    /// (`payload_sha256_hex`), se a atestação interpretou.
    pub declared_payload_digest_hex: Option<String>,
    /// A conferência do digest do modelo contra o declarado pela atestação —
    /// [`crate::facade::verify_sha256`]. `None` se a atestação não interpretou.
    pub link: Option<Sha256Report>,
    /// `true` se decodificar `payload_b64` produz **exatamente** os bytes do
    /// modelo. É a checagem sem digest: identidade de bytes, não concordância
    /// de hashes.
    pub payload_is_model: bool,
    /// O relatório do core, sem tradução — o mesmo que
    /// [`crate::facade::verify_attestation`] devolve.
    pub attestation: AttestationReport,
    /// A causa da recusa, ou `None` se passou.
    pub error: Option<String>,
}

/// Lê o cabeçalho GGUF de `bytes`.
///
/// Nunca devolve `Err`: magic errado, versão não interpretada, contador
/// negativo ou arquivo truncado produzem um relatório com `ok: false` e a
/// causa em `error`, com todos os campos que os bytes presentes permitiram
/// ler.
///
/// Exemplos de entrada que **não** são um erro de programa, e sim um resultado:
/// um arquivo de 3 bytes, um `.gguf` que na verdade é um script, um arquivo de
/// 16 bytes que tem magic e versão mas não os contadores.
pub fn parse_header(bytes: &[u8]) -> GgufHeaderReport {
    let available_bytes = bytes.len();

    // Magic: sem 4 bytes não há nem como dizer se o arquivo é um GGUF.
    let magic = match bytes.get(..GGUF_MAGIC.len()) {
        Some(magic) => magic,
        None => {
            return GgufHeaderReport {
                ok: false,
                magic_ok: false,
                available_bytes,
                version: None,
                tensor_count: None,
                metadata_kv_count: None,
                error: Some(format!(
                    "cabeçalho truncado: {available_bytes} byte(s), e o magic `GGUF` ocupa {}",
                    GGUF_MAGIC.len()
                )),
            };
        }
    };

    if magic != GGUF_MAGIC {
        return GgufHeaderReport {
            ok: false,
            magic_ok: false,
            available_bytes,
            version: None,
            tensor_count: None,
            metadata_kv_count: None,
            error: Some(format!(
                "magic inválido: esperado `GGUF`, encontrado `{}` ({})",
                String::from_utf8_lossy(magic),
                encode_hex(magic)
            )),
        };
    }

    let version = match read_u32(bytes, 4) {
        Some(version) => version,
        None => {
            return GgufHeaderReport {
                ok: false,
                magic_ok: true,
                available_bytes,
                version: None,
                tensor_count: None,
                metadata_kv_count: None,
                error: Some(format!(
                    "cabeçalho truncado: {available_bytes} byte(s) — o campo de versão termina em 8"
                )),
            };
        }
    };

    // A versão decide o layout, então ela é checada antes dos contadores: ler
    // u64 onde a versão 1 tem u32 daria números com aparência de válidos.
    if !SUPPORTED_VERSIONS.contains(&version) {
        return GgufHeaderReport {
            ok: false,
            magic_ok: true,
            available_bytes,
            version: Some(version),
            tensor_count: None,
            metadata_kv_count: None,
            error: Some(unsupported_version_cause(version)),
        };
    }

    // Contadores: `i64` no fio, `u64` no relatório só depois da validação —
    // ver a nota do leitor de referência no topo do módulo.
    let mut error = None;
    let tensor_count = match read_i64(bytes, 8) {
        Some(count) if count >= 0 => Some(count as u64),
        Some(count) => {
            error = Some(format!(
                "contador de tensores negativo: {count} — o campo não é um u64 reinterpretado"
            ));
            None
        }
        None => None,
    };
    let metadata_kv_count = match read_i64(bytes, 16) {
        Some(count) if count >= 0 => Some(count as u64),
        Some(count) => {
            if error.is_none() {
                error = Some(format!("contador de pares chave-valor negativo: {count}"));
            }
            None
        }
        None => None,
    };

    if error.is_none() && (tensor_count.is_none() || metadata_kv_count.is_none()) {
        error = Some(format!(
            "cabeçalho truncado: {available_bytes} byte(s), e o cabeçalho da versão {version} ocupa {HEADER_LEN}"
        ));
    }

    GgufHeaderReport {
        ok: error.is_none(),
        magic_ok: true,
        available_bytes,
        version: Some(version),
        tensor_count,
        metadata_kv_count,
        error,
    }
}

/// A causa de uma versão não interpretada, com o diagnóstico de endianness
/// quando ele se aplica.
///
/// O teste `version & 0x0000FFFF == 0` é o do leitor de referência: um arquivo
/// de versão 3 escrito em endianness oposta lê a versão como `0x03000000`.
fn unsupported_version_cause(version: u32) -> String {
    if version != 0 && version & 0x0000_FFFF == 0 {
        return format!(
            "versão {version} (0x{version:08X}) inesperada: a metade alta está preenchida, o que é o \
             padrão de um arquivo de endianness oposta — a versão 3 escrita ao contrário lê como 0x03000000"
        );
    }
    format!(
        "versão {version} não interpretada: esta casca lê as versões {SUPPORTED_VERSIONS:?}, e os \
         contadores só são lidos nelas"
    )
}

/// O digest SHA-256 dos bytes do modelo, em hex minúsculo.
///
/// É a metade de [`verify_model_digest`] que **expõe** o digest, para quem
/// precisa do valor antes de ter um esperado com que comparar (construir um
/// manifesto, publicar o modelo). A conferência contra um esperado é
/// [`verify_model_digest`], que reusa [`crate::facade::verify_sha256`].
pub fn model_digest(model: &[u8]) -> String {
    sha256_hex(model)
}

/// Confere que `SHA-256(model)` é igual a `expected_hex`, e reporta o
/// cabeçalho do modelo junto.
///
/// A conferência é delegada a [`crate::facade::verify_sha256`] — não há uma
/// segunda comparação de digest escrita aqui. Como naquela função, um
/// `expected_hex` malformado produz `sha256.ok: false` com a causa, e o digest
/// calculado é reportado de qualquer forma.
pub fn verify_model_digest(model: &[u8], expected_hex: &str) -> GgufModelReport {
    let header = parse_header(model);
    let sha256 = verify_sha256(model, expected_hex);
    GgufModelReport {
        ok: header.ok && sha256.ok,
        header,
        sha256,
    }
}

/// Verifica se `attestation_json` é uma atestação **deste** modelo, e delega o
/// pipeline inteiro a [`crate::facade::verify_attestation`].
///
/// # O que a ligação é, exatamente
///
/// O *subject* que o signatário e as testemunhas assinam é
/// `"arkhe-attestation/v1" ∥ SHA-256(payload) ∥ raiz ∥ leaf_index ∥ tree_size`
/// (ver `attestation.rs` do core): ele amarra o **digest do payload**, não o
/// payload. Então a ligação entre o modelo e a atestação se faz em dois passos,
/// e os dois são reportados:
///
/// 1. [`Self::link`] — `SHA-256(model)` contra o `payload_sha256_hex` que a
///    atestação declara. Reusa [`crate::facade::verify_sha256`].
/// 2. [`Self::payload_is_model`] — os bytes do `payload_b64` decodificado são
///    **os mesmos** que os do modelo. É a checagem que não depende de
///    resistência a colisão de hash.
///
/// A conclusão só vale com os dois **e** com o pipeline inteiro passando:
/// quando `ok` é `true`, a etapa `sha256` da atestação garante que o digest
/// declarado é o digest do payload, o passo 1 garante que é o digest do modelo,
/// e o passo 2 garante que os bytes coincidem — logo a assinatura, que cobre o
/// subject, cobre o digest deste modelo, verificada contra o trust root.
///
/// # Onde esta rota **não** serve — e é honesto dizê-lo
///
/// A forma atual da atestação liga um digest de um único jeito: o **payload da
/// atestação sendo o próprio modelo**. Não existe campo para "o digest do
/// artefato" distinto do payload; o `payload_sha256_hex` é sempre o digest do
/// payload.
///
/// Consequência prática: uma entrada de log cujo corpo seja um **manifesto**
/// que *menciona* o digest do modelo (um JSON com `"model_sha256": "..."`) não
/// pode ser verificada por esta função, e ela responde negativo com a causa —
/// é o comportamento correto, não uma limitação escondida. Verificar essa outra
/// forma exigiria um subject novo, com domínio próprio, ao lado do
/// `arkhe-attestation/v1`: isto é, uma **segunda construção de subject**, e ela
/// pertence ao core de `arkhe-verify-wasm`, não a esta casca. Escrevê-la aqui
/// seria duplicar a decisão criptográfica fora do core — exatamente o que a
/// arquitetura "um core, duas cascas" existe para impedir.
///
/// Custo que também vale registrar: o payload viaja em base64 dentro do JSON,
/// então o modelo é carregado (e decodificado) inteiro em memória por esta
/// chamada. Para os GGUF reais deste repositório — 206 KB — é trivial; para um
/// modelo de dezenas de GB, não é uma rota praticável. E não há o que conferir
/// hoje: os digests dos dois GGUF reais não aparecem em nenhum arquivo
/// rastreado do repositório, isto é, **nenhuma atestação existe para eles** —
/// as fixtures dos testes são construídas no próprio teste, e não atestações
/// reais.
pub fn verify_model_attestation(
    model: &[u8],
    attestation_json: &str,
    trust_root: &TrustRoot,
) -> GgufAttestationReport {
    let header = parse_header(model);
    let model_digest_hex = sha256_hex(model);

    // A atestação interpreta pelo **mesmo** tipo do core, e não por um
    // `serde_json::Value` lido a campo: o contrato é o JSON do core, e um
    // segundo parser dele seria uma segunda chance de divergir.
    let parsed: Option<Attestation> = serde_json::from_str(attestation_json).ok();

    let declared_payload_digest_hex = parsed
        .as_ref()
        .map(|attestation| attestation.payload_sha256_hex.clone());

    // Passo 1: o digest do modelo contra o que a atestação declara.
    let link = declared_payload_digest_hex
        .as_deref()
        .map(|declared| verify_sha256(model, declared));

    // Passo 2: identidade dos bytes — sem hash, sem colisão.
    let payload_is_model = parsed
        .as_ref()
        .and_then(|attestation| decode_base64(&attestation.payload_b64).ok())
        .is_some_and(|payload| payload == model);

    // O pipeline inteiro, do core, pela fachada.
    let attestation = verify_attestation(attestation_json, trust_root);

    let link_ok = link.as_ref().is_some_and(|link| link.ok);
    let ok = header.ok && link_ok && payload_is_model && attestation.ok;

    let error = if ok {
        None
    } else if !header.ok {
        header.error.clone().or_else(|| Some("cabeçalho GGUF inválido".to_string()))
    } else if declared_payload_digest_hex.is_none() {
        // A atestação não interpretou, então não há digest a conferir: a causa
        // útil é a do pipeline (JSON inválido, campo faltando), não uma
        // "divergência de digest" que nunca foi medida.
        attestation.error.clone().or_else(|| {
            Some("atestação não interpretou: nenhum digest declarado".to_string())
        })
    } else if !link_ok {
        Some(format!(
            "link: o modelo não tem o digest que a atestação declara ({}); calculado: {}",
            declared_payload_digest_hex.as_deref().unwrap_or("<atestação não interpretou>"),
            model_digest_hex
        ))
    } else if !payload_is_model {
        Some(
            "link: o digest declarado confere, mas os bytes do payload da atestação não são os do \
             modelo — a atestação não atesta estes bytes"
                .to_string(),
        )
    } else {
        attestation
            .error
            .clone()
            .or_else(|| Some("atestação: pipeline recusado sem causa reportada".to_string()))
    };

    GgufAttestationReport {
        ok,
        header,
        model_digest_hex,
        declared_payload_digest_hex,
        link,
        payload_is_model,
        attestation,
        error,
    }
}

/// O `u32` little-endian em `offset`, ou `None` se os 4 bytes não estiverem lá.
fn read_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    let field: [u8; 4] = bytes.get(offset..offset.checked_add(4)?)?.try_into().ok()?;
    Some(u32::from_le_bytes(field))
}

/// O `i64` little-endian em `offset`, ou `None` se os 8 bytes não estiverem lá.
fn read_i64(bytes: &[u8], offset: usize) -> Option<i64> {
    let field: [u8; 8] = bytes.get(offset..offset.checked_add(8)?)?.try_into().ok()?;
    Some(i64::from_le_bytes(field))
}

#[cfg(test)]
mod tests {
    use super::*;
    use arkhe_verify_wasm::{merkle_root_from_leaves, sha256_bytes};
    use ct_merkle::mem_backed_tree::MemoryBackedTree;
    use ed25519_dalek::{Signer, SigningKey};
    use sha2::Sha256;

    /// Monta um cabeçalho de 24 bytes à mão: nenhuma fixture externa entra nos
    /// testes, e cada campo é posicionado byte a byte.
    fn header(version: u32, tensor_count: i64, kv_count: i64) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(HEADER_LEN);
        bytes.extend_from_slice(&GGUF_MAGIC);
        bytes.extend_from_slice(&version.to_le_bytes());
        bytes.extend_from_slice(&tensor_count.to_le_bytes());
        bytes.extend_from_slice(&kv_count.to_le_bytes());
        bytes
    }

    /// Um modelo sintético: cabeçalho válido mais um corpo arbitrário, para que
    /// o digest cubra mais do que o cabeçalho.
    fn model() -> Vec<u8> {
        let mut bytes = header(3, 2, 5);
        bytes.extend_from_slice(&[0x42; 64]);
        bytes
    }

    // --- cabeçalho: caminho válido ----------------------------------------

    #[test]
    fn a_synthetic_header_is_read_field_by_field() {
        let report = parse_header(&header(3, 7, 18));

        assert!(report.ok, "erro: {:?}", report.error);
        assert!(report.magic_ok);
        assert_eq!(report.available_bytes, HEADER_LEN);
        assert_eq!(report.version, Some(3));
        assert_eq!(report.tensor_count, Some(7));
        assert_eq!(report.metadata_kv_count, Some(18));
        assert!(report.error.is_none());
    }

    #[test]
    fn zero_counts_are_valid_and_not_a_truncation() {
        // Distinguir "contador zero" de "contador ausente" é o que o `Option`
        // faz: um arquivo só de cabeçalho é válido.
        let report = parse_header(&header(3, 0, 0));
        assert!(report.ok);
        assert_eq!(report.tensor_count, Some(0));
        assert_eq!(report.metadata_kv_count, Some(0));
    }

    #[test]
    fn the_real_16_byte_shape_is_a_truncated_header_with_a_valid_magic() {
        // A forma exata do `arkhe.gguf` real (16 bytes): magic, versão 3 e o
        // contador de tensores presente; falta o de pares chave-valor.
        let mut bytes = header(3, 0, 0);
        bytes.truncate(16);
        let report = parse_header(&bytes);

        assert!(!report.ok);
        assert!(report.magic_ok);
        assert_eq!(report.available_bytes, 16);
        assert_eq!(report.version, Some(3));
        assert_eq!(report.tensor_count, Some(0), "os bytes presentes são lidos");
        assert_eq!(report.metadata_kv_count, None, "e o ausente é `None`");
        let cause = report.error.expect("causa");
        assert!(cause.contains("truncado"), "causa: {cause}");
        assert!(cause.contains("24"), "a causa diz o tamanho esperado: {cause}");
    }

    #[test]
    fn a_header_shorter_than_the_magic_is_truncated_before_anything_else() {
        let report = parse_header(b"GGU");
        assert!(!report.ok);
        assert!(!report.magic_ok);
        assert_eq!(report.available_bytes, 3);
        assert_eq!(report.version, None);
        assert!(report.error.expect("causa").contains("truncado"));
    }

    #[test]
    fn a_header_with_magic_but_without_the_version_is_truncated() {
        let report = parse_header(b"GGUF\x03\x00");
        assert!(!report.ok);
        assert!(report.magic_ok, "o magic está lá");
        assert_eq!(report.version, None, "a versão não foi lida por inteiro");
        assert!(report.error.expect("causa").contains("truncado"));
    }

    // --- cabeçalho: magic ------------------------------------------------

    #[test]
    fn an_invalid_magic_is_reported_with_the_bytes_found() {
        let report = parse_header(b"zkAG\x03\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00");

        assert!(!report.ok);
        assert!(!report.magic_ok);
        assert_eq!(report.version, None, "sem magic não há versão a declarar");
        assert_eq!(report.tensor_count, None);
        let cause = report.error.expect("causa");
        assert!(cause.contains("magic"), "causa: {cause}");
        assert!(cause.contains("zkAG"), "a causa mostra o que encontrou: {cause}");
    }

    // --- cabeçalho: versão ------------------------------------------------

    #[test]
    fn an_unsupported_version_is_reported_without_claiming_the_counts() {
        for version in [1u32, 4, 99] {
            let report = parse_header(&header(version, 7, 18));
            assert!(!report.ok, "versão {version}");
            assert!(report.magic_ok);
            assert_eq!(report.version, Some(version));
            assert_eq!(
                (report.tensor_count, report.metadata_kv_count),
                (None, None),
                "os contadores não são lidos onde o layout não é o desta casca"
            );
            let cause = report.error.expect("causa");
            assert!(cause.contains("versão"), "causa: {cause}");
        }
    }

    #[test]
    fn version_0_is_reported_as_unsupported_rather_than_as_empty() {
        let report = parse_header(&header(0, 0, 0));
        assert!(!report.ok);
        assert_eq!(report.version, Some(0));
        assert!(report.error.expect("causa").contains("versão"));
    }

    #[test]
    fn a_byte_swapped_version_is_diagnosed_as_endianness() {
        let report = parse_header(&header(0x0300_0000, 7, 18));
        assert!(!report.ok);
        assert_eq!(report.version, Some(0x0300_0000));
        let cause = report.error.expect("causa");
        assert!(cause.contains("endianness"), "causa: {cause}");
        assert!(cause.contains("0x03000000"), "causa: {cause}");
    }

    #[test]
    fn a_negative_tensor_count_is_rejected_rather_than_widened() {
        let report = parse_header(&header(3, -1, 18));
        assert!(!report.ok);
        assert_eq!(report.tensor_count, None, "não é reinterpretado como u64 enorme");
        assert_eq!(report.metadata_kv_count, Some(18));
        assert!(report.error.expect("causa").contains("negativo"));
    }

    #[test]
    fn a_negative_kv_count_is_rejected_rather_than_widened() {
        let report = parse_header(&header(3, 7, -2));
        assert!(!report.ok);
        assert_eq!(report.tensor_count, Some(7));
        assert_eq!(report.metadata_kv_count, None);
        assert!(report.error.expect("causa").contains("negativo"));
    }

    #[test]
    fn a_large_but_plausible_count_is_reported_as_is() {
        // Um `u64`/`i64` grande é reportado, não "corrigido": o verificador
        // mostra o que está no arquivo. `u64::MAX as i64` é -1 e é recusado
        // (teste acima); `i64::MAX` é aceito e aparece por inteiro.
        let report = parse_header(&header(3, i64::MAX, 0));
        assert!(report.ok);
        assert_eq!(report.tensor_count, Some(i64::MAX as u64));
    }

    // --- digest -----------------------------------------------------------

    #[test]
    fn the_digest_of_a_known_input_matches_the_published_vector() {
        assert_eq!(
            model_digest(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn the_digest_matches_the_core_primitive_on_a_synthetic_model() {
        assert_eq!(model_digest(&model()), encode_hex(&sha256_bytes(&model())));
    }

    #[test]
    fn a_model_with_the_right_digest_verifies() {
        let bytes = model();
        let report = verify_model_digest(&bytes, &model_digest(&bytes));

        assert!(report.ok, "erro: {:?}", report.sha256.error);
        assert!(report.header.ok);
        assert!(report.sha256.ok);
        assert!(report.sha256.error.is_none());
        assert_eq!(report.sha256.expected_hex, model_digest(&bytes));
        assert_eq!(
            report.sha256.expected_hex, report.sha256.computed_hex,
            "o esperado e o calculado são o mesmo valor"
        );
    }

    #[test]
    fn a_wrong_digest_is_rejected_but_the_computed_one_is_still_reported() {
        let report = verify_model_digest(&model(), &"00".repeat(32));

        assert!(!report.ok);
        assert!(!report.sha256.ok);
        assert_eq!(
            report.sha256.computed_hex,
            model_digest(&model()),
            "o digest calculado é a informação que sobrevive à recusa"
        );
        assert!(report.sha256.error.expect("causa").contains("não confere"));
    }

    #[test]
    fn a_malformed_expected_digest_is_a_negative_report_not_an_error() {
        let report = verify_model_digest(&model(), "não é hex");

        assert!(!report.ok);
        assert!(!report.sha256.ok);
        assert_eq!(report.sha256.computed_hex, model_digest(&model()));
        assert!(report.sha256.error.expect("causa").contains("hex"));
    }

    #[test]
    fn a_truncated_model_can_have_a_correct_digest_and_still_be_rejected() {
        // A forma do `arkhe.gguf` real: o digest confere e o cabeçalho não.
        // `ok` é `false` e o relatório diz qual dos dois falhou.
        let bytes = {
            let mut bytes = header(3, 0, 0);
            bytes.truncate(16);
            bytes
        };
        let report = verify_model_digest(&bytes, &model_digest(&bytes));

        assert!(!report.ok);
        assert!(report.sha256.ok, "o digest destes bytes confere");
        assert!(!report.header.ok, "o cabeçalho é que não fecha");
        assert!(report.header.error.is_some());
    }

    #[test]
    fn the_model_report_serializes_to_the_documented_shape() {
        let bytes = model();
        let report = verify_model_digest(&bytes, &model_digest(&bytes));
        let value: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&report).expect("serializa"))
                .expect("parse");

        assert_eq!(value["ok"], true);
        assert_eq!(value["header"]["ok"], true);
        assert_eq!(value["header"]["version"], 3);
        assert_eq!(value["header"]["tensor_count"], 2);
        assert_eq!(value["header"]["metadata_kv_count"], 5);
        assert_eq!(value["sha256"]["computed_hex"], model_digest(&bytes));
    }

    // --- atestação: fixtures ----------------------------------------------

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

    /// Uma atestação **válida de ponta a ponta** cujo payload é `payload`:
    /// 3 folhas, a folha 1 atestada, assinada pelo seed 1, com 2 witnesses
    /// (seeds 2 e 3).
    fn attestation_over(payload: &[u8]) -> (String, TrustRoot) {
        let leaves: Vec<Vec<u8>> = vec![
            b"outro-artifact".to_vec(),
            payload.to_vec(),
            b"terceiro-artifact".to_vec(),
        ];
        let leaf_index = 1u64;

        let mut tree = MemoryBackedTree::<Sha256, Vec<u8>>::new();
        for leaf in &leaves {
            tree.push(leaf.clone());
        }
        let tree_size = tree.len();
        let root = merkle_root_from_leaves(&leaves);
        let proof = tree.prove_inclusion(leaf_index as usize).as_bytes().to_vec();

        let digest = sha256_bytes(payload);
        let subject = arkhe_verify_wasm::attestation::attestation_subject(
            &digest, &root, leaf_index, tree_size,
        );

        let signature = signing_key(1).sign(&subject).to_bytes();
        let witnesses: Vec<serde_json::Value> = [2u8, 3]
            .iter()
            .map(|&seed| {
                serde_json::json!({
                    "public_key_hex": public_hex(seed),
                    "signature_hex": encode_hex(&signing_key(seed).sign(&subject).to_bytes()),
                })
            })
            .collect();

        let json = serde_json::json!({
            "payload_b64": base64_encode(payload),
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

        let trust_root =
            TrustRoot::parse(&serde_json::json!([public_hex(1), public_hex(2), public_hex(3)]).to_string())
                .expect("parse");
        (json, trust_root)
    }

    fn with_field(json: &str, field: &str, value: serde_json::Value) -> String {
        let mut root: serde_json::Value = serde_json::from_str(json).expect("parse");
        root[field] = value;
        root.to_string()
    }

    // --- atestação: caminho válido ----------------------------------------

    #[test]
    fn an_attestation_whose_payload_is_the_model_verifies_end_to_end() {
        let bytes = model();
        let (json, trust_root) = attestation_over(&bytes);
        let report = verify_model_attestation(&bytes, &json, &trust_root);

        assert!(report.ok, "erro: {:?}", report.error);
        assert!(report.header.ok);
        assert!(report.link.as_ref().expect("link").ok);
        assert!(report.payload_is_model);
        assert_eq!(
            report.attestation,
            AttestationReport {
                ok: true,
                sha256: true,
                signature: true,
                inclusion: true,
                quorum: true,
                error: None,
            }
        );
        assert!(report.error.is_none());
    }

    #[test]
    fn the_linked_digest_is_the_digest_of_the_model() {
        let bytes = model();
        let (json, trust_root) = attestation_over(&bytes);
        let report = verify_model_attestation(&bytes, &json, &trust_root);

        assert_eq!(report.model_digest_hex, model_digest(&bytes));
        assert_eq!(
            report.declared_payload_digest_hex.as_deref(),
            Some(report.model_digest_hex.as_str())
        );
        assert_eq!(
            report.link.as_ref().expect("link").computed_hex,
            report.model_digest_hex,
            "o digest do link e o do relatório vêm do mesmo cálculo"
        );
    }

    #[test]
    fn the_attestation_report_serializes_to_the_documented_shape() {
        let bytes = model();
        let (json, trust_root) = attestation_over(&bytes);
        let report = verify_model_attestation(&bytes, &json, &trust_root);
        let value: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&report).expect("serializa"))
                .expect("parse");

        assert_eq!(value["ok"], true);
        assert_eq!(value["payload_is_model"], true);
        assert_eq!(value["link"]["ok"], true);
        assert_eq!(value["model_digest_hex"], model_digest(&bytes));
        assert_eq!(value["attestation"]["ok"], true);
        assert!(value["error"].is_null());
    }

    // --- atestação: recusas ----------------------------------------------

    #[test]
    fn an_attestation_over_a_different_model_does_not_verify_this_one() {
        let (json, trust_root) = attestation_over(b"o modelo de verdade");
        let report = verify_model_attestation(&model(), &json, &trust_root);

        assert!(!report.ok);
        assert!(!report.link.as_ref().expect("link").ok, "o digest não bate");
        assert!(!report.payload_is_model);
        assert!(
            report.attestation.ok,
            "a atestação é perfeitamente válida sobre o outro artefato"
        );
        assert!(report.error.expect("causa").contains("link"));
    }

    #[test]
    fn a_model_with_the_declared_digest_but_a_different_body_is_rejected_by_the_byte_check() {
        // Aqui o passo 1 é forçado a passar: o digest declarado é trocado pelo
        // do modelo, e a assinatura deixa de fechar — mas o que este teste
        // isola é o passo 2, então a atestação é refeita sobre ela mesma.
        // O payload continua sendo o outro artefato, logo a identidade de
        // bytes falha e o relatório diz isso.
        let bytes = model();
        let (json, trust_root) = attestation_over(b"outro payload qualquer");
        let json = with_field(
            &json,
            "payload_sha256_hex",
            serde_json::Value::String(model_digest(&bytes)),
        );
        let report = verify_model_attestation(&bytes, &json, &trust_root);

        assert!(!report.ok);
        assert!(report.link.as_ref().expect("link").ok, "o digest declarado confere");
        assert!(!report.payload_is_model, "os bytes do payload não são os do modelo");
        assert!(!report.attestation.sha256, "e a etapa 1 da atestação recusa a troca");
        assert!(report.error.expect("causa").contains("payload"));
    }

    #[test]
    fn an_empty_trust_root_leaves_the_math_intact_and_the_verdict_negative() {
        let bytes = model();
        let (json, _trust_root) = attestation_over(&bytes);
        let report = verify_model_attestation(&bytes, &json, &TrustRoot::default());

        assert!(!report.ok);
        assert!(report.header.ok);
        assert!(report.link.as_ref().expect("link").ok);
        assert!(report.payload_is_model);
        assert!(report.attestation.sha256, "a matemática do log continua conferível");
        assert!(report.attestation.inclusion);
        assert!(!report.attestation.signature);
        assert!(!report.attestation.quorum);
        assert!(report.error.is_some());
    }

    #[test]
    fn a_truncated_model_is_rejected_by_the_attestation_route_too() {
        let bytes = {
            let mut bytes = model();
            bytes.truncate(16);
            bytes
        };
        let (json, trust_root) = attestation_over(&bytes);
        let report = verify_model_attestation(&bytes, &json, &trust_root);

        assert!(!report.ok);
        assert!(!report.header.ok);
        assert!(report.link.as_ref().expect("link").ok, "o digest destes bytes confere");
        assert!(report.payload_is_model);
        assert!(report.attestation.ok);
        assert!(report.error.expect("causa").contains("truncado"));
    }

    // --- atestação: entradas malformadas ---------------------------------

    #[test]
    fn malformed_json_is_reported_rather_than_panicking() {
        let report = verify_model_attestation(
            &model(),
            "isto não é json",
            &TrustRoot::default(),
        );

        assert!(!report.ok);
        assert!(report.header.ok, "o cabeçalho do modelo não depende do JSON");
        assert_eq!(report.declared_payload_digest_hex, None);
        assert_eq!(report.link, None, "sem atestação não há digest a conferir");
        assert!(!report.payload_is_model);
        assert!(!report.attestation.ok);
        assert!(report.error.expect("causa").contains("JSON inválido"));
    }

    #[test]
    fn a_missing_declared_digest_is_reported_as_invalid_json() {
        let bytes = model();
        let (json, trust_root) = attestation_over(&bytes);
        let mut root: serde_json::Value = serde_json::from_str(&json).expect("parse");
        root.as_object_mut().expect("objeto").remove("payload_sha256_hex");

        let report = verify_model_attestation(&bytes, &root.to_string(), &trust_root);
        assert!(!report.ok);
        assert_eq!(report.link, None);
        assert!(report.error.expect("causa").contains("JSON inválido"));
    }

    #[test]
    fn an_invalid_payload_base64_is_reported_as_a_byte_mismatch_not_a_panic() {
        let bytes = model();
        let (json, trust_root) = attestation_over(&bytes);
        let json = with_field(
            &json,
            "payload_b64",
            serde_json::Value::String("!!! não é base64 !!!".to_string()),
        );

        let report = verify_model_attestation(&bytes, &json, &trust_root);
        assert!(!report.ok);
        assert!(!report.payload_is_model, "sem decodificar, não há identidade");
        assert!(!report.attestation.ok);
        assert!(report.error.is_some());
    }

    // --- a ausência de I/O é uma propriedade da API -----------------------

    #[test]
    fn the_api_is_over_bytes_and_a_truncated_buffer_is_not_an_error() {
        // Nenhuma função deste módulo recebe um caminho, então o "arquivo
        // truncado" é só um `&[u8]` curto — e o resultado é um relatório, não
        // um `Err`. Este teste existe para fixar essa forma: se alguém trocar
        // a assinatura por um `Path`, ele para de compilar.
        let all_ff = [0xFF; HEADER_LEN];
        for bytes in [&b""[..], &b"G"[..], &b"GGUF"[..], &all_ff[..]] {
            let report: GgufHeaderReport = parse_header(bytes);
            assert!(!report.ok);
            assert!(report.error.is_some());
            let model_report: GgufModelReport = verify_model_digest(bytes, &model_digest(bytes));
            assert!(model_report.sha256.ok, "o digest de qualquer byte existe");
        }
    }
}
