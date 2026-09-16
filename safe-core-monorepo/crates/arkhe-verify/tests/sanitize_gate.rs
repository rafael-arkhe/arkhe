//! O **Gate 0** pela rota do CLI: a fixture válida passa por ele, e um ficheiro
//! deliberadamente malformado é recusado por ele — antes de qualquer outro
//! gate.
//!
//! # Porque estes testes existem ao lado dos de `sanitize.rs`
//!
//! Os testes de unidade de `sanitize.rs` provam o veredito da função. Estes
//! provam a **ligação**: que o binário invoca o Gate 0, que o veredito dele
//! aparece no relatório, e — o que mais importa — que quando ele recusa, os
//! outros gates **não correm**. É a diferença entre "a função está certa" e "o
//! programa usa a função antes das outras".
//!
//! # Porque o ficheiro malformado é construído aqui e não é uma fixture
//!
//! A primeira versão disto era uma fixture em `fixtures/`, ao lado da
//! `arkhe.gguf`. Ela **não seria commitada**: `.gitignore:48` tem `*.gguf`, e
//! um ficheiro que existe no disco de quem escreve e não existe no repositório
//! faz este teste passar localmente e falhar em qualquer outro sítio — o teste
//! dependeria de um ficheiro invisível ao git. A fixture válida só está
//! rastreada porque entrou antes da regra (ou com `-f`). Em vez de mexer no
//! `.gitignore` (fora do âmbito deste trabalho), o ficheiro é construído em
//! memória aqui, byte a byte, e escrito num diretório temporário em tempo de
//! execução — o mesmo padrão que `tests/cli_test.rs` já usa para os manifestos.
//! Assim o teste depende **só** do que está versionado: 24 bytes que se leem no
//! próprio ficheiro de teste.
//!
//! # O que o ficheiro malformado é
//!
//! Um cabeçalho GGUF v3 válido de 24 bytes cujo contador de pares chave-valor
//! declara **99 999**. O valor está **dentro** de `max_kv_pairs` (100 000),
//! portanto a recusa não vem do limite de contagem e sim do confronto com os
//! bytes reais: 99 999 pares exigem pelo menos 1 299 987 bytes e o ficheiro tem
//! zero depois do cabeçalho. É exatamente a defesa do CVE-2026-5757 e do
//! CVE-2026-7482 — a contagem declarada confrontada com o tamanho real antes de
//! qualquer iteração ou alocação.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

/// O caminho do binário compilado, publicado pelo Cargo para os testes de
/// integração do pacote que o declara.
const BIN: &str = env!("CARGO_BIN_EXE_arkhe-verify");

/// Uma fixture do crate, por nome, com caminho absoluto a partir do diretório do
/// crate — e não do diretório de trabalho, que o Cargo não promete que seja o
/// mesmo.
fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(name)
}

/// O ficheiro malformado: 24 bytes, versão 3, zero tensores, **99 999** pares
/// chave-valor declarados e nenhum trazido.
///
/// Os bytes estão escritos um a um, e não empacotados com um `struct`, para que
/// quem leia o teste veja o ficheiro sem traduzir nada: `GGUF`, a versão 3 em
/// little-endian, oito bytes de zero para o contador de tensores, e o contador
/// de pares — `99_999` é `0x0001_869F`, logo `9f 86 01 00 00 00 00 00`.
fn malformed_bytes() -> Vec<u8> {
    let mut bytes = Vec::with_capacity(24);
    bytes.extend_from_slice(b"GGUF");
    bytes.extend_from_slice(&[3, 0, 0, 0]); // versão 3, u32 LE
    bytes.extend_from_slice(&[0; 8]); // zero tensores, u64 LE
    bytes.extend_from_slice(&99_999u64.to_le_bytes()); // 99 999 pares
    bytes
}

