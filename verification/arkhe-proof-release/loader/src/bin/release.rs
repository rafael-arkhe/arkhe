use arkhe_verify_self_demo::{
    production::{execute_vectors, Capture, ReleaseTrust},
    sha256_hex,
};
use sigstore_trust_root::TrustedRoot;
use std::{
    fs,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<_> = std::env::args().collect();
    match a.get(1).map(String::as_str) {
        Some("capture-trust-cosign") if a.len()==4 => {
            // Acquisition is online and authenticated by Cosign's TUF client.
            // The subsequent verify-run phase performs no network operations.
            let status=std::process::Command::new(&a[2]).arg("initialize").status()?;
            if !status.success() { return Err("Cosign TUF initialization failed; no fallback".into()); }
            let home=std::env::var_os("USERPROFILE").or_else(||std::env::var_os("HOME")).ok_or("home unavailable")?;
            let base=std::path::PathBuf::from(home).join(".sigstore/root");
            let candidate=[base.join("tuf-repo-cdn.sigstore.dev/targets/trusted_root.json"),base.join("targets/trusted_root.json")]
                .into_iter().find(|p|p.is_file()).ok_or("authenticated Cosign root cache missing")?;
            let json=fs::read_to_string(candidate)?;
            TrustedRoot::from_json(&json)?;
            let capture=Capture {root_sha256:sha256_hex(json.as_bytes()),captured_at_unix:SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs()};
            let dir=Path::new(&a[3]); fs::create_dir_all(dir)?;
            fs::write(dir.join("trusted_root.json"),json)?;
            fs::write(dir.join("capture.json"),serde_json::to_vec_pretty(&capture)?)?;
            println!("Cosign TUF-authenticated trust captured; offline verification can now run");
        }
        Some("capture-trust") if a.len()==3 => {
            let root=TrustedRoot::production().await?;
            let json=serde_json::to_string_pretty(&root)?;
            let capture=Capture {root_sha256:sha256_hex(json.as_bytes()),captured_at_unix:SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs()};
            let dir=Path::new(&a[2]); fs::create_dir_all(dir)?;
            fs::write(dir.join("trusted_root.json"),json)?;
            fs::write(dir.join("capture.json"),serde_json::to_vec_pretty(&capture)?)?;
            println!("TUF-authenticated public trust snapshot captured");
        }
        Some("verify-run") if a.len()==9 => {
            let wasm=fs::read(&a[2])?; let bundle=fs::read(&a[3])?; let dir=Path::new(&a[4]);
            let root=fs::read_to_string(dir.join("trusted_root.json"))?;
            let capture:Capture=serde_json::from_slice(&fs::read(dir.join("capture.json"))?)?;
            let trust=ReleaseTrust::from_trusted_capture(&root,&capture,a[5].clone(),a[6].clone())?;
            let module=trust.verify(&wasm,&bundle)?;
            let mut report=execute_vectors(&module,&serde_json::from_slice(&fs::read(&a[7])?)?)?;
            report["wasm_sha256"]=serde_json::json!(sha256_hex(&wasm)); report["identity"]=serde_json::json!(&a[5]);
            fs::write(&a[8],serde_json::to_vec_pretty(&report)?)?;
            println!("public attestation verified offline; production WASM policy vectors executed");
        }
        Some("inspect-local-abi") if a.len()==5 => {
            let wasm=fs::read(&a[2])?; let engine=wasmtime::Engine::default(); let module=wasmtime::Module::new(&engine,&wasm)?;
            let report=execute_vectors(&module,&serde_json::from_slice(&fs::read(&a[3])?)?)?;
            fs::write(&a[4],serde_json::to_vec_pretty(&report)?)?;
            println!("local ABI diagnostic only; no attestation claim");
        }
        _=>return Err("usage: release capture-trust DIR | verify-run WASM BUNDLE TRUST_DIR IDENTITY HASH VECTORS REPORT | inspect-local-abi WASM VECTORS REPORT".into()),
    }
    Ok(())
}
