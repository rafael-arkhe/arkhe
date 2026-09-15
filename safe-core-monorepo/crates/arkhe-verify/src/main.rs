//! `arkhe-verify` — o CLI que corre o pipeline de verificação sobre um ficheiro
//! GGUF e reporta **cada gate**, com o exit code correspondente.
//!
//! Uso:
//!
//! ```text
//! cargo run -p arkhe-verify -- <ficheiro.gguf> [--manifest <path>] [--trust-root <path>] [--verbose] [--strict]
//! ```
//!
//! # Porque este binário está em `src/main.rs` e **não** em `src/bin/`
//!
//! O caminho natural de um binário em Cargo seria
//! `crates/arkhe-verify/src/bin/arkhe-verify.rs`. Não é o usado aqui, e o motivo
//! é concreto: uma regra do gitignore **global** desta máquina
//! (`~/.gitignore_global:80`, o padrão `bin/`) torna qualquer `src/bin/`
//! invisível ao git. O ficheiro existiria no disco e nunca entraria no
//! repositório. `src/main.rs` é o caminho por omissão de um binário de Cargo e
//! não é coberto por essa regra — `git check-ignore -v` sobre ele devolve exit
//! 1 —, e o nome do binário fica sendo o nome do pacote: `arkhe-verify`.
//!
//! # O que este CLI liga, e o que ele não liga
//!
//! O CLI **não** verifica nada por conta própria: cada gate é uma chamada à API
//! real da casca nativa, e o veredito vem de lá. As funções usadas, com o
//! ficheiro e a linha onde vivem:
//!
//! | Gate | API real |
//! |:---|:---|
//! | cabeçalho | `arkhe_verify::gguf::parse_header` (`gguf.rs:192`) |
//! | digest | `arkhe_verify::gguf::model_digest` (`gguf.rs:326`), comparado por `arkhe_verify::facade::verify_sha256` (`facade.rs:51`, ligado em `gguf.rs:415`) |
//! | assinatura, inclusão, quórum | o pipeline de `arkhe_verify::gguf::verify_model_attestation` (`gguf.rs:395`), que delega a `arkhe_verify::facade::verify_attestation` (`facade.rs:155`) |
//! | trust root | `TrustRoot::parse` (`arkhe-verify-wasm/src/encoding.rs:82`) |
//! | consistência | **nenhuma** — não existe no core, e o gate diz isso |
//!
//! O gate da **consistência** é o caso em que o CLI reporta
//! [`Status::Skipped`] com o motivo em vez de fingir sucesso: o core não tem um
//! verificador de prova de consistência (`arkhe-verify/src/lib.rs:54-57`), e
//! `RekorClient::consistency_proof` apenas **busca** os bytes. Escrever a
//! verificação aqui seria duplicar lógica de verificação fora do core — o que a
//! arquitetura "um core, duas cascas" existe para impedir.
//!
//! Consequência que vale declarar: como este gate nunca passa, `--strict`
//! **não pode** devolver exit 0 enquanto o core não tiver esse verificador.
//!
//! # O que é o `--manifest`
//!
//! O manifesto é a **atestação** no formato que o core consome — exatamente o
//! JSON documentado em `arkhe-verify-wasm/src/attestation.rs:37-52`
//! (`payload_b64`, `payload_sha256_hex`, `merkle_*`, `signer_public_key_hex`,
//! `signature_hex`, `witnesses`, `quorum_threshold`). Não é um formato novo
//! inventado por este CLI: é o mesmo documento que o pipeline verifica.
//!
//! # Exit codes
//!
//! | Code | Quando |
//! |:---|:---|
//! | 0 | nenhum gate falhou, e (sem `--strict`) os não avaliados são tolerados |
//! | 1 | algum gate **falhou** — mesmo sem `--strict` —, ou `--strict` com gate(s) não avaliado(s) |
//! | 2 | erro de **entrada**: ficheiro, manifesto ou trust root ilegível ou ininterpretável (`clap` também usa 2 para uso incorrecto) |
//!
//! Um gate que **falhou** sai com 1 mesmo sem `--strict`: uma ferramenta de
//! verificação não pode devolver sucesso quando uma verificação que correu deu
//! negativo. O que `--strict` acrescenta é tratar os gates **não avaliados**
//! como falha também.

