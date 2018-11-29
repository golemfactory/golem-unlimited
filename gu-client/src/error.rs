pub enum Error {
    CannotAddPeers(Vec<String>),
    CannotCreateBlob,
    CannotCreateRequest,
    CannotCreateSession,
    CannotGetResponseBody,
    CannotSendRequest,
    InternalError,
    InvalidHubSessionParameters,
    InvalidJSONResponse,
    SessionNotFound,
}
