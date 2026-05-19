use std::path::PathBuf;
use crate::path;
use knus::Decode;

#[derive(Decode)]
pub struct Server {
    pub address: String,
    pub port: u16,
}

#[derive(Decode, Clone)]
pub struct Auth {
    #[knus(child, unwrap(argument))]
    pub token: String,
}

#[derive(Decode, Default)]
pub struct Path {
    #[knus(argument)]
    pub path: PathBuf
}

#[derive(Decode)]
pub struct Certificates {
    #[knus(child)]
    pub server_cert: Path,
    #[knus(child)]
    pub server_key: Path,
}

#[derive(Decode, Default)]
pub struct Config {
    #[knus(child)]
    pub server: Option<Server>,
    #[knus(child)]
    pub auth: Option<Auth>,
    #[knus(child)]
    pub certificates: Option<Certificates>,
}

pub async fn use_config() -> Config {
    let config_path = path::use_config_path();
    if tokio::fs::metadata(&config_path).await.is_ok() {
        let config_content = tokio::fs::read_to_string(config_path).await.expect("could not read config file");
        knus::parse("config", &config_content).expect("could not parse config file")
    }
    else {
        Config::default()
    }
}
