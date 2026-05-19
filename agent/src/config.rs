use knus::Decode;

#[derive(Decode)]
pub struct Server {
    #[knus(argument)]
    pub address: String,
    #[knus(argument)]
    pub port: Option<u16>,
}

#[derive(Decode)]
pub struct Certificate {
    #[knus(argument)]
    pub ca_cert: String,
}

#[derive(Decode, Default)]
pub struct Auth {
    #[knus(child, unwrap(argument))]
    pub token: String,
}

#[derive(Decode)]
pub struct Config {
    #[knus(child)]
    pub server: Server,
    #[knus(child)]
    pub auth: Option<Auth>,
    #[knus(child)]
    pub certificate: Option<Certificate>,
}
