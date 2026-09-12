//! `verify <dir>` — verificação canônica de diretórios de blocos.
//!
//! Espelho do `tools/verify-blocks.ps1` (mesmos 5 checks e exit codes) mais a
//! regra 6 de encadeamento estrito (superset honesto — ver docstring em lib.rs).
//!
//! Exit codes: 0 = PASS, 1 = FAIL, 2 = diretório inválido, 3 = vazio.

use arkhe_block_registry::{BlockRecord, Hash};
use serde_json::Value;
use std::collections::HashSet;
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let dir: PathBuf = std::env::args_os().nth(1).map(Into::into).unwrap_or_else(|| {
        eprintln!("uso: verify <diretorio-de-blocos>");
        std::process::exit(2);
    });

    if !dir.is_dir() {
        eprintln!("diretório não encontrado: {}", dir.display());
        return ExitCode::from(2);
    }

    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("falha ao ler diretório")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .map(|n| n.to_string_lossy().starts_with("bloco_"))
                .unwrap_or(false)
        })
        .collect();
    files.sort();

    if files.is_empty() {
        eprintln!("nenhum bloco encontrado em {}", dir.display());
        return ExitCode::from(3);
    }

    let mut issues: Vec<String> = Vec::new();
    let mut blocks: Vec<BlockRecord> = Vec::new();

    for file in &files {
        let name = file.file_name().unwrap().to_string_lossy().into_owned();
        let raw = match std::fs::read_to_string(file) {
            Ok(s) => s,
            Err(e) => {
                issues.push(format!("{name}: erro de leitura: {e}"));
                continue;
            }
        };
        match serde_json::from_str::<BlockRecord>(&raw) {
            Ok(block) => blocks.push(block),
            Err(e) => {
                // isola se é JSON malformado ou campo faltante para o reporte
                let reason = serde_json::from_str::<Value>(&raw)
                    .err()
                    .map(|p| p.to_string())
                    .unwrap_or_else(|| e.to_string());
                issues.push(format!("{name}: {reason}"));
            }
        }
    }

    // Checagem estrutural — paridade com verify-blocks.ps1
    let mut by_numero: std::collections::HashMap<u32, Vec<&BlockRecord>> =
        std::collections::HashMap::new();
    for b in &blocks {
        by_numero.entry(b.numero).or_default().push(b);
    }

    let mut hash_to_nums: std::collections::HashMap<&Hash, Vec<u32>> =
        std::collections::HashMap::new();
    for b in &blocks {
        hash_to_nums.entry(&b.hash).or_default().push(b.numero);
    }

    for (&numero, group) in &by_numero {
        let tipos: HashSet<&str> = group.iter().map(|b| b.tipo.as_str()).collect();
        if tipos.len() > 1 {
            issues.push(format!(
                "colisão de número {numero}: tipos {}",
                tipos.into_iter().collect::<Vec<_>>().join(", ")
            ));
        }
        if group.len() > 1 {
            let hashes: HashSet<&Hash> = group.iter().map(|b| &b.hash).collect();
            if hashes.len() > 1 {
                issues.push(format!(
                    "DuplicateHash no número {numero}: {} hashes diferentes",
                    hashes.len()
                ));
            }
        }
    }

    for (hash, nums) in &hash_to_nums {
        let distinct: HashSet<u32> = nums.iter().copied().collect();
        if distinct.len() > 1 {
            let list: Vec<String> = distinct.iter().map(|n| n.to_string()).collect();
            issues.push(format!("HashReuse: hash {} usado por números {}", hash.to_hex(), list.join(", ")));
        }
    }

    let known: HashSet<String> = blocks.iter().map(|b| b.hash.to_hex()).collect();
    for b in &blocks {
        if let Some(parent) = &b.parent_hash {
            if !known.contains(&parent.to_hex()) {
                issues.push(format!(
                    "órfão: bloco {} referencia parent inexistente {}",
                    b.numero,
                    parent.to_hex()
                ));
            }
        }
    }

    // Regra 6 (superset Rust): encadeamento estrito por ordem numérica
    let mut sorted: Vec<u32> = by_numero.keys().copied().collect();
    sorted.sort_unstable();
    for w in sorted.windows(2) {
        let prev = &by_numero[&w[0]][0];
        let next = &by_numero[&w[1]][0];
        if let Some(parent) = &next.parent_hash {
            if parent != &prev.hash {
                issues.push(format!(
                    "cadeia quebrada: parent do bloco {} ({}) != hash do bloco {} ({})",
                    next.numero,
                    parent.to_hex(),
                    prev.numero,
                    prev.hash.to_hex()
                ));
            }
        }
    }

    // Relatório
    let decision = if issues.is_empty() { "PASS" } else { "FAIL" };
    let report = serde_json::json!({
        "timestamp": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
        "blocks_dir": dir.canonicalize().unwrap_or(dir.clone()).to_string_lossy(),
        "total_blocks": blocks.len(),
        "parse_errors": 0,
        "issues": issues,
        "decision": decision,
    });
    print!("{}", serde_json::to_string_pretty(&report).unwrap());

    if decision == "PASS" {
        eprintln!("\nPASS: cadeia válida ({} blocos)", blocks.len());
        ExitCode::SUCCESS
    } else {
        eprintln!("\nFAIL: verificação falhou");
        ExitCode::from(1)
    }
}