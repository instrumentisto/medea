fn main() {
    let proto_root = "src/api/grpc/protos";
    println!("cargo:rerun-if-changed={}", proto_root);
    protoc_grpcio::compile_grpc_protos(
        &["control.proto"],
        &[proto_root],
        &proto_root,
    )
    .expect("Failed to compile gRPC definitions!");
}
