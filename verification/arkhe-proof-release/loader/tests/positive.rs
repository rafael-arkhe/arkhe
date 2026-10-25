use arkhe_verify_self_demo::{load_verified_module, sha256_hex, verify_self, EXPECTED_WASM_SHA256};
use wasmtime::{Engine, Instance, Store};

const WASM: &[u8] = include_bytes!("../fixtures/arkhe_verify.wasm");
const BUNDLE: &[u8] = include_bytes!("../fixtures/arkhe_verify.wasm.bundle.json");

#[test]
fn positive_bundle_verifies() {
    let report = verify_self(WASM, BUNDLE).expect("positive local-domain fixture must verify");
    assert_eq!(report.wasm_sha256, sha256_hex(WASM));
    assert!(report.integrated_time.is_some());
    assert_eq!(report.wasm_sha256, EXPECTED_WASM_SHA256);
}

#[test]
fn verified_bytes_compile_and_execute() {
    let engine = Engine::default();
    let module = load_verified_module(&engine, WASM, BUNDLE).expect("verified WASM compiles");
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[]).unwrap();
    let answer = instance
        .get_typed_func::<(), i32>(&mut store, "answer")
        .unwrap();
    assert_eq!(answer.call(&mut store, ()).unwrap(), 42);
}
