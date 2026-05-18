use common::path::use_app_folder;
use std::path::PathBuf;

pub fn use_config_path() -> PathBuf {
    use_app_folder().join("agent.kdl")
}