#![deny(unsafe_code)]

use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{bail, Context, Result};
use clap::Parser;

use arkhe_verify::gguf::{
    model_digest, parse_header, verify_model_attestation, GgufAttestationReport,
};
use arkhe_verify::TrustRoot;

/// Exit code de um erro de entrada (ficheiro, manifesto ou trust root).
const EXIT_INPUT: u8 = 2;

/// Exit code de um veredito negativo (um gate falhado, ou `--strict` com gates
/// não avaliados).
const EXIT_GATES: u8 = 1;

/// Exit code de sucesso.
const EXIT_OK: u8 = 0;

/// Verificação de um modelo GGUF: corre o pipeline da casca nativa de
/// `arkhe-verify` sobre um ficheiro e reporta cada gate.
#[derive(Debug, Parser)]
#[command(
    name = "arkhe-verify",
    version,
    about = "Verifica um modelo GGUF e reporta cada gate do pipeline",
    long_about = "Verifica um modelo GGUF pelo pipeline da casca nativa de `arkhe-verify`, gate a gate.\n\n\
                  Sem `--manifest`, só o cabeçalho do ficheiro é estruturalmente verificável: o digest \
                  SHA-256 é calculado e impresso, mas não há valor com que compará-lo. Com `--manifest`, \
                  o manifesto é a atestação no formato do core e o pipeline inteiro corre sobre ela; \
                  `--trust-root` é o que torna os gates de assinatura e de quórum avaliáveis.\n\n\
                  O gate de consistência nunca passa: o core não tem verificador de prova de \
                  consistência, e o CLI não escreve um."
)]
struct Args {
    /// Ficheiro GGUF a verificar.
    #[arg(value_name = "FILE")]
    file: PathBuf,

    /// Atestação no formato do core (o JSON que o pipeline consome). Activa os
    /// gates de assinatura, inclusão e quórum, e dá ao gate do digest o valor
    /// esperado (`payload_sha256_hex`).
    #[arg(long, value_name = "PATH")]
    manifest: Option<PathBuf>,

    /// Trust root: um array JSON de chaves públicas Ed25519 em hex. Sem este
    /// argumento (ou com uma lista vazia) nenhuma chave é confiável, e os gates
    /// de assinatura e de quórum ficam **não avaliados** — o core devolveria
    /// `false` por falta de confiança, e reportar isso como falha diria algo
    /// que não foi medido.
    #[arg(long = "trust-root", value_name = "PATH")]
    trust_root: Option<PathBuf>,

    /// Acrescenta o detalhe de cada gate: o motivo completo dos não avaliados e
    /// os factos por trás de cada veredito.
    #[arg(long)]
    verbose: bool,

    /// Trata gate não avaliado como gate não passado: se algum gate não tiver
    /// passado, o exit code é de falha.
    #[arg(long)]
    strict: bool,
}

/// O veredito de um gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Status {
    /// A verificação correu e passou.
    Passed,
    /// A verificação correu e deu negativo.
    Failed,
    /// A verificação **não** correu (ou não existe) — nunca reportado como
    /// sucesso.
    Skipped,
}

impl Status {
    /// A marca de uma linha de gate.
    fn mark(self) -> &'static str {
        match self {
            Status::Passed => "[ ok ]",
            Status::Failed => "[falha]",
            Status::Skipped => "[ -- ]",
        }
    }
}

/// Um gate do relatório: o veredito, o facto principal numa linha e o detalhe
/// que só `--verbose` mostra.
#[derive(Debug)]
struct Gate {
    name: &'static str,
    status: Status,
    summary: String,
    detail: Vec<String>,
}

impl Gate {
    /// Um gate que passou.
    fn passed(name: &'static str, summary: impl Into<String>) -> Self {
        Self {
            name,
            status: Status::Passed,
            summary: summary.into(),
            detail: Vec::new(),
        }
    }

