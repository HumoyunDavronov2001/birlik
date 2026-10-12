//! Generates Rust types from `protocol/birlik/v1/*.proto` with a vendored `protoc`,
//! so building needs no system Protobuf installation.

use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../protocol");
    let files = ["device", "pairing", "telemetry", "plan", "cluster_service"]
        .map(|name| root.join(format!("birlik/v1/{name}.proto")));
    for file in &files {
        println!("cargo:rerun-if-changed={}", file.display());
    }
    prost_build::Config::new()
        .protoc_executable(protoc_bin_vendored::protoc_bin_path()?)
        .compile_protos(&files, &[root])?;
    Ok(())
}
