use std::{env, path::PathBuf};

pub fn use_app_path() -> PathBuf {
    env::current_exe()
        .expect("failed to get current executable")
        .parent()
        .expect("no parent????")
        .to_path_buf()
}

pub fn use_config_path() -> PathBuf {
    use_app_path().join("config.kdl")
}
