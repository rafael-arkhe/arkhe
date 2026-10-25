//! Production admission under a host-owned release pin and TUF trust capture.
use crate::{
    enforce_report, now_unix, sha256_hex, validate_trust_root_age, verify_inclusion, VerifyError,
};
use serde::{Deserialize, Serialize};
use sigstore_trust_root::TrustedRoot;
use sigstore_types::Bundle;
use sigstore_verify::{VerificationPolicy, Verifier};
use wasmtime::{Engine, Linker, Module, Store};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Capture {
    pub root_sha256: String,
    pub captured_at_unix: u64,
}

pub struct ReleaseTrust {
    root: TrustedRoot,
    identity: String,
    expected_hash: String,
    captured_at: u64,
}
impl ReleaseTrust {
    /// The snapshot directory must come from trusted TUF acquisition, not from
    /// an agent or an arbitrary artifact's adjacent root. Checksum != authentication.
    pub fn from_trusted_capture(
        root_json: &str,
        capture: &Capture,
        identity: String,
        expected_hash: String,
    ) -> Result<Self, VerifyError> {
        if sha256_hex(root_json.as_bytes()) != capture.root_sha256 || identity.contains('*') || !identity.starts_with("https://github.com/rafael-arkhe/arkhe/.github/workflows/build-verify-wasm.yml@refs/heads/") {
            return Err(VerifyError::Configuration("mismatched trusted capture or unexpected exact workflow identity".into()));
        }
        let root = TrustedRoot::from_json(root_json)
            .map_err(|e| VerifyError::Configuration(e.to_string()))?;
        Ok(Self {
            root,
            identity,
            expected_hash,
            captured_at: capture.captured_at_unix,
        })
    }
    pub fn verify(&self, wasm: &[u8], bundle_json: &[u8]) -> Result<Module, VerifyError> {
        validate_trust_root_age(self.captured_at, now_unix()?)?;
        if sha256_hex(wasm) != self.expected_hash {
            return Err(VerifyError::HashMismatch);
        }
        verify_inclusion(bundle_json, "rekor.sigstore.dev - 1193050959916656506")?;
        let json =
            std::str::from_utf8(bundle_json).map_err(|e| VerifyError::Bundle(e.to_string()))?;
        let bundle = Bundle::from_json(json).map_err(|e| VerifyError::Bundle(e.to_string()))?;
        let policy = VerificationPolicy {
            identity: Some(self.identity.clone()),
            issuer: Some("https://token.actions.githubusercontent.com".into()),
            verify_tlog: true,
            verify_timestamp: true,
            verify_certificate: true,
            clock_skew_seconds: 60,
        };
        let result = Verifier::new(&self.root)
            .verify(wasm, &bundle, &policy)
            .map_err(|e| VerifyError::Verification(e.to_string()))?;
        enforce_report(result.success, result.warnings)?;
        Module::new(&Engine::default(), wasm).map_err(|e| VerifyError::Compilation(e.to_string()))
    }
}

/// Browser-only imports trap if reached. No import grants fake success; the raw
/// policy ABI uses only the Rust verifier and guest memory, not these JS imports.
pub fn execute_vectors(
    module: &Module,
    vectors: &serde_json::Value,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let mut store = Store::new(module.engine(), ());
    let mut linker = Linker::new(module.engine());
    linker.define_unknown_imports_as_traps(module)?;
    let instance = linker.instantiate(&mut store, module)?;
    let alloc = instance.get_typed_func::<u32, u32>(&mut store, "arkhe_policy_alloc")?;
    let free = instance.get_typed_func::<(u32, u32), ()>(&mut store, "arkhe_policy_free")?;
    let check =
        instance.get_typed_func::<(u32, u32, u32, u32), i32>(&mut store, "arkhe_policy_check")?;
    let memory = instance
        .get_memory(&mut store, "memory")
        .ok_or("memory export missing")?;
    let vectors = vectors.as_array().ok_or("vectors must be array")?;
    let (mut positives, mut negatives) = (0, 0);
    let mut results = vec![];
    for v in vectors {
        let request = serde_json::to_vec(&v["request"])?;
        let context = serde_json::to_vec(&v["context"])?;
        if request.len() > 1_048_576 || context.len() > 65_536 {
            return Err("vector too large".into());
        }
        let a = alloc.call(&mut store, request.len() as u32)?;
        let b = alloc.call(&mut store, context.len() as u32)?;
        if a == 0 || b == 0 {
            return Err("allocation failed".into());
        }
        memory.write(&mut store, a as usize, &request)?;
        memory.write(&mut store, b as usize, &context)?;
        let answer = check.call(
            &mut store,
            (a, request.len() as u32, b, context.len() as u32),
        )?;
        free.call(&mut store, (a, request.len() as u32))?;
        free.call(&mut store, (b, context.len() as u32))?;
        let expected = v["expected_allow"]
            .as_bool()
            .ok_or("expected verdict missing")?;
        if answer != i32::from(expected) {
            return Err(format!("vector {} expected {expected}, got {answer}", v["name"]).into());
        }
        if expected {
            positives += 1;
        } else {
            negatives += 1;
        }
        results.push(serde_json::json!({"name":v["name"],"allow":answer==1}));
    }
    if positives == 0 || negatives < 10 {
        return Err("positive control and >=10 attacks required".into());
    }
    Ok(serde_json::json!({"positive":positives,"rejected_attacks":negatives,"results":results}))
}
