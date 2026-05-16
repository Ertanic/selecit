use std::{env, fs, path::PathBuf};

const CONFIG_FILE: &str = "config.kdl";

fn main() {
    let proto_folder = build_common::get_proto_folder();
    build_common::compile_protos_folder(&proto_folder).unwrap();

    println!("cargo:rerun-if-changed={CONFIG_FILE}");
    let target_folder = PathBuf::from_iter([
        env::var("CARGO_MANIFEST_DIR").unwrap(),
        "..".to_owned(),
        "target".to_owned(),
        env::var("PROFILE").unwrap(),
    ]);

    let config_path = target_folder.join(CONFIG_FILE);
    fs::copy(CONFIG_FILE, &config_path).unwrap();
}
