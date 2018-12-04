/// Errors returned by Rust API for Golem Unlimited
#[derive(Debug)]
pub enum Error {
    CannotCreateBlob,
    CannotCreateRequest,
    CannotCreateHubSession,
    CannotCreatePeerSession,
    CannotGetResponseBody,
    CannotSendRequest,
    InternalError,
    InvalidAddress,
    InvalidHubSessionParameters,
    InvalidJSONResponse,
    InvalidPeerSessionParameters,
    SessionNotFound(String),
}
