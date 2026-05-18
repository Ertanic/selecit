use crate::{
    config::Config,
    modules::{Args, ExecuteResult, ModulesRegistry, info::GetInfoModule, version::AgentVersionModule},
    proto::{ExcavatorHeartbeat, ExcavatorMessage, Message, MessageResult, excavator_client::ExcavatorClient, excavator_message::Request},
};
use common::{SERVER_PORT, path::use_certs_folder};
use std::path::PathBuf;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{
    codegen::tokio_stream::StreamExt,
    transport::{Certificate, Channel, ClientTlsConfig},
};

mod config;
mod modules;
mod path;
mod proto;

#[tokio::main]
async fn main() {
    let registry = ModulesRegistry::default();

    registry
        .build(|builder, registry| {
            builder.register(AgentVersionModule).register(GetInfoModule::new(registry.clone()));
        })
        .await;

    let config_path = path::use_config_path();
    let config_content = tokio::fs::read_to_string(&config_path).await.expect("unable to read config file");
    let config: Config = knus::parse("config", &config_content).expect("failed to parse config");

    let mut client = if let Some(cert) = config.certificate {
        let cert = {
            let path = PathBuf::from(cert.ca_cert);
            if path.is_absolute() { path } else { use_certs_folder().join(path) }
        };

        let cert_content = tokio::fs::read_to_string(cert).await.expect("unable to read certificate file");
        let cert = Certificate::from_pem(cert_content);

        let addr = format!("https://{}:{}", config.server.address, config.server.port.unwrap_or(SERVER_PORT));

        let tls = ClientTlsConfig::new().ca_certificate(cert);
        let channel = Channel::from_shared(addr.clone())
            .unwrap()
            .tls_config(tls)
            .unwrap()
            .connect()
            .await
            .expect("TLS connection failed");

        println!("listening server at {}", addr);

        ExcavatorClient::new(channel)
    }
    else {
        let addr = format!("http://{}:{}", config.server.address, config.server.port.unwrap_or(SERVER_PORT));
        println!("listening server at {}", addr);
        ExcavatorClient::connect(addr.clone()).await.expect("connection to server failed")
    };

    let (tx, rx) = tokio::sync::mpsc::channel(128);
    tx.send(ExcavatorMessage {
        request: Some(Request::Heartbeat(ExcavatorHeartbeat::default())),
    })
    .await
    .expect("failed to send heartbeat");

    let mut stream = client
        .run_excavator(ReceiverStream::new(rx))
        .await
        .expect("failed to run excavator")
        .into_inner();

    while let Some(msg) = stream.next().await {
        let Ok(msg) = msg
        else {
            continue;
        };

        let Some(command) = msg.request
        else {
            continue;
        };

        match command {
            Request::Command(cmd) => {
                let args = Args::new(cmd.args);

                let module = registry.get(&cmd.name).await;
                if let Some(module) = module {
                    let tx = tx.clone();
                    tokio::spawn(async move {
                        let ExecuteResult { code, output } = module.execute(args).await;

                        let results = output
                            .into_iter()
                            .map(|output| MessageResult {
                                key: cmd.name.clone(),
                                value: output,
                            })
                            .collect();

                        tx.send(Message::new(cmd.uid, code, results).into())
                            .await
                            .expect("failed to send response");
                    });
                }
                else {
                    tx.send(
                        Message::new(
                            cmd.uid,
                            1,
                            vec![MessageResult {
                                key: "error".to_string(),
                                value: format!("module '{}' not found", cmd.name),
                            }],
                        )
                        .into(),
                    )
                    .await
                    .expect("failed to send response");
                }
            }
            Request::Response(_) => {}
            Request::Heartbeat(_) => {}
        }
    }
}
