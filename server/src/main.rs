use crate::{
    config::use_config,
    proto::{
        client::{LOGS_MANAGER, QueryService},
        excavator::{AgentsMap, ExcavatorService},
        excavator_server::ExcavatorServer,
        query_server::QueryServer,
    },
};
use common::{SERVER_PORT, path::use_certs_folder};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use tonic::transport::{Identity, Server, ServerTlsConfig};

mod config;
mod path;
mod proto;
mod query;

#[tokio::main]
async fn main() {
    let config = use_config().await;

    let addr = if let Some(server) = config.server {
        format!("{}:{}", server.address, server.port).parse().expect("could not parse address")
    }
    else {
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), SERVER_PORT)
    };

    let client_map = AgentsMap::default();

    LOGS_MANAGER.send_log(format!("server listening on {}", addr)).await;

    let mut builder = if let Some(certs) = config.certificates {
        let cert_path = {
            let path = certs.server_cert.path;
            if path.is_absolute() { path } else { use_certs_folder().join(path) }
        };

        let key_path = {
            let path = certs.server_key.path;
            if path.is_absolute() { path } else { use_certs_folder().join(path) }
        };

        let cert = tokio::fs::read_to_string(cert_path).await.expect("could not read certificate");
        let key = tokio::fs::read_to_string(key_path).await.expect("could not read key");

        let identity = Identity::from_pem(cert, key);

        let tls_config = ServerTlsConfig::new().identity(identity);
        match Server::builder().tls_config(tls_config) {
            Ok(b) => {
                LOGS_MANAGER.send_log("server using tls".to_owned()).await;
                b
            }
            Err(err) => {
                LOGS_MANAGER.send_log(format!("failed to set tls config: {}", err)).await;
                Server::builder()
            }
        }
    }
    else {
        Server::builder()
    };

    builder
        .add_service(ExcavatorServer::new(ExcavatorService::new(client_map.clone())))
        .add_service(QueryServer::new(QueryService::new(LOGS_MANAGER.subscribe(), client_map)))
        .serve(addr)
        .await
        .expect("failed to serve")
}