    /// Um gate que correu e foi recusado.
    fn failed(name: &'static str, summary: impl Into<String>) -> Self {
        Self {
            name,
            status: Status::Failed,
            summary: summary.into(),
            detail: Vec::new(),
        }
    }

    /// Um gate que não foi avaliado, com o motivo **completo** em `reason`.
    ///
    /// `summary` é a forma compacta, e também tem de dizer porque não foi
    /// avaliado: `--strict` imprime os gates não passados sem `--verbose`, e
    /// uma razão que só aparecesse no modo verboso seria uma razão escondida.
    fn skipped(name: &'static str, summary: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            name,
            status: Status::Skipped,
            summary: summary.into(),
            detail: vec![format!("motivo: {}", reason.into())],
        }
    }

    /// Acrescenta uma linha de detalhe.
    fn detail(mut self, line: impl Into<String>) -> Self {
        self.detail.push(line.into());
        self
    }
}

/// Um valor de cabeçalho, ou `?` quando os bytes não estavam presentes.
///
/// O leitor é incremental (ver `gguf.rs:101-108`): um ficheiro truncado reporta
/// os campos cujos bytes existiam e `None` no primeiro que faltou.
fn field<T: std::fmt::Display>(value: Option<T>) -> String {
    match value {
        Some(value) => value.to_string(),
        None => "?".to_string(),
    }
}

fn main() -> ExitCode {
    let args = Args::parse();

    match run(&args) {
        Ok(code) => code,
        Err(err) => {
            eprintln!("arkhe-verify: {err:#}");
            ExitCode::from(EXIT_INPUT)
        }
    }
}

fn run(args: &Args) -> Result<ExitCode> {
    // O I/O vive aqui, e só aqui: a API de `arkhe-verify` é sobre `&[u8]` e não
    // conhece caminhos (`gguf.rs:16-23`). Quem lê o ficheiro é quem chama.
    let bytes = fs::read(&args.file)
        .with_context(|| format!("não foi possível ler o ficheiro `{}`", args.file.display()))?;

    let trust_root = match &args.trust_root {
        Some(path) => {
            let json = fs::read_to_string(path).with_context(|| {
                format!("não foi possível ler o trust root `{}`", path.display())
            })?;
            TrustRoot::parse(&json).with_context(|| {
                format!(
                    "trust root inválido em `{}`: esperado um array JSON de chaves públicas Ed25519 \
                     em hex, como `[\"1f8f…\", \"a30b…\"]`",
                    path.display()
                )
            })?
        }
        // `TrustRoot::default()` é o array vazio: não confia em chave nenhuma.
        None => TrustRoot::default(),
    };

    let manifest =
        match &args.manifest {
            Some(path) => Some(fs::read_to_string(path).with_context(|| {
                format!("não foi possível ler o manifesto `{}`", path.display())
            })?),
            None => None,
        };

    // Uma só chamada ao pipeline do core, quando há manifesto: é a mesma que
    // `gguf.rs` documenta como a rota do modelo (`gguf.rs:347-395`), e devolve o
    // cabeçalho, a ligação do modelo à atestação e os quatro estágios do core.
    let report = manifest
        .as_deref()
        .map(|json| verify_model_attestation(&bytes, json, &trust_root));

    // Um manifesto que não interpreta como atestação é um erro de **entrada**,
    // não um veredito: nada foi verificado ainda, e repetir a mesma causa em
    // cinco gates esconderia isso. `link` é `None` exatamente quando a
    // desserialização do documento falhou (`gguf.rs:413-415`).
    if let Some(report) = &report {
        if report.link.is_none() {
            let cause = report
                .attestation
                .error
                .clone()
                .unwrap_or_else(|| "sem causa reportada pelo pipeline".to_string());
            let path = args
                .manifest
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_default();
            bail!(
                "o manifesto `{path}` não é uma atestação interpretável pelo pipeline: {cause}\n\
                 (o manifesto deve ser o JSON que `arkhe_verify::facade::verify_attestation` consome — \
                 ver `arkhe-verify-wasm/src/attestation.rs:37-52`)"
            );
        }
    }

    let gates = vec![
        header_gate(&bytes),
        digest_gate(&bytes, report.as_ref()),
        stage_gate(Stage::Signature, report.as_ref(), &trust_root),
        stage_gate(Stage::Inclusion, report.as_ref(), &trust_root),
        stage_gate(Stage::Quorum, report.as_ref(), &trust_root),
        consistency_gate(),
    ];

    print_report(args, &bytes, &trust_root, report.as_ref(), &gates);

    let failed = gates.iter().filter(|g| g.status == Status::Failed).count();
    let skipped = gates.iter().filter(|g| g.status == Status::Skipped).count();

    // Um gate que **correu** e deu negativo nunca é tolerado.
    if failed > 0 {
        return Ok(ExitCode::from(EXIT_GATES));
    }
    if args.strict && skipped > 0 {
        return Ok(ExitCode::from(EXIT_GATES));
    }
    Ok(ExitCode::from(EXIT_OK))
}

