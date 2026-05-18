use crate::{
    config::Config,
    modules::{Args, ExecuteResult, ModulesRegistry, info::GetInfoModule, version::AgentVersionModule},
    proto::{ExcavatorHeartbeat, ExcavatorMessage, Message, MessageResult, excavator_client::ExcavatorClient, excavator_message::Request},
};
use tokio_stream::wrappers::ReceiverStream;
use tonic::codegen::tokio_stream::StreamExt;

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

    let addr = format!("http://{}:{}", config.server.address, config.server.port.unwrap_or(1299));
    println!("listening server at {}", addr);
    let mut client = ExcavatorClient::connect(addr).await.expect("failed to connect to excavator");

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

    while let Some(command) = stream.next().await {
        if let Ok(command) = command {
            let Some(command) = command.request
            else {
                continue;
            };

            match command {
                Request::Command(cmd) => {
                    let args = Args::new(cmd.args);

                    let module = registry.get(&cmd.name).await;
                    if let Some(module) = module {
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
                    }
                    else {
                        tx.send(Message::new(cmd.uid, 1, vec![]).into()).await.expect("failed to send response");
                    }
                }
                Request::Response(_) => {}
                Request::Heartbeat(_) => {}
            }
        }
    }
}
