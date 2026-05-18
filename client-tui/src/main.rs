use crate::{
    args::{Args, Command},
    proto::query_client::QueryClient,
    tui::App,
};
use clap::Parser;
use common::{SERVER_PORT, path::use_certs_folder};
use std::path::PathBuf;
use tonic::transport::{Certificate, Channel, ClientTlsConfig};

mod args;
mod path;
mod proto;
mod tui;

#[tokio::main]
async fn main() {
    let args = Args::parse();
    match args.command {
        Command::Connect { host, port, ca: ca_cert } => {
            let (client, server_addr) = if let Some(ca_cert) = ca_cert {
                let cert = {
                    let path = PathBuf::from(ca_cert);
                    if path.is_absolute() { path } else { use_certs_folder().join(path) }
                };

                let cert_content = tokio::fs::read_to_string(cert).await.expect("unable to read certificate file");
                let cert = Certificate::from_pem(cert_content);

                let server_addr = format!("https://{}:{}", host, port.unwrap_or(SERVER_PORT));

                let tls = ClientTlsConfig::new().ca_certificate(cert);
                let channel = Channel::from_shared(server_addr.clone())
                    .unwrap()
                    .tls_config(tls)
                    .unwrap()
                    .connect()
                    .await
                    .expect("TLS connection failed");

                println!("listening server at {}", server_addr);

                (QueryClient::new(channel), server_addr)
            }
            else {
                let server_addr = format!("http://{}:{}", host, port.unwrap_or(SERVER_PORT));
                let client = QueryClient::connect(server_addr.clone()).await.expect("failed to connect");
                (client, server_addr)
            };

            let mut terminal = ratatui::init();
            App::new(server_addr, client).await.run(&mut terminal).expect("failed to run app");
            ratatui::restore();
        }
    }
}