/// Imprime o relatório: o cabeçalho, os gates, o veredito do core e o resumo.
fn print_report(
    args: &Args,
    bytes: &[u8],
    trust_root: &TrustRoot,
    report: Option<&GgufAttestationReport>,
    gates: &[Gate],
) {
    println!("arkhe-verify — verificação de um modelo GGUF");
    println!();
    println!(
        "ficheiro:   {} ({} bytes)",
        args.file.display(),
        bytes.len()
    );
    match &args.manifest {
        Some(path) => println!("manifesto:  {}", path.display()),
        None => println!("manifesto:  nenhum (sem --manifest)"),
    }
    println!(
        "trust root: {} chave(s) confiáveis{}",
        trust_root.len(),
        if args.trust_root.is_none() {
            " (sem --trust-root)"
        } else {
            ""
        }
    );
    println!();

    println!("gates:");
    for gate in gates {
        println!(
            "  {} {:<13} {}",
            gate.status.mark(),
            gate.name,
            gate.summary
        );
        if args.verbose {
            for line in &gate.detail {
                println!("              {line}");
            }
        }
    }

    // O veredito do core, sem tradução: é a conferência do relatório acima
    // contra a autoridade que o produziu. Nunca é escondido atrás de
    // `--verbose`, porque é ele que diz ao utilizador o que o pipeline concluiu.
    if let Some(report) = report {
        println!();
        println!(
            "core: ok={} (cabeçalho={} digest={} payload={} pipeline={})",
            report.ok,
            report.header.ok,
            report.link.as_ref().is_some_and(|link| link.ok),
            report.payload_is_model,
            report.attestation.ok,
        );
        println!(
            "core, estágios: sha256={} assinatura={} inclusão={} quórum={}",
            report.attestation.sha256,
            report.attestation.signature,
            report.attestation.inclusion,
            report.attestation.quorum,
        );
        if let Some(error) = &report.attestation.error {
            println!("core, causa (primeiro estágio que falhou): {error}");
        }
        if args.verbose {
            match &report.error {
                Some(error) => {
                    println!("core, causa do modelo (`GgufAttestationReport.error`): {error}")
                }
                None => println!("core, causa do modelo: nenhuma"),
            }
        }
    }

    let passed = gates.iter().filter(|g| g.status == Status::Passed).count();
    let failed = gates.iter().filter(|g| g.status == Status::Failed).count();
    let skipped = gates.iter().filter(|g| g.status == Status::Skipped).count();

    println!();
    println!(
        "resumo: {passed} de {} gates passaram ({failed} falharam, {skipped} não avaliados)",
        gates.len()
    );

    let non_passed: Vec<&Gate> = gates
        .iter()
        .filter(|g| g.status != Status::Passed)
        .collect();

    if args.strict && !non_passed.is_empty() {
        println!();
        println!(
            "strict: {} gate(s) não passaram, e --strict exige todos:",
            non_passed.len()
        );
        for gate in &non_passed {
            println!(
                "  {} {:<13} {}",
                gate.status.mark(),
                gate.name,
                gate.summary
            );
        }
        // O `--strict` deste CLI não pode devolver 0 hoje. Dizer isso é mais útil
        // do que deixar o utilizador procurar a causa.
        if non_passed.iter().any(|gate| gate.name == "consistência") {
            println!(
                "  nota: o gate `consistência` nunca passa — o core não tem verificador de prova de \
                 consistência (crates/arkhe-verify/src/lib.rs:54-57). Com --strict, este CLI não devolve \
                 exit 0 enquanto essa verificação não existir no core."
            );
        }
    } else if failed > 0 {
        println!();
        println!(
            "nota: {failed} gate(s) falharam. Um gate que correu e deu negativo sai com {EXIT_GATES} \
             mesmo sem --strict; --strict é o que trata os {skipped} gate(s) não avaliados como falha também."
        );
    }
}

