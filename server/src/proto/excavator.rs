use crate::proto::{ExcavatorHeartbeat, ExcavatorRequest, query_server::Query};
use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
    net::SocketAddr,
    ops::Deref,
    pin::Pin,
    sync::Arc,
};
use tokio::sync::{Mutex, RwLock, mpsc};
use tokio_stream::StreamExt;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status, Streaming, codegen::tokio_stream::Stream};

type AgentsMapInner = Arc<RwLock<HashMap<AgentId, mpsc::Sender<Result<ExcavatorRequest, Status>>>>>;

#[derive(Eq, PartialEq, Hash)]
struct AgentInner {
    name: String,
    addr: SocketAddr,
}

#[derive(Clone)]
pub struct AgentId(Arc<Mutex<AgentInner>>);

impl Hash for AgentId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.blocking_lock().hash(state);
    }
}

impl Eq for AgentId {}

impl PartialEq for AgentId {
    fn eq(&self, other: &Self) -> bool {
        *self.0.blocking_lock() == *other.0.blocking_lock()
    }
}

impl Deref for AgentId {
    type Target = Arc<Mutex<AgentInner>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AgentId {
    pub fn generate(addr: SocketAddr) -> Self {
        let bytes: &[u8] = unsafe { std::slice::from_raw_parts(&addr as *const _ as *const u8, std::mem::size_of::<SocketAddr>()) };
        let hex = hex::encode(&bytes[..bytes.len() / 2]);
        let name = format!("client{}", hex);

        let inner = AgentInner { name, addr };
        Self(Arc::new(Mutex::new(inner)))
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
impl Query for ExcavatorService {
    type RunExcavatorStream = Pin<Box<dyn Stream<Item = Result<ExcavatorRequest, Status>> + Send + 'static>>;

    async fn run_excavator(&self, request: Request<Streaming<ExcavatorHeartbeat>>) -> Result<Response<Self::RunExcavatorStream>, Status> {
        let addr = request.remote_addr().ok_or(Status::aborted("remote address not found"))?;
        let id = AgentId::generate(addr);

        let (tx, rx) = tokio::sync::mpsc::channel(128);

        self.0.write().await.insert(id.clone(), tx);

        let mut stream = request.into_inner();
        tokio::spawn(async move {
            while let Some(request) = stream.next().await {
                match request {
                    Ok(_) => {
                        println!(
                            "Received a command from the excavator ({}), but that shouldn't be the case. What does it think it's doing!?",
                            id.lock().await.name
                        );
                    }
                    Err(err) => {
                        println!("Received an error from the excavator: {}", err);
                        break;
                    }
                }
            }
        });

        let stream = ReceiverStream::new(rx);
        Ok(Response::new(Box::pin(stream) as Self::RunExcavatorStream))
    }
}
