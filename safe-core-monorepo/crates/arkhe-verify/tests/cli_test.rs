//! Testes de integração do **binário** `arkhe-verify`.
//!
//! Cada teste corre o executável de verdade, com `std::process::Command`, e
//! afirma sobre o **exit code** e sobre o que sai em `stdout`/`stderr` — porque
//! o contrato deste CLI é exatamente isso: quais gates passaram e com que exit
//! code ele responde.
//!
//! # Porque `std::process::Command` e não `assert_cmd`
//!
//! `assert_cmd` e `predicates` foram considerados e **não** instalados. Duas
//! razões, nesta ordem:
//!
//! 1. O que este ficheiro precisa do Cargo já vem dele:
//!    `env!("CARGO_BIN_EXE_arkhe-verify")` é o caminho do binário compilado, que
//!    é o único serviço do `assert_cmd` que os testes usam. O resto — correr,
//!    capturar, ler o exit code — é `std::process::Command`.
//! 2. `assert_cmd` arrastaria `predicates`, `bstr`, `wait-timeout` e
//!    `doc-comment` para dentro do `Cargo.lock` de um workspace que outros
//!    crates (`arkhe-orcid`, `arkhe-mcp`, …) também compilam. Num workspace com
//!    pins deliberados — `ct-merkle` 0.3.0, `sha2` 0.11, `reqwest` 0.13.4 — é
//!    na resolução compartilhada que uma versão pode ser movida sem ninguém
//!    pedir, e os testes de um CLI não valem esse risco.
//!
//! Efeito colateral a registrar: `cargo test -p arkhe-verify` continua a
//! resolver e a compilar **só** com o que já estava no `Cargo.lock`.
//!
//! A fachada do `clap` (uso incorrecto → exit 2) também não é testada aqui: ela
//! é do `clap`, não deste crate.
//!
//! # O que estes testes **não** provam
//!
//! - Nenhum ficheiro GGUF real é lido além da fixture de 24 bytes: um modelo de
//!   verdade não é verificado aqui.
//! - A atestação dos testes é montada **no próprio teste**, com a API pública da
//!   casca para o digest, a raiz e o *subject* — não é uma atestação produzida
//!   por um log nem assinada por uma chave que exista fora deste ficheiro. O que
//!   ela prova é o caminho `--manifest`/`--trust-root`, não que exista uma
//!   atestação real para a fixture.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

use arkhe_verify::encoding::encode_hex;
use arkhe_verify::facade::attestation_subject;
use arkhe_verify::gguf::model_digest;
use arkhe_verify::merkle_root_from_leaves;
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use ct_merkle::mem_backed_tree::MemoryBackedTree;
use ed25519_dalek::{Signer, SigningKey};
use sha2::Sha256;

/// O digest SHA-256 da fixture, confirmado independentemente por `sha256sum`
/// sobre `fixtures/arkhe.gguf` (não só por este crate).
const FIXTURE_SHA256: &str = "a4e5e156ddec27e286f75328784d7106b60a4eb1d246e950a001a3f944fbda99";

/// O caminho da fixture, absoluto — a partir do diretório do crate, e não do
/// diretório de trabalho, que Cargo não promete que seja o mesmo.
fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/arkhe.gguf")
}

/// O caminho do binário compilado, publicado pelo Cargo para os testes de
/// integração do pacote que o declara.
const BIN: &str = env!("CARGO_BIN_EXE_arkhe-verify");

