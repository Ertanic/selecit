use std::{env, fs, io, path::Path};

pub const PROTO_FOLDER: &str = "proto";

pub fn get_proto_folder() -> std::path::PathBuf {
    Path::new(&env::var("CARGO_MANIFEST_DIR").expect("CARGO_WORKSPACE not set"))
        .join("..")
        .join(PROTO_FOLDER)
}

pub fn compile_protos_folder(folder: impl AsRef<Path>) -> io::Result<()> {
    let folder = folder.as_ref();
    println!("cargo:rerun-if-changed={}/*", folder.display());
    for entry in fs::read_dir(folder)? {
        tonic_prost_build::compile_protos(entry.unwrap().path())?;
    }
    Ok(())
}
