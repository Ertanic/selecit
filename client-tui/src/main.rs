use crate::{
    args::{Args, Command},
    proto::query_client::QueryClient,
    tui::{App, InterceptFn},
};
use clap::Parser;
use common::{AUTHORIZATION, SERVER_PORT, path::use_certs_folder};
use std::{path::PathBuf, str::FromStr};
use tonic::{
    metadata::MetadataValue,
    transport::{Certificate, Channel, ClientTlsConfig},
};

mod args;
mod path;
mod proto;
mod tui;

#[tokio::main]
async fn main() {
    let args = Args::parse();
    match args.command {
        Command::Connect {
            host,
            port,
            ca: ca_cert,
            mut token,
        } => {
            let interceptor: InterceptFn = Box::new(move |mut req: tonic::Request<()>| {
                if let Some(auth) = token.as_mut() {
                    req.metadata_mut().insert(AUTHORIZATION, MetadataValue::from_str(auth.as_str()).unwrap());
                }
                Ok(req)
            });

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

                (QueryClient::with_interceptor(channel, interceptor), server_addr)
            }
            else {
                let server_addr = format!("http://{}:{}", host, port.unwrap_or(SERVER_PORT));
                let channel = Channel::from_shared(server_addr.clone())
                    .unwrap()
                    .connect()
                    .await
                    .expect("HTTP connection failed");
                (QueryClient::with_interceptor(channel, interceptor), server_addr)
            };

            let mut terminal = ratatui::init();
            App::new(server_addr, client).await.run(&mut terminal).expect("failed to run app");
            ratatui::restore();
        }
    }
}