/// O gate do cabeçalho: `arkhe_verify::gguf::parse_header` (`gguf.rs:192`).
///
/// É o único gate que corre sempre — não depende de manifesto nem de trust root.
fn header_gate(bytes: &[u8]) -> Gate {
    let header = parse_header(bytes);

    let gate = if header.ok {
        Gate::passed(
            "cabeçalho",
            format!(
                "versão {}, {} tensores, {} pares chave-valor",
                field(header.version),
                field(header.tensor_count),
                field(header.metadata_kv_count)
            ),
        )
    } else {
        Gate::failed(
            "cabeçalho",
            match &header.error {
                Some(cause) => format!("recusado: {cause}"),
                None => "recusado sem causa reportada".to_string(),
            },
        )
    };

    gate.detail(format!("bytes disponíveis: {}", header.available_bytes))
        .detail(format!(
            "magic `GGUF`: {}",
            if header.magic_ok { "presente" } else { "ausente" }
        ))
        .detail(format!(
            "versão declarada: {} (versões interpretadas: {:?})",
            field(header.version),
            arkhe_verify::gguf::SUPPORTED_VERSIONS
        ))
        .detail(format!("tensores: {}", field(header.tensor_count)))
        .detail(format!("pares chave-valor: {}", field(header.metadata_kv_count)))
        .detail(
            "fonte: arkhe_verify::gguf::parse_header (crates/arkhe-verify/src/gguf.rs:192). Um \
             cabeçalho válido não é um modelo válido: o leitor lê 24 bytes de estrutura e não olha os \
             pares chave-valor, os descritores de tensor nem o bloco de dados (gguf.rs:41-58).",
        )
}

