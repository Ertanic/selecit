use crate::{
    config::Config,
    proto::{
        excavator::{ClientMap, ExcavatorService},
        query_server::QueryServer,
    },
};
use tonic::transport::Server;

mod config;
mod path;
mod proto;

#[tokio::main]
async fn main() {
    let config_path = path::use_config_path();
    let config = if let Ok(_) = tokio::fs::metadata(&config_path).await {
        let config_content = tokio::fs::read_to_string(config_path).await.expect("could not read config file");
        knus::parse("config", &config_content).expect("could not parse config file")
    }
    else {
        Config::default()
    };

    let addr = format!("{}:{}", config.server.address, config.server.port)
        .parse()
        .expect("could not parse address");

    println!("server listening on {}", addr);

    let client_map = ClientMap::default();

    tokio::spawn(async move {
        Server::builder()
            .add_service(QueryServer::new(ExcavatorService::new(client_map)))
            .serve(addr)
            .await
            .expect("failed to serve")
    });
}
