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
    const GRPC_DIR: &str = "src/grpc/";
    const GRPC_SPEC_FILE: &str = "src/grpc/api.proto";
    const OUT_FILES: [&str; 2] = ["src/grpc/api.rs", "src/grpc/api_grpc.rs"];

    println!("cargo:rerun-if-changed={}", GRPC_SPEC_FILE);
    for filename in &OUT_FILES {
        println!("cargo:rerun-if-changed={}", filename);
    }

    for filename in &OUT_FILES {
        if let Err(e) = File::open(filename) {
            if let ErrorKind::NotFound = e.kind() {
                protoc_grpcio::compile_grpc_protos(
                    &[GRPC_SPEC_FILE],
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
