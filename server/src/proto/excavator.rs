use crate::proto::{ExcavatorRequest, ExcavatorResponse, HandshakeRequest, HandshakeResponse, query_server::Query};
use std::pin::Pin;
use tonic::{Request, Response, Status, Streaming, codegen::tokio_stream::Stream};

#[derive(Default)]
pub struct ExcavatorService;

#[tonic::async_trait]
impl Query for ExcavatorService {
    type RunExcavatorStream = Pin<Box<dyn Stream<Item = Result<ExcavatorResponse, Status>> + Send>>;

    async fn run_excavator(&self, request: Request<Streaming<ExcavatorRequest>>) -> Result<Response<Self::RunExcavatorStream>, Status> {
        todo!()
    }

    async fn handshake(&self, request: Request<HandshakeRequest>) -> Result<Response<HandshakeResponse>, Status> {
        todo!()
    }
}
