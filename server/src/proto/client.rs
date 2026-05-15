use crate::proto::{LogsQueryRequest, LogsQueryResponse, QueryRequest, QueryResponse, query_server::Query};
use lazy_static::lazy_static;
use std::{ops::Deref, pin::Pin};
use tokio::sync::{Mutex, broadcast, broadcast::error::SendError};
use tokio_stream::{Stream, wrappers::ReceiverStream};
use tonic::{Request, Response, Status};

lazy_static! {
    pub static ref LOGS_MANAGER: LogsManager = LogsManager(broadcast::channel(128).0);
    static ref LOGS: Mutex<Vec<String>> = Mutex::new(Vec::new());
}

pub struct LogsManager(broadcast::Sender<String>);

impl LogsManager {
    pub async fn send_log(&self, log: String) {
        LOGS.lock().await.push(log.clone());
        if let Err(err) = self.0.send(log) {
            match err {
                SendError(log) => {
                    LOGS.lock().await.push(format!("dropped log when no subscribers: '{log}'"));
                }
            }
        }
    }

    pub fn subscribe(&self) -> LogsReceiver {
        LogsReceiver(self.0.subscribe())
    }
}

pub struct LogsReceiver(broadcast::Receiver<String>);

impl Deref for LogsReceiver {
    type Target = broadcast::Receiver<String>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub struct QueryService(LogsReceiver);

impl QueryService {
    pub fn new(logs: LogsReceiver) -> Self {
        Self(logs)
    }
}

#[tonic::async_trait]
impl Query for QueryService {
    async fn query(&self, request: Request<QueryRequest>) -> Result<Response<QueryResponse>, Status> {
        todo!()
    }

    type LogsStream = Pin<Box<dyn Stream<Item = Result<LogsQueryResponse, Status>> + Send>>;
    async fn logs(&self, request: Request<LogsQueryRequest>) -> Result<Response<Self::LogsStream>, Status> {
        let (tx, rx) = tokio::sync::mpsc::channel(128);

        let mut logs_rx = self.0.resubscribe();

        tokio::spawn(async move {
            let log = LOGS.lock().await.clone();
            let entry = LogsQueryResponse { log };
            tx.send(Ok(entry)).await.expect("failed to send log");

            while let Ok(log) = logs_rx.recv().await {
                let entry = LogsQueryResponse { log: vec![log] };
                tx.send(Ok(entry)).await.expect("failed to send log");
            }
        });

        let stream = ReceiverStream::new(rx);
        Ok(Response::new(Box::pin(stream)))
    }
}
