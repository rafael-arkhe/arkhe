use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf};

fn main() {
    let wasm = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap())
        .join("fixtures/arkhe_verify.wasm");
    println!("cargo:rerun-if-changed={}", wasm.display());
    println!(
        "cargo:rustc-env=ARKHE_VERIFY_WASM_SHA256={}",
        hex::encode(Sha256::digest(
            fs::read(wasm).expect("fixture WASM must exist before build")
        ))
    );
}
