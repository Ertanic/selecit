use std::{env, path::PathBuf};

const CONFIG_FILE_NAME: &str = "config.kdl";

pub fn use_app_folder() -> PathBuf {
    env::current_exe()
        .expect("failed to get current executable")
        .parent()
        .expect("failed to get current executable parent")
        .to_path_buf()
}

pub fn use_config_path() -> PathBuf {
    use_app_folder().join(CONFIG_FILE_NAME)
}
