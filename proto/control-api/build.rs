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
fn compile_grpc_specs() -> Result<(), Box<dyn Error>> {
    const GRPC_DIR: &str = "src/grpc";

    let proto_names: Vec<String> = std::fs::read_dir(GRPC_DIR)
        .unwrap()
        .into_iter()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.is_file()
                && path.extension().unwrap().to_str().unwrap() == "proto"
        })
        .map(|path| path.file_stem().unwrap().to_str().unwrap().to_string())
        .collect();

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

fn main() -> Result<(), Box<dyn Error>> {
    #[cfg(feature = "grpc")]
    compile_grpc_specs()?;

    Ok(())
}
