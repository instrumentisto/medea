use std::{error::Error, fs::File, io::ErrorKind};

/// Builds gRPC protobuf specs to Rust source files.
///
/// Specs will be generated only if you've deleted old generated specs.
/// For rebuilding you may simply execute
/// ```bash
/// make cargo.gen crate=medea-control-api-proto
/// ```
/// in the root of the project.
#[cfg(feature = "grpc")]
fn main() -> Result<(), Box<dyn Error>> {
    const GRPC_DIR: &str = "src/grpc";
    let proto_names = vec!["api", "callback"];
    let grpc_spec_files: Vec<String> = proto_names
        .iter()
        .map(|name| format!("{}/{}.proto", GRPC_DIR, name))
        .collect();
    let out_files: Vec<String> = proto_names
        .iter()
        .map(|filename| format!("{}/{}.rs", GRPC_DIR, filename))
        .chain(
            proto_names
                .iter()
                .map(|filename| format!("{}/{}_grpc.rs", GRPC_DIR, filename)),
        )
        .collect();

    grpc_spec_files
        .iter()
        .chain(out_files.iter())
        .for_each(|filename| println!("cargo:rerun-if-changed={}", filename));

    for filename in &out_files {
        if let Err(e) = File::open(filename) {
            if let ErrorKind::NotFound = e.kind() {
                protoc_grpcio::compile_grpc_protos(
                    &grpc_spec_files,
                    &[GRPC_DIR],
                    &GRPC_DIR,
                    None,
                )
                .expect("Failed to compile gRPC definitions");
                break;
            } else {
                panic!("{}", e);
            }
        }
    }

    Ok(())
}

#[cfg(not(feature = "grpc"))]
fn main() {}
