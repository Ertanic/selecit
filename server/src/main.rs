use crate::{
    config::Config,
    proto::{
        client::{QueryService, LOGS_MANAGER},
        excavator::{AgentsMap, ExcavatorService},
        excavator_server::ExcavatorServer,
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

    let client_map = AgentsMap::default();

    LOGS_MANAGER.send_log(format!("server listening on {}", addr)).await;

    Server::builder()
        .add_service(ExcavatorServer::new(ExcavatorService::new(client_map)))
        .add_service(QueryServer::new(QueryService::new(LOGS_MANAGER.subscribe())))
        .serve(addr)
        .await
        .expect("failed to serve")
}
