

type QueueId = u64;

pub enum EventType {
    Stdout,
    Stderr,
    ProcessExit
}

pub enum OutputFormat {
    None,
    Text,
    Binary
}

pub enum EventBody {
    Text(EventType, String),
    Binary(EventType, Vec<u8>),
    ProcessExit {
        exit_code : i64
    }
}

pub struct EventDestination {
    queue_id : QueueId,
    format : OutputFormat
}