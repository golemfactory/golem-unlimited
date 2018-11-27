pub enum Error {
    CannotAddPeers(Vec<String>),
    CannotCreateRequest,
    CannotGetResponseBody,
    CannotSendRequest,
    InvalidHubSessionParameters,
    InvalidJSONResponse,
}
