use knus::Decode;

#[derive(Decode)]
pub struct Server {
    #[knus(argument)]
    pub address: String,
    #[knus(argument)]
    pub port: Option<u16>,
}

#[derive(Decode, Default)]
pub struct TokenAuthMethod {
    pub token: String,
}

#[derive(Decode, Default)]
pub struct CertificateAuthMethod {
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

#[derive(Decode)]
pub struct Config {
    #[knus(child)]
    pub server: Server,
    #[knus(child)]
    pub auth: Auth,
}
