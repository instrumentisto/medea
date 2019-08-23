use std::{env, fs::File, io::Write as _};

static CONTROL_API_MOD_RS: &[u8] = b"
/// Generated from protobuf.
pub mod control;
/// Generated from protobuf.
pub mod control_grpc;
";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = env::var("OUT_DIR")?;

    protoc_grpcio::compile_grpc_protos(
        &["proto/control.proto"],
        &["proto"],
        &out_dir,
        None,
    )
    .expect("Failed to compile gRPC definitions!");
    File::create(out_dir + "/mod.rs")?.write_all(CONTROL_API_MOD_RS)?;
    Ok(())
}