/// O gate do digest: o SHA-256 do ficheiro contra o `payload_sha256_hex` que o
/// manifesto declara, **e** a identidade dos bytes do payload.
///
/// Sem manifesto, o digest é calculado e impresso — é o valor que se publica
/// num manifesto (`gguf.rs:320-328`) —, mas o gate fica `Skipped`: não houve
/// comparação nenhuma, e reportá-lo como passado seria fingir uma verificação
/// que não aconteceu.
fn digest_gate(bytes: &[u8], report: Option<&GgufAttestationReport>) -> Gate {
    let computed = model_digest(bytes);
    let source = "fonte: digest: arkhe_verify::gguf::model_digest (crates/arkhe-verify/src/gguf.rs:326); \
                  comparação: arkhe_verify::facade::verify_sha256 (facade.rs:51), ligada em gguf.rs:415";

    let Some(report) = report else {
        return Gate::skipped(
            "digest",
            format!(
                "SHA-256 do modelo: {computed} — não avaliado: sem --manifest não há digest esperado, \
                 e nada foi comparado"
            ),
            "nenhum digest esperado foi fornecido. O valor impresso é o digest calculado dos bytes do \
             ficheiro, que é o que se publica num manifesto (`payload_sha256_hex`); sem `--manifest` não \
             existe contra o que comparar. O gate fica não avaliado em vez de passado, para que o resumo \
             não conte como verificada uma comparação que não ocorreu.",
        )
        .detail(source);
    };

    let Some(link) = report.link.as_ref() else {
        return Gate::failed(
            "digest",
            "não avaliado: o manifesto não interpretou como atestação, então não há digest declarado",
        )
        .detail(source);
    };

    if !link.ok {
        return Gate::failed(
            "digest",
            format!(
                "não confere: calculado {computed}, declarado `{}`",
                link.expected_hex
            ),
        )
        .detail(match &link.error {
            Some(error) => format!("causa: {error}"),
            None => "causa: não reportada".to_string(),
        })
        .detail(source);
    }

    if !report.payload_is_model {
        return Gate::failed(
            "digest",
            format!(
                "o digest declarado confere ({computed}), mas os bytes do `payload_b64` do manifesto \
                 não são os bytes deste ficheiro — a atestação não é sobre este ficheiro"
            ),
        )
        .detail(
            "a identidade dos bytes é a checagem que não depende de resistência a colisão de hash \
             (`gguf.rs:356-362`): um payload com o mesmo digest e bytes diferentes é recusado aqui.",
        )
        .detail(source);
    }

    Gate::passed(
        "digest",
        format!(
            "SHA-256 do modelo: {computed} — confere com o `payload_sha256_hex` do manifesto, e os \
             bytes do payload são os do ficheiro"
        ),
    )
    .detail(format!("calculado: {}", computed))
    .detail(format!("declarado: {}", link.expected_hex))
    .detail(source)
}

/// Um estágio do pipeline da atestação, do relatório do core.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stage {
    /// Estágio 2: a assinatura do signatário, contra o trust root.
    Signature,
    /// Estágio 3: a prova de inclusão Merkle RFC 6962.
    Inclusion,
    /// Estágio 4: o quórum de witnesses distintos e confiáveis.
    Quorum,
}

impl Stage {
    /// O nome do gate.
    fn name(self) -> &'static str {
        match self {
            Stage::Signature => "assinatura",
            Stage::Inclusion => "inclusão",
            Stage::Quorum => "quórum",
        }
    }

    /// Se o estágio depende do trust root para ser avaliável.
    ///
    /// A inclusão **não** depende: a verificação RFC 6962 compara a prova com a
    /// raiz declarada, e não consulta chave nenhuma.
    fn needs_trust_root(self) -> bool {
        match self {
            Stage::Signature | Stage::Quorum => true,
            Stage::Inclusion => false,
        }
    }

    /// O veredito do estágio, do relatório do core.
    fn verdict(self, report: &GgufAttestationReport) -> bool {
        match self {
            Stage::Signature => report.attestation.signature,
            Stage::Inclusion => report.attestation.inclusion,
            Stage::Quorum => report.attestation.quorum,
        }
    }

    /// A citação do estágio no core, para o detalhe.
    fn source(self) -> &'static str {
        match self {
            Stage::Signature => {
                "fonte: arkhe_verify::gguf::verify_model_attestation (gguf.rs:395) → \
                 arkhe_verify::facade::verify_attestation (facade.rs:155) → \
                 arkhe-verify-wasm/src/attestation.rs:216 (`verify_signature_inner`)"
            }
            Stage::Inclusion => {
                "fonte: arkhe_verify::gguf::verify_model_attestation (gguf.rs:395) → \
                 arkhe_verify::facade::verify_attestation (facade.rs:155) → \
                 arkhe-verify-wasm/src/attestation.rs:228 (`verify_inclusion_inner`)"
            }
            Stage::Quorum => {
                "fonte: arkhe_verify::gguf::verify_model_attestation (gguf.rs:395) → \
                 arkhe_verify::facade::verify_attestation (facade.rs:155) → \
                 arkhe-verify-wasm/src/attestation.rs:241 (`verify_witness_quorum_inner`)"
            }
        }
    }
}

