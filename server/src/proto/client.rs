use crate::{
    proto::{
        LogsQueryRequest, LogsQueryResponse, QueryRequest, QueryResponse, TableCol, TableRow, excavator::AgentsMap, query_server::Query,
        table_col::Data,
    },
    query::{QueryExpr, QueryParseError, parse_query},
};
use futures::{StreamExt, stream};
use lazy_static::lazy_static;
use small_uid::SmallUid;
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

pub struct QueryService(LogsReceiver, AgentsMap);

impl QueryService {
    pub fn new(logs: LogsReceiver, agents: AgentsMap) -> Self {
        Self(logs, agents)
    }
}

#[tonic::async_trait]
impl Query for QueryService {
    async fn query(&self, request: Request<QueryRequest>) -> Result<Response<QueryResponse>, Status> {
        let query = request.into_inner().command;

        if query.is_empty() {
            return Err(Status::invalid_argument("empty query"));
        }

        LOGS_MANAGER.send_log(format!("got query: '{query}'")).await;

        let query = parse_query(query.trim())
            .await
            .map_err(|_| Status::invalid_argument("failed to parse query"))?;

        let uid = SmallUid::new().to_string();

        match query {
            QueryExpr::ListBy(field) => {
                let map = self.1.read().await;
                let agents = stream::iter(map.keys())
                    .filter_map(|k| async move { Some(k.lock().await.name.clone()) })
                    .map(|k| TableCol {
                        key: "name".to_owned(),
                        data: Some(Data::Str(k)),
                    })
                    .collect::<Vec<_>>()
                    .await;
                let rows = vec![TableRow { cols: agents }];
                let response = Response::new(QueryResponse { rows });
                Ok(response)
            }
            QueryExpr::SelectFrom { .. } => {
                todo!()
            }
        }
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
