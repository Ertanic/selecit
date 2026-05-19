use crate::{
    proto::{
        ExcavatorCommand, ExcavatorCommandArg, LogsQueryRequest, LogsQueryResponse, QueryRequest, QueryResponse, TableCol, TableRow,
        excavator::{AgentCommandManager, AgentId, AgentsMap},
        query_server::Query,
    },
    query::{QueryConditionExpr, QueryConditionTerm, QueryExpr, QueryValue, parse_query},
};
use futures::{StreamExt, stream};
use lazy_static::lazy_static;
use small_uid::SmallUid;
use std::{
    collections::HashMap,
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
    ops::Deref,
    pin::Pin,
    sync::Arc,
};
use thiserror::Error;
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

        match query {
            QueryExpr::ListBy { field, condition } => {
                let mut map = self.1.write().await;

                if let Some(condition) = condition {
                    let condition = Arc::new(condition);
                    let res = stream::iter(map.iter_mut())
                        .filter_map({
                            |(id, agent)| {
                                let condition = condition.clone();
                                let field = field.clone();
                                async move {
                                    if let Ok(result) = EvalContext::new(agent).eval(&condition).await
                                        && result
                                    {
                                        let id = id.lock().await;
                                        Some(if field.as_str() == "name" {
                                            id.name.clone()
                                        }
                                        else {
                                            id.addr.to_string()
                                        })
                                    }
                                    else {
                                        None
                                    }
                                }
                            }
                        })
                        .map({
                            let field = field.clone();
                            move |r| TableCol { key: field.clone(), data: r }
                        })
                        .collect::<Vec<_>>()
                        .await;

                    let rows = vec![TableRow { cols: res }];
                    let response = Response::new(QueryResponse { rows });
                    return Ok(response);
                }

                match field.as_str() {
                    "name" => {
                        let agents = stream::iter(map.keys())
                            .filter_map(|k| async move { Some(k.lock().await.name.clone()) })
                            .map(|data| TableCol {
                                key: "name".to_owned(),
                                data,
                            })
                            .collect::<Vec<_>>()
                            .await;
                        let rows = vec![TableRow { cols: agents }];
                        let response = Response::new(QueryResponse { rows });
                        Ok(response)
                    }
                    "addr" => {
                        let agents = stream::iter(map.keys())
                            .filter_map(|k| async move { Some(k.lock().await.addr.to_string()) })
                            .map(|data| TableCol {
                                key: "addr".to_owned(),
                                data,
                            })
                            .collect::<Vec<_>>()
                            .await;
                        let rows = vec![TableRow { cols: agents }];
                        let response = Response::new(QueryResponse { rows });
                        Ok(response)
                    }
                    _ => Err(Status::not_found("unknown field")),
                }
            }
            QueryExpr::SelectFrom { from, select } => {
                let map = self.1.read().await;
                let id = AgentId::new(&from, SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(0, 0, 0, 0), 0)));

                let mut agent = if let Some(agent) = map.get(&id).cloned() {
                    agent
                }
                else {
                    stream::iter(map.iter())
                        .filter_map(|(k, m)| Box::pin(async { if k.lock().await.name.starts_with(&from) { Some(m.clone()) } else { None } }))
                        .next()
                        .await
                        .ok_or(Status::not_found("failed to get agent id"))?
                };

                let mut rows = Vec::new();
                for crate::query::InvokeFunc { name, args } in select {
                    let uid = SmallUid::new().to_string();
                    let args = args.into_iter().map(|a| ExcavatorCommandArg { key: a.name, value: a.value }).collect();

                    LOGS_MANAGER.send_log(format!("executing command: '{name}' with args: '{args:?}'")).await;

                    let result = agent
                        .send(ExcavatorCommand {
                            uid,
                            name: name.clone(),
                            args,
                        })
                        .await;

                    if let Some(response) = result {
                        LOGS_MANAGER
                            .send_log(format!("'{name}' command is done with status '{}'", response.code))
                            .await;

                        let cols = response
                            .results
                            .into_iter()
                            .fold(HashMap::new(), |mut acc: HashMap<String, Vec<String>>, res| {
                                if let Some(col) = acc.get_mut(&res.key) {
                                    col.push(res.value);
                                }
                                else {
                                    acc.insert(res.key, vec![res.value]);
                                }
                                acc
                            });

                        if rows.is_empty() {
                            for (col_name, values) in cols {
                                for row in values {
                                    rows.push(TableRow {
                                        cols: vec![TableCol {
                                            key: col_name.clone(),
                                            data: row,
                                        }],
                                    });
                                }
                            }
                        }
                        else {
                            for (col_name, values) in &cols {
                                for (i, r) in values.iter().enumerate() {
                                    let row = rows.get_mut(i);
                                    if let Some(row) = row {
                                        row.cols.push(TableCol {
                                            key: col_name.clone(),
                                            data: r.clone(),
                                        });
                                    }
                                    else {
                                        let cols_len = rows.first().map(|r| r.cols.len()).unwrap_or(0);
                                        let cols = (0..cols_len)
                                            .filter_map(|i| {
                                                cols.iter().nth(i).map(|(c, _)| TableCol {
                                                    key: c.clone(),
                                                    data: Default::default(),
                                                })
                                            })
                                            .chain(vec![TableCol {
                                                key: col_name.clone(),
                                                data: r.clone(),
                                            }])
                                            .collect();

                                        rows.push(TableRow { cols });
                                    }
                                }
                            }
                        }
                    }
                    else {
                        return Err(Status::cancelled("failed to execute command".to_string()));
                    }
                }

                let response = Response::new(QueryResponse { rows });
                Ok(response)
            }
        }
    }

    type LogsStream = Pin<Box<dyn Stream<Item = Result<LogsQueryResponse, Status>> + Send>>;
    async fn logs(&self, _: Request<LogsQueryRequest>) -> Result<Response<Self::LogsStream>, Status> {
        let (tx, rx) = tokio::sync::mpsc::channel(128);

        let mut logs_rx = self.0.resubscribe();

        tokio::spawn(async move {
            let log = LOGS.lock().await.clone();
            let entry = LogsQueryResponse { log };
            tx.send(Ok(entry)).await.expect("failed to send log");

            while let Ok(log) = logs_rx.recv().await {
                let entry = LogsQueryResponse { log: vec![log] };
                if tx.send(Ok(entry)).await.is_err() {
                    break;
                }
            }
        });

        let stream = ReceiverStream::new(rx);
        Ok(Response::new(Box::pin(stream)))
    }
}

