use knus::Decode;

#[derive(Decode)]
pub struct Server {
    pub address: String,
    pub port: u16,
}

impl Default for Server {
    fn default() -> Self {
        Self {
            address: "0.0.0.0".to_string(),
            port: 1299,
        }
    }
}

#[derive(Decode, Default)]
pub struct TokenAuthMethod {
    pub token: String,
}

#[derive(Decode, Default)]
pub struct CertificateAuthMethod {
    server_cert: String,
    server_key: String,
    ca_cert: String,
}

#[derive(Decode, Default)]
pub enum AuthType {
    #[default]
    None,
    Token(TokenAuthMethod),
    Certificate(CertificateAuthMethod),
}

#[derive(Decode, Default)]
pub struct Auth {
    #[knus(property)]
    pub enabled: bool,
    #[knus(child)]
    pub auth_type: AuthType,
}

#[derive(Decode, Default)]
pub struct Config {
    #[knus(child)]
    pub server: Server,
    #[knus(child)]
    pub auth: Auth,
}
