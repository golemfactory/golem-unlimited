/// Errors returned by Rust API for Golem Unlimited
#[derive(Debug)]
pub enum Error {
    CannotAddPeers(Vec<String>),
    CannotCreateBlob,
    CannotCreateRequest,
    CannotCreateSession,
    CannotGetResponseBody,
    CannotSendRequest,
    InternalError,
    InvalidHubSessionParameters,
    InvalidPeerSessionParameters,
    InvalidJSONResponse,
    SessionNotFound(String),
}