/// Um diretório temporário próprio do teste, removido no `Drop`.
///
/// Sem a crate `tempfile` — ver a nota de `tests/cli_test.rs` sobre a resolução
/// de dependências deste workspace. O nome carrega um contador atómico além do
/// pid, porque os testes correm em threads do mesmo processo.
struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "arkhe-verify-gate-0-{tag}-{}-{unique}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("criar o diretório temporário");
        Self(path)
    }

    fn write_bytes(&self, name: &str, contents: &[u8]) -> PathBuf {
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

/// Corre o binário com os argumentos dados.
fn run(args: &[&str]) -> Output {
    Command::new(BIN)
        .args(args)
        .output()
        .unwrap_or_else(|err| panic!("não foi possível executar `{BIN}`: {err}"))
}

/// O `stdout` da execução, como texto.
fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// O exit code, exigindo que o processo tenha terminado por exit code.
fn code(output: &Output) -> i32 {
    output
        .status
        .code()
        .unwrap_or_else(|| panic!("o processo não terminou por exit code: {:?}", output.status))
}

/// A linha do relatório que começa com `mark` e o nome do gate — para afirmar
/// sobre o veredito de **um** gate sem depender da posição dele no texto.
fn gate_line<'a>(report: &'a str, mark: &str, name: &str) -> &'a str {
    report
        .lines()
        .find(|line| line.trim_start().starts_with(mark) && line.contains(name))
        .unwrap_or_else(|| {
            panic!("não há uma linha de `{name}` com `{mark}` no relatório:\n{report}")
        })
}

/// A linha do veredito do Gate 0 — a que segue o cabeçalho do bloco dele.
///
/// O Gate 0 tem vocabulário próprio (`passou`/`RECUSADO`) e não as marcas
/// `[ ok ]`/`[falha]`/`[ -- ]` dos seis gates: ver a nota de `print_gate_0` em
/// `src/main.rs` sobre porque ele não é uma das seis linhas.
fn gate_0_line(report: &str) -> &str {
    let mut lines = report.lines();
    while let Some(line) = lines.next() {
        if line.starts_with("Gate 0 — sanitização dos metadados") {
            return lines
                .next()
                .unwrap_or_else(|| panic!("o bloco do Gate 0:\n{report}"));
        }
    }
    panic!("não há bloco do Gate 0 no relatório:\n{report}");
}

/// O caminho em `str` para passar ao binário.
fn path_arg(path: &Path) -> String {
    path.to_str().expect("caminho em UTF-8").to_string()
}

#[test]
fn the_valid_fixture_passes_gate_0_and_exits_zero() {
    let output = run(&[&path_arg(&fixture("arkhe.gguf")), "--verbose"]);
    let report = stdout(&output);

    assert_eq!(code(&output), 0, "relatório:\n{report}");
    assert!(
        report.contains("Gate 0 — sanitização dos metadados"),
        "o Gate 0 tem de estar no relatório:\n{report}"
    );

    let line = gate_0_line(&report);
    assert!(
        line.contains("passou: 0 limites excedidos"),
        "a linha do Gate 0: {line}"
    );
    assert!(
        line.contains("0 pares chave-valor e 0 tensor(es)"),
        "a linha do Gate 0 conta o que percorreu: {line}"
    );
    assert!(
        report.contains("fonte: arkhe_verify::sanitize::sanitize_gguf_with_limits"),
        "o detalhe cita a API que correu:\n{report}"
    );
    // O Gate 0 vem antes dos seis gates, e o contrato dos seis não muda por
    // causa dele: a contagem e as marcas continuam a ser as de antes.
    let gate_0 = report.find("Gate 0 — sanitização").expect("Gate 0");
    let gates = report.find("\ngates:").expect("a lista dos gates");
    assert!(gate_0 < gates, "o Gate 0 vem primeiro:\n{report}");
    assert!(
        report
            .contains("resumo: 1 de 6 gates passaram (0 falharam, 5 não avaliados); Gate 0: passou"),
        "o resumo dos seis mantém-se, com o veredito do Gate 0 dito à parte:\n{report}"
    );

    // Os seis gates continuam a correr quando o Gate 0 passa: o cabeçalho é
    // lido e o digest é calculado (e fica não avaliado por falta de manifesto).
    assert!(gate_line(&report, "[ ok ]", "cabeçalho").contains("versão 3"));
    assert!(report.contains("SHA-256 do modelo:"));
}

