/// Errors returned by Rust API for Golem Unlimited
#[derive(Debug)]
pub enum Error {
    CannotCreateBlob(String),
    CannotCreateRequest(String),
    CannotCreateHubSession(String),
    CannotCreatePeerSession(String),
    CannotGetResponseBody(String),
    CannotSendRequest(String),
    InternalError(String),
    InvalidAddress(String),
    InvalidHubSessionParameters(String),
    InvalidJSONResponse(String),
    InvalidPeerSessionParameters(String),
    SessionNotFound(String),
}
