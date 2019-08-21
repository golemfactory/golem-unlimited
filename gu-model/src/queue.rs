use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub type QueueId = u64;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MessageType {
    Request,
    Reply,
    Event,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Message<T> {
    pub id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply_to: Option<String>,
    pub ts: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub valid_to: Option<DateTime<Utc>>,
    pub msg_type: MessageType,
    #[serde(skip_serializing_if = "HashSet::is_empty")]
    pub tags: HashSet<String>,
    #[serde(flatten)]
    pub body: MessageBody<T>,
}

impl<T> Default for Message<T> {
    fn default() -> Self {
        Message {
            id: Default::default(),
            reply_to: Default::default(),
            ts: chrono::Utc::now(),
            valid_to: None,
            msg_type: MessageType::Event,
            tags: HashSet::new(),
            body: MessageBody::Text(String::default()),
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MessageBody<T> {
    Text(String),
    Binary(#[serde(with = "serde_bytes")] Vec<u8>),
    Object(T),
}

impl<T> Default for MessageBody<T> {
    fn default() -> Self {
        MessageBody::Text(String::default())
    }
}

#[derive(Clone)]
pub enum EventType {
    Stdout,
    Stderr,
    ProcessExit,
}

pub enum OutputFormat {
    None,
    Text,
    Binary,
}

pub enum EventBody {
    Text(EventType, String),
    Binary(EventType, Vec<u8>),
    ProcessExit { exit_code: i64 },
}

pub struct EventDestination {
    queue_id: QueueId,
    format: OutputFormat,
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_ser() {
        let m = Message::<()> {
            id: 0,
            body: MessageBody::Binary(vec![0, 0, 0, 0]),
            ..Message::default()
        };

        eprintln!("{}", serde_json::to_string(&m).unwrap());
    }

}