fn run(args: &[&str]) -> Output {
    Command::new(BIN)
        .args(args)
        .output()
        .unwrap_or_else(|err| panic!("não foi possível executar `{BIN}`: {err}"))
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// O exit code, exigindo que o processo tenha terminado por exit code (e não
/// por sinal) — sem isso, um `code()` devolvido como `None` no Windows passaria
/// como sucesso num `assert_eq!` mal escrito.
fn code(output: &Output) -> i32 {
    output
        .status
        .code()
        .unwrap_or_else(|| panic!("o processo não terminou por exit code: {:?}", output.status))
}

/// Um diretório temporário próprio do teste, removido no `Drop`.
///
/// Sem a crate `tempfile` (ver a nota no topo): o nome carrega um contador
/// atómico além do pid, porque os testes correm em threads do **mesmo**
/// processo e um pid só não distingue dois deles.
struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "arkhe-verify-cli-{tag}-{}-{unique}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("criar o diretório temporário");
        Self(path)
    }

    fn write(&self, name: &str, contents: &str) -> PathBuf {
        let path = self.0.join(name);
        std::fs::write(&path, contents).expect("escrever o ficheiro temporário");
        path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn signing_key(seed: u8) -> SigningKey {
    SigningKey::from_bytes(&[seed; 32])
}

fn public_hex(seed: u8) -> String {
    encode_hex(signing_key(seed).verifying_key().as_bytes())
}

fn base64_encode(bytes: &[u8]) -> String {
    BASE64.encode(bytes)
}

/// Troca um campo de um JSON, preservando o resto.
fn with_field(json: &str, field: &str, value: serde_json::Value) -> String {
    let mut root: serde_json::Value =
        serde_json::from_str(json).expect("o JSON do teste tem de parsear");
    root[field] = value;
    root.to_string()
}

/// Monta uma atestação **válida de ponta a ponta** sobre `model`, e o trust root
/// que confia nas três chaves dela — 3 folhas, a folha 1 atestada, assinada pelo
/// seed 1, com 2 witnesses (seeds 2 e 3) e limiar 2.
///
/// O digest, a raiz e o *subject* vêm da API pública da casca
/// (`gguf::model_digest`, `merkle_root_from_leaves`, `facade::attestation_subject`)
/// e não de uma segunda implementação escrita no teste: o *subject* é o que o
/// core assina, e calculá-lo à mão aqui seria a chance de o teste e o core
/// concordarem por estarem ambos errados.
fn attestation_over(model: &[u8]) -> (String, String) {
    let payload = model.to_vec();
    let leaves: Vec<Vec<u8>> = vec![
        b"outro-artifact".to_vec(),
        payload.clone(),
        b"terceiro-artifact".to_vec(),
    ];
    let leaf_index = 1u64;

    let mut tree = MemoryBackedTree::<Sha256, Vec<u8>>::new();
    for leaf in &leaves {
        tree.push(leaf.clone());
    }
    let tree_size = tree.len();
    let root_hex = encode_hex(&merkle_root_from_leaves(&leaves));
    let proof_hex = encode_hex(&tree.prove_inclusion(leaf_index as usize).as_bytes());

    // O digest que a atestação declara: a API real, sobre os bytes do payload.
    let digest_hex = model_digest(&payload);

    // O esqueleto tem tudo menos a assinatura e as testemunhas — o *subject* só
    // depende do payload, da raiz e da posição, então ele já é calculável aqui.
    let skeleton = serde_json::json!({
        "payload_b64": base64_encode(&payload),
        "payload_sha256_hex": digest_hex,
        "merkle_leaf_index": leaf_index,
        "merkle_tree_size": tree_size,
        "merkle_proof_hex": proof_hex,
        "merkle_root_hex": root_hex,
        "signer_public_key_hex": public_hex(1),
        "signature_hex": "",
        "witnesses": [],
        "quorum_threshold": 2,
    })
    .to_string();

    let subject = attestation_subject(&skeleton).expect("o subject da atestação");

    let signature_hex = encode_hex(&signing_key(1).sign(&subject).to_bytes());
    let witnesses: Vec<serde_json::Value> = [2u8, 3]
        .iter()
        .map(|&seed| {
            serde_json::json!({
                "public_key_hex": public_hex(seed),
                "signature_hex": encode_hex(&signing_key(seed).sign(&subject).to_bytes()),
            })
        })
        .collect();

    let attestation = with_field(
        &with_field(
            &skeleton,
            "signature_hex",
            serde_json::Value::String(signature_hex),
        ),
        "witnesses",
        serde_json::Value::Array(witnesses),
    );

    let trust_root = serde_json::json!([public_hex(1), public_hex(2), public_hex(3)]).to_string();

    (attestation, trust_root)
}

// --- 1: a fixture, sem manifesto ------------------------------------------

#[test]
fn the_fixture_passes_and_the_hash_gate_is_reported() {
    let out = run(&[fixture().to_str().expect("caminho UTF-8")]);

    assert_eq!(code(&out), 0, "stdout: {}", stdout(&out));

    let text = stdout(&out);
    assert!(
        text.contains(FIXTURE_SHA256),
        "o digest SHA-256 da fixture tem de sair no relatório: {text}"
    );
    assert!(
        text.contains("digest"),
        "o relatório tem de mencionar o gate do digest: {text}"
    );
    // O cabeçalho é o único gate que corre sem manifesto, e ele passa: magic
    // `GGUF`, versão 3, 0 tensores, 0 pares chave-valor.
    assert!(
        text.contains("[ ok ] cabeçalho"),
        "o cabeçalho da fixture é válido e o gate tem de passar: {text}"
    );
    assert!(
        text.contains("1 de 6 gates passaram"),
        "o resumo tem de contar 1 gate passado e os outros não avaliados: {text}"
    );
}

// --- 2: `--strict` falha enquanto houver gates não avaliados --------------

#[test]
fn strict_fails_while_gates_are_not_evaluated() {
    let path = fixture();
    let path = path.to_str().expect("caminho UTF-8");

    let lax = run(&[path]);
    let strict = run(&[path, "--strict"]);

    assert_eq!(
        code(&lax),
        0,
        "sem --strict, os gates não avaliados são tolerados: {}",
        stdout(&lax)
    );
    assert_eq!(
        code(&strict),
        1,
        "--strict exige todos os gates: {}",
        stdout(&strict)
    );

    let text = stdout(&strict);
    assert!(
        text.contains("strict:"),
        "a saída tem de dizer que o --strict é a causa do exit 1: {text}"
    );
    assert!(
        text.contains("não avaliado"),
        "a saída tem de mostrar a razão dos gates não avaliados: {text}"
    );
    // E o gate que nunca pode passar tem de ser nomeado como tal, senão o
    // utilizador procura a causa no sítio errado.
    assert!(
        text.contains("nunca passa"),
        "o relatório tem de dizer que o gate da consistência nunca passa: {text}"
    );
}

// --- 3: `--verbose` mostra também os gates não avaliados ------------------

#[test]
fn verbose_shows_the_reason_of_the_not_evaluated_gates() {
    let path = fixture();
    let path = path.to_str().expect("caminho UTF-8");

    let plain = stdout(&run(&[path]));
    let verbose = stdout(&run(&[path, "--verbose"]));

    // Os dois modos imprimem os **mesmos** gates: o que o `--verbose` muda é o
    // detalhe.
    for gate in ["cabeçalho", "digest", "consistência"] {
        assert!(
            plain.contains(gate),
            "o modo compacto lista `{gate}`: {plain}"
        );
        assert!(
            verbose.contains(gate),
            "o modo verboso lista `{gate}`: {verbose}"
        );
    }

    // A razão completa dos não avaliados só sai no modo verboso — e tem de
    // nomear o que falta, não só dizer "não avaliado".
    assert!(
        verbose.contains("motivo:"),
        "o modo verboso imprime o motivo completo: {verbose}"
    );
    assert!(
        !plain.contains("motivo:"),
        "o modo compacto não imprime o motivo completo: {plain}"
    );
    assert!(
        verbose.contains("consistency_proof"),
        "o motivo do gate da consistência nomeia a função do core que só busca a prova: {verbose}"
    );
    assert!(
        !plain.contains("consistency_proof"),
        "esse detalhe é do modo verboso: {plain}"
    );

    // O detalhe do cabeçalho passa a aparecer.
    assert!(
        verbose.contains("parse_header"),
        "o detalhe cita a API real usada por cada gate: {verbose}"
    );

    // `--verbose` sozinho não muda o veredito.
    assert_eq!(code(&run(&[path, "--verbose"])), 0);
}

// --- 4: ficheiro ausente ---------------------------------------------------

#[test]
fn a_missing_file_fails_with_a_clear_message() {
    let out = run(&["nao-existe.gguf"]);

    assert_ne!(
        code(&out),
        0,
        "um ficheiro ausente não pode sair com 0: {}",
        stdout(&out)
    );
    let err = stderr(&out);
    assert!(
        err.contains("nao-existe.gguf"),
        "a mensagem tem de nomear o caminho: {err}"
    );
    assert!(
        err.contains("não foi possível ler"),
        "a mensagem tem de dizer o que falhou: {err}"
    );
    assert!(
        stdout(&out).is_empty(),
        "um erro de entrada não produz relatório de gates: {}",
        stdout(&out)
    );
}

// --- 5: manifesto válido e trust root -------------------------------------

#[test]
fn a_valid_manifest_evaluates_the_attestation_gates() {
    let dir = TempDir::new("valid");
    let model = std::fs::read(fixture()).expect("ler a fixture");
    let (manifest, trust_root) = attestation_over(&model);
    let manifest = dir.write("manifest.json", &manifest);
    let trust_root = dir.write("trust-root.json", &trust_root);
    let path = fixture();

    let args = [
        path.to_str().expect("caminho UTF-8"),
        "--manifest",
        manifest.to_str().expect("caminho UTF-8"),
        "--trust-root",
        trust_root.to_str().expect("caminho UTF-8"),
    ];

    let out = run(&args);
    let text = stdout(&out);

    assert_eq!(code(&out), 0, "stdout: {text}");
    assert_eq!(
        text.matches("[ ok ]").count(),
        5,
        "com a atestação válida, os cinco gates avaliáveis passam: {text}"
    );
    for gate in ["cabeçalho", "digest", "assinatura", "inclusão", "quórum"] {
        assert!(
            text.contains(&format!("[ ok ] {gate}")),
            "o gate `{gate}` tem de constar como passado: {text}"
        );
    }
    assert!(
        text.contains("5 de 6 gates passaram"),
        "o resumo tem de contar 5 de 6: {text}"
    );
    assert!(
        text.contains("core: ok=true"),
        "o veredito do core sobre a mesma atestação é `ok=true`: {text}"
    );

    // O gate da consistência não passa **mesmo com tudo o resto validado**: é
    // por isso que `--strict` não pode devolver 0 hoje.
    assert_eq!(
        code(&run(&[
            args[0], args[1], args[2], args[3], args[4], "--strict"
        ])),
        1,
        "com tudo o mais válido, o --strict ainda falha no gate não implementado"
    );
}

// --- 6: um manifesto que declara outro digest ------------------------------

#[test]
fn a_manifest_declaring_another_digest_fails_the_digest_gate() {
    let dir = TempDir::new("wrong-digest");
    let model = std::fs::read(fixture()).expect("ler a fixture");
    let (manifest, trust_root) = attestation_over(&model);
    // O digest declarado passa a ser o de outro payload qualquer: o modelo
    // deixa de ter o digest que a atestação declara.
    let manifest = with_field(
        &manifest,
        "payload_sha256_hex",
        serde_json::Value::String("00".repeat(32)),
    );
    let manifest = dir.write("manifest.json", &manifest);
    let trust_root = dir.write("trust-root.json", &trust_root);
    let path = fixture();

    let out = run(&[
        path.to_str().expect("caminho UTF-8"),
        "--manifest",
        manifest.to_str().expect("caminho UTF-8"),
        "--trust-root",
        trust_root.to_str().expect("caminho UTF-8"),
    ]);

    let text = stdout(&out);
    assert_eq!(
        code(&out),
        1,
        "um gate que correu e deu negativo sai com 1 **mesmo sem --strict**: {text}"
    );
    assert!(
        text.contains("[falha] digest"),
        "o gate do digest é o que falha: {text}"
    );
    assert!(
        text.contains(FIXTURE_SHA256),
        "o digest calculado continua a ser reportado: {text}"
    );
}

// --- 7: manifesto ininterpretável e trust root inválido -------------------

#[test]
fn an_uninterpretable_manifest_is_an_input_error() {
    let dir = TempDir::new("bad-manifest");
    let manifest = dir.write("manifest.json", "isto não é uma atestação");
    let path = fixture();

    let out = run(&[
        path.to_str().expect("caminho UTF-8"),
        "--manifest",
        manifest.to_str().expect("caminho UTF-8"),
    ]);

    assert_eq!(
        code(&out),
        2,
        "um manifesto que não interpreta é erro de entrada, não veredito: {}",
        stdout(&out)
    );
    let err = stderr(&out);
    assert!(
        err.contains("não é uma atestação interpretável"),
        "a mensagem tem de dizer o que se esperava do manifesto: {err}"
    );
}

#[test]
fn a_malformed_trust_root_is_an_input_error() {
    let dir = TempDir::new("bad-trust-root");
    let manifest = dir.write("manifest.json", "{}");
    let trust_root = dir.write("trust-root.json", r#"["curta"]"#);
    let path = fixture();

    let out = run(&[
        path.to_str().expect("caminho UTF-8"),
        "--manifest",
        manifest.to_str().expect("caminho UTF-8"),
        "--trust-root",
        trust_root.to_str().expect("caminho UTF-8"),
    ]);

    assert_eq!(
        code(&out),
        2,
        "um trust root malformado é erro de entrada: {}",
        stdout(&out)
    );
    let err = stderr(&out);
    assert!(
        err.contains("trust root inválido"),
        "a mensagem tem de nomear o trust root: {err}"
    );
}