/// O gate de um estágio do pipeline da atestação.
///
/// Os três estágios vêm do **mesmo** `AttestationReport` do core, sem tradução:
/// a `bool` de cada um é o veredito, e o CLI não o recalcula. O que o CLI
/// acrescenta é a razão de um estágio não ser avaliável quando não é.
fn stage_gate(
    stage: Stage,
    report: Option<&GgufAttestationReport>,
    trust_root: &TrustRoot,
) -> Gate {
    let name = stage.name();

    let Some(report) = report else {
        return Gate::skipped(
            name,
            "não avaliado: sem --manifest não há atestação a avaliar",
            "sem `--manifest` não há assinatura, prova de inclusão nem witnesses — os três gates da \
             atestação não têm entrada nenhuma. Passe `--manifest <path>` com a atestação no formato do \
             core para os avaliar.",
        );
    };

    // Com 0 chaves confiáveis, o `false` do core mede a ausência de confiança, e
    // não a assinatura. Reportá-lo como falha diria algo que não foi medido.
    if stage.needs_trust_root() && trust_root.is_empty() {
        return Gate::skipped(
            name,
            "não avaliado: trust root vazio (0 chaves — sem --trust-root)",
            format!(
                "o trust root tem 0 chaves, e o core recusa qualquer {} por chave fora do trust root. O \
                 `false` que o pipeline devolve neste caso não mede a {}, mede que nenhuma chave é \
                 confiável — reportá-lo como falha diria algo que não foi medido. Passe `--trust-root \
                 <path>` com as chaves em que confia para avaliar este gate.",
                if stage == Stage::Signature {
                    "assinatura"
                } else {
                    "testemunha"
                },
                if stage == Stage::Signature {
                    "assinatura"
                } else {
                    "reunião de testemunhas"
                },
            ),
        )
        .detail(format!(
            "o pipeline devolveu `{}: false` com o trust root vazio; o veredito cru está impresso no \
             bloco `core` acima.",
            name
        ))
        .detail(stage.source());
    }

    let gate = if stage.verdict(report) {
        Gate::passed(
            name,
            match stage {
                Stage::Signature => {
                    "a assinatura do signatário confere e a chave está no trust root"
                }
                Stage::Inclusion => "a prova reconstrói a raiz declarada na posição declarada",
                Stage::Quorum => "há quórum de witnesses distintos e confiáveis",
            },
        )
    } else {
        Gate::failed(
            name,
            match stage {
                Stage::Signature => {
                    "o estágio `signature` do pipeline recusou (assinatura inválida **ou** chave fora \
                     do trust root)"
                }
                Stage::Inclusion => {
                    "o estágio `inclusion` do pipeline recusou: a prova não reconstrói a raiz declarada"
                }
                Stage::Quorum => {
                    "o estágio `quorum` do pipeline recusou: witnesses distintos e confiáveis \
                     insuficientes, abaixo do limiar declarado ou do mínimo de 2 do core"
                }
            },
        )
    };

    gate.detail(stage.source())
}

/// O gate da prova de consistência: **não implementado**, e o gate diz isso.
///
/// Não há função nenhuma no core para ligar aqui — `lib.rs:54-57` registra a
/// ausência, e `RekorClient::consistency_proof` apenas busca os bytes. Escrever
/// a verificação neste CLI seria duplicar lógica de verificação fora do core.
fn consistency_gate() -> Gate {
    Gate::skipped(
        "consistência",
        "não implementado: o core não tem verificador de prova de consistência",
        "não implementado, e o CLI **não** escreve a verificação que falta. O core não tem um \
         verificador de prova de consistência entre tamanhos de árvore (crates/arkhe-verify/src/lib.rs:54-57), \
         e `RekorClient::consistency_proof` apenas **busca** os bytes — verificá-los não está \
         implementado em lugar nenhum. Um verificador escrito neste CLI seria uma segunda implementação \
         de verificação fora do core, que é o que a arquitetura \"um core, duas cascas\" existe para \
         impedir. Este gate nunca passa.",
    )
    .detail(
        "fonte: nenhuma. Não há função do core para ligar — o gate reporta a ausência em vez de fingir \
         sucesso.",
    )
}