#[test]
fn the_malformed_file_is_rejected_by_gate_0_with_a_non_zero_exit() {
    let dir = TempDir::new("malformed");
    let path = dir.write_bytes("declara-99999.gguf", &malformed_bytes());
    let output = run(&[&path_arg(&path), "--verbose"]);
    let report = stdout(&output);

    assert_eq!(code(&output), 1, "relatório:\n{report}");

    let line = gate_0_line(&report);
    assert!(
        line.contains("RECUSADO:"),
        "o Gate 0 diz o que recusou: {line}"
    );
    assert!(
        line.contains("declara 99999"),
        "a recusa nomeia a contagem declarada: {line}"
    );
    assert!(
        // 99 999 pares × 13 bytes por par.
        line.contains("que exige no mínimo 1299987 byte(s)"),
        "a recusa nomeia o mínimo que a declaração exige: {line}"
    );
    assert!(
        line.contains("só tem 0 byte(s) depois do cabeçalho"),
        "a recusa confronta a declaração com os bytes reais: {line}"
    );
    assert!(
        report.contains("max_kv_pairs=100000") && report.contains("max_string_len=65535"),
        "o detalhe do Gate 0 imprime os limites que correram:\n{report}"
    );
}

#[test]
fn when_gate_0_refuses_nothing_else_runs() {
    let dir = TempDir::new("refused");
    let path = dir.write_bytes("declara-99999.gguf", &malformed_bytes());
    let output = run(&[&path_arg(&path), "--verbose"]);
    let report = stdout(&output);

    assert_eq!(code(&output), 1);

    // Os seis gates ficam **não avaliados**, um por um — e nenhum deles reporta
    // um veredito próprio, porque nenhum foi invocado.
    for name in [
        "cabeçalho",
        "digest",
        "assinatura",
        "inclusão",
        "quórum",
        "consistência",
    ] {
        let line = gate_line(&report, "[ -- ]", name);
        assert!(
            line.contains("o Gate 0 recusou o ficheiro antes deste gate"),
            "o gate `{name}` tem de dizer porque não correu: {line}"
        );
    }
    assert!(
        report.contains("0 de 6 gates passaram (0 falharam, 6 não avaliados); Gate 0: RECUSADO"),
        "o resumo diz por que razão o exit code é 1 sem nenhum gate ter falhado:\n{report}"
    );

    // A prova de que o digest **não** foi calculado: o relatório não traz a
    // linha do digest em bloco nenhum, e o trust root não foi lido.
    assert!(
        !report.contains("SHA-256 do modelo"),
        "o digest não pode ter sido calculado:\n{report}"
    );
    assert!(report.contains("trust root: não lido"), "{report}");

    // E nada do que os outros gates produzem aparece: sem o bloco `core`, não
    // houve pipeline de atestação.
    assert!(!report.contains("core:"), "o pipeline não correu:\n{report}");
}

#[test]
fn the_malformed_bytes_are_rejected_by_the_sanitizer_itself() {
    // O mesmo veredito pela API, para que a recusa do CLI acima não seja a única
    // testemunha dela: o que o CLI imprime é o que `sanitize_gguf` devolveu.
    let bytes = malformed_bytes();
    assert_eq!(bytes.len(), 24);
    assert_eq!(&bytes[..4], b"GGUF");
    assert_eq!(u32::from_le_bytes(bytes[4..8].try_into().expect("versão")), 3);
    assert_eq!(
        u64::from_le_bytes(bytes[8..16].try_into().expect("tensores")),
        0
    );
    assert_eq!(
        u64::from_le_bytes(bytes[16..24].try_into().expect("pares")),
        99_999
    );

    let report = arkhe_verify::sanitize::sanitize_gguf(&bytes);

    assert!(!report.ok);
    assert_eq!(report.declared_kv_count, Some(99_999));
    assert_eq!(report.kv_pairs_walked, 0, "nada foi percorrido");
    assert!(matches!(
        report.rejection,
        Some(arkhe_verify::sanitize::Rejection::DeclaredCountExceedsFile {
            declared: 99_999,
            available_bytes: 0,
            ..
        })
    ));
}
