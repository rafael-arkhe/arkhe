//! Conferência manual de um GGUF real pela API de `arkhe_verify::gguf`.
//!
//! Este exemplo é o **chamador** que a arquitetura exige: `arkhe-verify` não
//! tem I/O, então quem lê o arquivo é quem chama — e é isto aqui. A biblioteca
//! só vê `&[u8]`.
//!
//! Uso:
//!
//! ```text
//! cargo run -p arkhe-verify --example conferir_gguf -- <caminho>
//! cargo run -p arkhe-verify --example conferir_gguf -- <caminho>=<digest-esperado-hex>
//! ```
//!
//! Sem `=<digest>`, o exemplo imprime os bytes disponíveis, o cabeçalho lido e
//! o digest calculado (`model_digest`) — o valor para publicar num manifesto.
//! Com `=<digest>`, ele imprime o relatório de `verify_model_digest`: o
//! esperado deve vir de **fora** da implementação (por exemplo de
//! `sha256sum`), senão a conferência seria a implementação concordando consigo
//! mesma.
//!
//! Nenhum arquivo é lido a menos que seja passado na linha de comando, e nada
//! é embutido no binário: os GGUF reais servem de conferência, não de fixture.

use std::process::ExitCode;

use arkhe_verify::gguf::{model_digest, parse_header, verify_model_digest};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("uso: conferir_gguf <caminho>[=<digest-esperado-hex>] ...");
        return ExitCode::from(2);
    }

    let mut failed = false;

    for arg in args {
        let (path, expected) = match arg.split_once('=') {
            Some((path, expected)) => (path, Some(expected)),
            None => (arg.as_str(), None),
        };

        // O I/O vive aqui, e só aqui: a biblioteca recebe bytes.
        let bytes = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(err) => {
                eprintln!("{path}: não foi possível ler: {err}");
                failed = true;
                continue;
            }
        };

        let header = parse_header(&bytes);
        println!("== {path}");
        println!("   bytes: {}", bytes.len());
        println!(
            "   cabeçalho: ok={} magic_ok={} versão={:?} tensores={:?} pares_kv={:?}",
            header.ok,
            header.magic_ok,
            header.version,
            header.tensor_count,
            header.metadata_kv_count
        );
        if let Some(error) = &header.error {
            println!("   causa do cabeçalho: {error}");
        }
        println!("   digest SHA-256: {}", model_digest(&bytes));

        if let Some(expected) = expected {
            let report = verify_model_digest(&bytes, expected);
            println!("   esperado: {expected}");
            println!("   veredito: ok={}", report.ok);
            if let Some(error) = &report.sha256.error {
                println!("   causa do digest: {error}");
            }
            failed |= !report.ok;
        }
    }

    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
