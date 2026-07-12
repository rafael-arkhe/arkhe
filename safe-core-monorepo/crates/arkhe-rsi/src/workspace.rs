//! Isolamento de workspace compartilhado entre `CargoTestEvaluator` e
//! `StaticValidator`s: nenhum dos dois deve escrever no `manifest_dir` real.

use arkhe_rsi_core::RsiError;
use std::path::Path;

/// Copia `manifest_dir` para um `tempfile::TempDir` novo, pulando `target/`
/// (recompilado do zero na cópia). O chamador nunca escreve em `manifest_dir`
/// diretamente — só no `TempDir` retornado, que se apaga sozinho ao sair de
/// escopo, com sucesso ou falha.
pub(crate) fn isolate(manifest_dir: &Path) -> Result<tempfile::TempDir, RsiError> {
    let workspace = tempfile::tempdir()
        .map_err(|e| RsiError::Backend(format!("failed to create isolated workspace: {e}")))?;
    copy_dir_excluding_target(manifest_dir, workspace.path()).map_err(|e| {
        RsiError::Backend(format!(
            "failed to copy {} into isolated workspace: {e}",
            manifest_dir.display()
        ))
    })?;
    Ok(workspace)
}

fn copy_dir_excluding_target(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if entry.file_type()?.is_dir() {
            if entry.file_name() == "target" {
                continue;
            }
            copy_dir_excluding_target(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}
