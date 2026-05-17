use crate::proto::excavator_message::Request;

tonic::include_proto!("main");

pub struct MessageResult {
    pub key: String,
    pub value: String,
}

pub struct Message {
    uid: String,
    code: i64,
    results: Vec<MessageResult>,
}

impl Message {
    pub fn new(uid: String, code: i64, results: Vec<MessageResult>) -> Self {
        Self { uid, code, results }
    }
}

impl From<Message> for ExcavatorMessage {
    fn from(Message { uid, code, results }: Message) -> Self {
        let results = results
            .into_iter()
            .map(|MessageResult { key, value }: MessageResult| ExcavatorResponseResult { key, value })
            .collect();
        
        Self {
            request: Some(Request::Response(ExcavatorResponse { uid, code, results })),
        }
    }
}