#[derive(Error, Debug)]
enum EvalError {
    #[error("not a number: {0}")]
    NotNumber(QueryValue),
    #[error("not a boolean: {0}")]
    NotBool(QueryValue),
    #[error("invoke error {0}")]
    InvokeError(String),
}

fn must_number(val: &QueryValue) -> Result<f64, EvalError> {
    match val {
        QueryValue::Number(n) => Ok(*n),
        _ => Err(EvalError::NotNumber(val.clone())),
    }
}

struct EvalContext<'a> {
    agent: &'a mut AgentCommandManager,
}

impl<'a> EvalContext<'a> {
    fn new(agent: &'a mut AgentCommandManager) -> Self {
        Self { agent }
    }

    async fn eval(&mut self, expr: &QueryConditionExpr) -> Result<bool, EvalError> {
        match expr {
            QueryConditionExpr::And { left, right } => {
                let left = Box::pin(self.eval(left)).await?;
                let right = Box::pin(self.eval(right)).await?;
                Ok(left && right)
            }
            QueryConditionExpr::Or { left, right } => {
                let left = Box::pin(self.eval(left)).await?;
                let right = Box::pin(self.eval(right)).await?;
                Ok(left || right)
            }
            QueryConditionExpr::Term(term) => match self.eval_term(term).await? {
                QueryValue::Bool(b) => Ok(b),
                val => Err(EvalError::NotBool(val.clone())),
            },
        }
    }

    async fn eval_term(&mut self, term: &QueryConditionTerm) -> Result<QueryValue, EvalError> {
        match term {
            QueryConditionTerm::Eq { left, right } => Ok(QueryValue::Bool(Box::pin(self.eval_eq(left, right)).await?)),
            QueryConditionTerm::More { left, right } => Ok(QueryValue::Bool(Box::pin(self.eval_more(left, right)).await?)),
            QueryConditionTerm::Less { left, right } => Ok(QueryValue::Bool(Box::pin(self.eval_less(left, right)).await?)),
            QueryConditionTerm::Value(val) => Ok(val.clone()),
        }
    }

    async fn eval_eq(&mut self, left: &QueryConditionTerm, right: &QueryConditionTerm) -> Result<bool, EvalError> {
        let left = self.eval_term(left).await?;
        let right = self.eval_term(right).await?;

        if let QueryValue::FnField { func, field } = &left {
            let cmd = ExcavatorCommand {
                uid: SmallUid::new().to_string(),
                name: func.name.clone(),
                args: func
                    .args
                    .iter()
                    .map(|a| ExcavatorCommandArg {
                        key: a.name.clone(),
                        value: a.value.clone(),
                    })
                    .collect(),
            };

            Ok(self
                .agent
                .send(cmd)
                .await
                .ok_or(EvalError::InvokeError(format!("{}.{}", func.name, field)))?
                .results
                .iter()
                .any(|r| {
                    r.key == *field
                        && (match &right {
                            QueryValue::FnField { .. } => false,
                            QueryValue::Identifier(i) => r.value == *i,
                            QueryValue::String(s) => r.value == *s,
                            QueryValue::Number(n) => {
                                if let Ok(val) = r.value.parse::<f64>() {
                                    val == *n
                                }
                                else {
                                    false
                                }
                            }
                            QueryValue::Bool(b) => *b,
                            QueryValue::Null => r.value == "null",
                        })
                }))
        }
        else {
            Ok(left == right)
        }
    }

    async fn eval_less(&mut self, left: &QueryConditionTerm, right: &QueryConditionTerm) -> Result<bool, EvalError> {
        let left = must_number(&self.eval_term(left).await?)?;
        let right = must_number(&self.eval_term(right).await?)?;
        Ok(left < right)
    }

    async fn eval_more(&mut self, left: &QueryConditionTerm, right: &QueryConditionTerm) -> Result<bool, EvalError> {
        let left = must_number(&self.eval_term(left).await?)?;
        let right = must_number(&self.eval_term(right).await?)?;
        Ok(left > right)
    }
}
