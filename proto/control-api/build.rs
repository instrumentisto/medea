use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    #[cfg(feature = "grpc")]
    grpc::compile()?;

    Ok(())
}

/// gRPC Protobuf specs compilation.
#[cfg(feature = "grpc")]
mod grpc {
    use std::{borrow::Cow, error::Error, fs, io};

    /// Path to Protobuf source files.
    const GRPC_DIR: &str = "src/grpc";

    /// Compiles gRPC protobuf specs to Rust source files.
    ///
    /// Specs will be generated only if you've deleted old generated specs.
    ///
    /// For rebuilding you may simply execute:
    /// ```bash
    /// make cargo.gen crate=medea-control-api-proto
    /// ```
    /// in the root of the project.
    pub fn compile() -> Result<(), Box<dyn Error>> {
        let proto_names = ProtoNames::load()?;
        let grpc_spec_files = proto_names.get_grpc_spec_files();
        let out_files = proto_names.get_out_files();

        grpc_spec_files
            .iter()
            .chain(out_files.iter())
            .for_each(|filename| {
                println!("cargo:rerun-if-changed={}", filename)
            });

        for filename in &out_files {
            if let Err(e) = fs::File::open(filename) {
                if let io::ErrorKind::NotFound = e.kind() {
                    tonic_build::configure()
                        .out_dir(GRPC_DIR)
                        .format(false)
                        .build_client(true)
                        .build_server(true)
                        .compile(&grpc_spec_files, &[GRPC_DIR.to_string()])?;
                    fs::remove_file(format!("{}/google.protobuf.rs", GRPC_DIR))
                        .expect(
                            "'google.protobuf.rs' file isn't generated. This \
                             is good news, because maybe hyperium/tonic#314 \
                             issue was really fixed. Check it and if it is \
                             then just remove this line of code.",
                        );
                    break;
                } else {
                    panic!("{}", e);
                }
            }
        }

        Ok(())
    }

    /// All names of Protobuf specs from [`GRPC_DIR`] directory.
    ///
    /// This entity just stores file stems (for `api.proto`'s filename file stem
    /// is `api` for example) of all files from [`GRPC_DIR`] that have `.proto`
    /// extension.
    struct ProtoNames(Vec<String>);

    impl ProtoNames {
        /// Loads [`ProtoNames`] from [`GRPC_DIR`] directory.
        pub fn load() -> io::Result<Self> {
            let proto_names = fs::read_dir(GRPC_DIR)?
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .map(|entry| entry.path())
                .filter(|path| {
                    path.extension().map_or(false, |ext| {
                        path.is_file()
                            && ext.to_string_lossy() == Cow::from("proto")
                    })
                })
                .filter_map(|path| {
                    path.file_stem()
                        .map(|stem| stem.to_string_lossy().to_string())
                })
                .collect();
            Ok(Self(proto_names))
        }

        /// Returns paths to all Protobuf files from [`GRPC_DIR`].
        pub fn get_grpc_spec_files(&self) -> Vec<String> {
            self.0
                .iter()
                .map(|name| format!("{}/{}.proto", GRPC_DIR, name))
                .collect()
        }

        /// Returns paths to files which will be generated by [`tonic`] after
        /// compilation of Protobuf specs from [`GRPC_DIR`].
        pub fn get_out_files(&self) -> Vec<String> {
            self.0
                .iter()
                .map(|filename| format!("{}/{}.rs", GRPC_DIR, filename))
                .collect()
        }
    }
}
