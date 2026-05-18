use common::{path, path::use_app_folder};
use std::path::PathBuf;

const CONFIG_FILE_NAME: &str = "server.kdl";

pub fn use_config_path() -> PathBuf {
    if cfg!(debug_assertions) {
        path::use_project_folder().join(CONFIG_FILE_NAME)
    }
    else {
        use_app_folder().join(CONFIG_FILE_NAME)
    }
}
