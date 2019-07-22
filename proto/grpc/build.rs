fn main() {
    let proto_root = ".";
    let proto_output = "./src";
    println!("cargo:rerun-if-changed={}", proto_root);
    protoc_grpcio::compile_grpc_protos(
        &["control.proto"],
        &[proto_root],
        &proto_output,
    )
    .expect("Failed to compile gRPC definitions!");
}
