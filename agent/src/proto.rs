use crate::proto::excavator_message::Request;

tonic::include_proto!("main");

pub struct Message {
    uid: String,
    code: i64,
    output: String,
}

impl Message {
    pub fn new(uid: String, code: i64, output: String) -> Self {
        Self { uid, code, output }
    }
}

impl From<Message> for ExcavatorMessage {
    fn from(Message { uid, code, output }: Message) -> Self {
        Self {
            request: Some(Request::Response(ExcavatorResponse { uid, code, output })),
        }
    }
}
