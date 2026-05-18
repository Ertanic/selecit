use std::{env, path::PathBuf};

const CERTS_FOLDER: &str = "certs";

pub fn use_app_folder() -> PathBuf {
    env::current_exe()
        .expect("failed to get current executable")
        .parent()
        .expect("failed to get current executable parent")
        .to_path_buf()
}

#[cfg(debug_assertions)]
pub fn use_project_folder() -> PathBuf {
    PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("failed to get CARGO_MANIFEST_DIR"))
}

#[cfg(debug_assertions)]
pub fn use_workspace_folder() -> PathBuf {
    use_project_folder().join("..")
}

pub fn use_certs_folder() -> PathBuf {
    if cfg!(debug_assertions) {
        use_workspace_folder().join(CERTS_FOLDER)
    }
    else {
        use_app_folder().join(CERTS_FOLDER)
    }
}
