use std::{error, fmt};

/// Errors returned by Rust API for Golem Unlimited
#[derive(Debug)]
pub enum Error {
    CannotAddPeersToSession(actix_web::http::StatusCode),
    CannotCreateBlob(actix_web::http::StatusCode),
    CannotConvertToUTF8(std::str::Utf8Error),
    CannotCreateRequest(actix_web::Error),
    CannotCreateHubSession(actix_web::http::StatusCode),
    CannotCreatePeerSession(actix_web::http::StatusCode),
    CannotDeleteBlob(actix_web::http::StatusCode),
    CannotDeleteHubSession(actix_web::http::StatusCode),
    CannotDeletePeerSession(actix_web::http::StatusCode),
    CannotGetHubSession(actix_web::http::StatusCode),
    CannotGetHubSessionConfig(actix_web::http::StatusCode),
    CannotGetPeerInfo(actix_web::http::StatusCode),
    CannotGetResponseBody(actix_web::error::PayloadError),
    CannotListHubSessions(actix_web::http::StatusCode),
    CannotListHubPeers(actix_web::http::StatusCode),
    CannotListSessionBlobs(actix_web::http::StatusCode),
    CannotListSessionPeers(actix_web::http::StatusCode),
    CannotReceiveBlob(actix_web::http::StatusCode),
    CannotReceiveBlobBody(actix_web::error::PayloadError),
    CannotSendRequest(actix_web::client::SendRequestError),
    CannotSetHubSessionConfig(actix_web::http::StatusCode),
    CannotUploadBlobFromStream(actix_web::http::StatusCode),
    CannotUpdateDeployment(actix_web::http::StatusCode),
    CannotUpdateHubSession(actix_web::http::StatusCode),
    InvalidAddress(url::ParseError),
    InvalidJSONResponse(actix_web::error::JsonPayloadError),
    InvalidPeer(String),
    SessionNotFound(String),
    ResourceNotFound,
}

impl fmt::Display for Error {
    // TODO @filipgolem please implement real Display for Error
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl error::Error for Error {}
