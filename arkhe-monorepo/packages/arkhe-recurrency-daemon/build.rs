fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Self-contained protoc: prefer a vendored binary so this crate builds
    // on any host, falling back to a system `protoc` on PATH if present.
    if std::env::var("PROTOC").is_err() {
        if let Ok(path) = protoc_bin_vendored::protoc_bin_path() {
            std::env::set_var("PROTOC", path);
        }
    }
    tonic_build::configure()
        .build_server(true)
        .compile_protos(&["proto/recurrency.proto"], &["proto"])?;
    Ok(())
}
