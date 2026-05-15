use std::{env, path::Path};

const MAIN_PROTO: &str = "main.proto";

fn main() {
    let main_proto = Path::new(&env::var("CARGO_MANIFEST_DIR").expect("CARGO_WORKSPACE not set"))
        .join("..")
        .join(MAIN_PROTO);
    println!("cargo:rerun-if-changed={}", main_proto.display());
    tonic_prost_build::compile_protos(main_proto).expect("Failed to compile protos");
}
