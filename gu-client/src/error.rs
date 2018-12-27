/// Errors returned by Rust API for Golem Unlimited
#[derive(Debug)]
pub enum Error {
    CannotCreateBlob(actix_web::error::PayloadError),
    CannotConvertToUTF8(std::str::Utf8Error),
    CannotCreateRequest(actix_web::Error),
    CannotCreateHubSession(String),
    CannotCreatePeerSession(String),
    CannotDeleteHubSession(actix_web::http::StatusCode),
    CannotDeletePeerSession(actix_web::http::StatusCode),
    CannotGetResponseBody(actix_web::error::PayloadError),
    CannotSendRequest(actix_web::client::SendRequestError),
    InternalError(String),
    InvalidAddress(url::ParseError),
    InvalidHubSessionParameters(String),
    InvalidJSONResponse(actix_web::error::JsonPayloadError),
    InvalidPeerSessionParameters(String),
    SessionNotFound(String),
}
