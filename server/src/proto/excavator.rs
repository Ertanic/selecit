use crate::proto::{ExcavatorCommand, ExcavatorMessage, ExcavatorResponse, client::LOGS_MANAGER, excavator_message, excavator_server::Excavator};
use futures::stream::StreamExt;
use std::{
    collections::HashMap,
    error::Error,
    hash::{Hash, Hasher},
    net::SocketAddr,
    ops::Deref,
    pin::Pin,
    sync::Arc,
};
use tokio::{
    runtime::Handle,
    sync::{Mutex, RwLock, broadcast, mpsc},
    task::block_in_place,
};
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Code, Request, Response, Status, Streaming, codegen::tokio_stream::Stream};

type AgentsMapInner = Arc<RwLock<HashMap<AgentId, AgentCommandManager>>>;

#[derive(Eq, PartialEq, Hash)]
pub struct AgentInner {
    pub name: String,
    pub addr: SocketAddr,
}

#[derive(Clone)]
pub struct AgentId(Arc<Mutex<AgentInner>>);

impl Hash for AgentId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let inner = block_in_place(|| Handle::current().block_on(async { self.0.lock().await }));
        inner.name.hash(state);
    }
}

impl Eq for AgentId {}

impl PartialEq for AgentId {
    fn eq(&self, other: &Self) -> bool {
        block_in_place(|| {
            Handle::current().block_on(async {
                let left = self.0.lock().await;
                // if the right is the left, then instead of catching a deadlock, we return true.
                match other.0.try_lock() {
                    Ok(right) => *left == *right,
                    Err(_) => true,
                }
            })
        })
    }
}

impl Deref for AgentId {
    type Target = Arc<Mutex<AgentInner>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AgentId {
    pub fn new(name: &str, addr: SocketAddr) -> Self {
        let inner = AgentInner { name: name.to_owned(), addr };
        Self(Arc::new(Mutex::new(inner)))
    }

    pub fn generate(addr: SocketAddr) -> Self {
        let bytes: &[u8] = unsafe { std::slice::from_raw_parts(&addr as *const _ as *const u8, std::mem::size_of::<SocketAddr>()) };
        let hex = hex::encode(&bytes[..bytes.len() / 2]);
        let name = format!("agent{}", hex);

        let inner = AgentInner { name, addr };
        Self(Arc::new(Mutex::new(inner)))
    }
}

pub struct AgentCommandManager {
    command_tx: mpsc::Sender<Result<ExcavatorMessage, Status>>,
    response_rx: broadcast::Receiver<ExcavatorResponse>,
}

impl AgentCommandManager {
    pub async fn send(&mut self, msg: ExcavatorCommand) -> Option<ExcavatorResponse> {
        let uid = msg.uid.clone();
        let msg = ExcavatorMessage {
            request: Some(excavator_message::Request::Command(msg)),
        };

        self.command_tx.send(Ok(msg)).await.expect("channel closed");
        while let Ok(msg) = self.response_rx.recv().await {
            if msg.uid == uid {
                return Some(msg);
            }
        }

        None
    }
}

impl Clone for AgentCommandManager {
    fn clone(&self) -> Self {
        Self {
            command_tx: self.command_tx.clone(),
            response_rx: self.response_rx.resubscribe(),
        }
    }
}

#[derive(Default, Clone)]
pub struct AgentsMap(AgentsMapInner);

impl Deref for AgentsMap {
    type Target = AgentsMapInner;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub struct ExcavatorService(AgentsMap);

impl ExcavatorService {
    pub fn new(map: AgentsMap) -> Self {
        Self(map)
    }
}

#[tonic::async_trait]
impl Excavator for ExcavatorService {
    type RunExcavatorStream = Pin<Box<dyn Stream<Item = Result<ExcavatorMessage, Status>> + Send + 'static>>;

    async fn run_excavator(&self, request: Request<Streaming<ExcavatorMessage>>) -> Result<Response<Self::RunExcavatorStream>, Status> {
        let addr = request.remote_addr().ok_or(Status::aborted("remote address not found"))?;
        let mut stream = request.into_inner();

        match stream.next().await {
            Some(msg) => match msg {
                Ok(msg) => {
                    if let Some(heartbeat) = msg.request {
                        match heartbeat {
                            excavator_message::Request::Heartbeat(_) => {
                                let id = AgentId::generate(addr);

                                let (command_tx, command_rx) = mpsc::channel(128);
                                let (response_tx, response_rx) = broadcast::channel(128);
                                let manager = AgentCommandManager { command_tx, response_rx };

                                self.0.write().await.insert(id.clone(), manager);

                                tokio::spawn({
                                    let id = id.clone();
                                    let map = self.0.clone();
                                    async move {
                                        while let Some(msg) = stream.next().await {
                                            match msg {
                                                Ok(ExcavatorMessage { request: Some(request) }) => match request {
                                                    excavator_message::Request::Heartbeat(_) => {
                                                        LOGS_MANAGER.send_log(format!("agent ({}) sent heartbeat", id.lock().await.name)).await;
                                                    }
                                                    excavator_message::Request::Response(response) => {
                                                        if let Err(err) = response_tx.send(response) {
                                                            LOGS_MANAGER.send_log(format!("occurred error: {err}")).await;
                                                            break;
                                                        }
                                                    }
                                                    excavator_message::Request::Command(_) => {
                                                        LOGS_MANAGER.send_log(format!("agent ({}) sent command", id.lock().await.name)).await;
                                                    }
                                                },
                                                Err(err) => {
                                                    LOGS_MANAGER.send_log(format!("occurred error: {err}")).await;
                                                    break;
                                                }
                                                _ => {
                                                    LOGS_MANAGER
                                                        .send_log(format!("received empty message from {}", id.lock().await.name))
                                                        .await;
                                                }
                                            }
                                        }

                                        LOGS_MANAGER.send_log(format!("exit agent ({}) event loop", id.lock().await.name)).await;

                                        if map.write().await.remove_entry(&id).is_some() {
                                            LOGS_MANAGER.send_log(format!("agent ({}) disconnected", id.lock().await.name)).await;
                                        }
                                    }
                                });

                                LOGS_MANAGER
                                    .send_log(format!("agent ({}) connected successfully", id.lock().await.name))
                                    .await;

                                let stream = ReceiverStream::new(command_rx);
                                Ok(Response::new(Box::pin(stream) as Self::RunExcavatorStream))
                            }
                            _ => {
                                LOGS_MANAGER
                                    .send_log(format!(
                                        "the agent ({addr}) tried to connect, but the first message should be a heartbeat"
                                    ))
                                    .await;

                                Err(Status::failed_precondition("no heartbeat"))
                            }
                        }
                    }
                    else {
                        LOGS_MANAGER
                            .send_log(format!("the agent {addr} tried to connect, but no heartbeat was detected"))
                            .await;
                        Err(Status::failed_precondition("empty heartbeat"))
                    }
                }
                Err(err) => {
                    LOGS_MANAGER
                        .send_log(format!(
                            "agent {addr} tried to connect, but the connection could not be established due to: {err}"
                        ))
                        .await;

                    Err(Status::unknown("failed to connect"))
                }
            },
            None => {
                LOGS_MANAGER
                    .send_log(format!(
                        "the agent ({addr}) tried to connect, but it failed because no message was received"
                    ))
                    .await;

                Err(Status::unknown("failed to connect"))
            }
        }
    }
}
