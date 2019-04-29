use actix::prelude::*;
use actix_web::{
    dev::HttpResponseBuilder,
    http::{
        header::{HeaderValue, ETAG},
        StatusCode,
    },
    HttpResponse,
};
use failure::Fail;
use log::error;
use serde_json::Value;

use gu_net::NodeId;

use super::{blob::Blob, manager::EnumeratedSessionInfo, session::SessionInfo};

pub type SessionResult = Result<SessionOk, SessionErr>;

fn include_version(mut build: HttpResponseBuilder, v: u64) -> HttpResponseBuilder {
    let val = HeaderValue::from_str(&format!("{}", v)).expect("Invalid ETag");
    build.header(ETAG, val);
    build
}

//#[derive(Debug)]
pub enum SessionOk {
    Ok,
    #[allow(unused)]
    SessionsList(Vec<EnumeratedSessionInfo>, u64),
    #[allow(unused)]
    SessionId(u64),
    #[allow(unused)]
    BlobId(u64),
    Blob(Blob),
    #[allow(unused)]
    SessionInfo(SessionInfo, u64),
    #[allow(unused)]
    SessionJson(Value),
    #[allow(unused)]
    SessionAlreadyDeleted,
    BlobAlreadyDeleted,
}

#[derive(Debug, Fail, Clone)]
pub enum SessionErr {
    #[fail(display = "Id conflict")]
    OverwriteError,
    #[fail(display = "Session not found")]
    SessionNotFoundError,
    #[fail(display = "Blob not found")]
    BlobNotFoundError,
    #[fail(display = "Blob locked")]
    BlobLockedError,
    #[fail(display = "Cannot create directory: {}", _0)]
    DirectoryCreationError(String),
    #[fail(display = "File related error: {}", _0)]
    FileError(String),
    #[fail(display = "Actix mailbox error: {}", _0)]
    MailboxError(String),
    #[fail(display = "{:?} node not found", _0)]
    NodeNotFound(NodeId),
    #[fail(display = "{} deployment not found", _0)]
    DeploymentNotFound(String),
    #[fail(display = "Cannot create peer deployment")]
    CannotCreatePeerDeployment,
    #[fail(display = "Cannot delete peer deployment")]
    CannotDeletePeerDeployment,
    #[fail(display = "Cannot update peer deployment")]
    CannotUpdatePeerDeployment,
}

impl From<MailboxError> for SessionErr {
    fn from(e: MailboxError) -> Self {
        SessionErr::MailboxError(format!("{}", e))
    }
}

impl actix_web::ResponseError for SessionErr {}

impl Into<HttpResponse> for SessionOk {
    fn into(self) -> HttpResponse {
        match self {
            SessionOk::Ok => HttpResponse::Ok().finish(),
            SessionOk::SessionsList(list, v) => include_version(HttpResponse::Ok(), v).json(list),
            SessionOk::SessionId(id) => HttpResponse::Created().json(id),
            SessionOk::BlobId(id) => HttpResponse::Created().json(id),
            SessionOk::Blob(_blob) => HttpResponse::Ok().finish(),
            SessionOk::SessionInfo(info, v) => include_version(HttpResponse::Ok(), v).json(info),
            SessionOk::SessionJson(val) => HttpResponse::Ok().json(val),
            SessionOk::SessionAlreadyDeleted => HttpResponse::Ok().finish(),
            SessionOk::BlobAlreadyDeleted => HttpResponse::Ok().finish(),
        }
    }
}

impl Into<HttpResponse> for SessionErr {
    fn into(self) -> HttpResponse {
        error!("{:?}", &self);

        match self {
            SessionErr::BlobLockedError => {
                HttpResponse::build(StatusCode::from_u16(423).unwrap()).finish()
            }
            x @ SessionErr::SessionNotFoundError
            | x @ SessionErr::BlobNotFoundError
            | x @ SessionErr::NodeNotFound(_)
            | x @ SessionErr::DeploymentNotFound(_) => HttpResponse::NotFound().body(x.to_string()),
            x => HttpResponse::InternalServerError().body(x.to_string()),
        }
    }
}

/*impl Into<ActixError> for SessionErr {
    fn into(self) -> ActixError {
        error!("{:?}", &self);

        InternalError::from_response("", Into::<HttpResponse>::into(self)).into()
    }
}*/
