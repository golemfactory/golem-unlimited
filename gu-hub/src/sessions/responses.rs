use actix::prelude::*;
use actix_web::{
    dev::HttpResponseBuilder,
    error::InternalError,
    http::{
        header::{HeaderValue, ETAG},
        StatusCode,
    },
    Error as ActixError, HttpResponse,
};
use gu_model::session::BlobInfo;
use gu_net::NodeId;
use serde_json::Value;
use sessions::{blob::Blob, manager::EnumeratedSessionInfo, session::SessionInfo};

pub type SessionResult = Result<SessionOk, SessionErr>;

fn include_version(mut build: HttpResponseBuilder, v: u64) -> HttpResponseBuilder {
    let val = HeaderValue::from_str(&format!("{}", v)).expect("Invalid ETag");
    build.header(ETAG, val);
    build
}

//#[derive(Debug)]
pub enum SessionOk {
    Ok,
    SessionsList(Vec<EnumeratedSessionInfo>, u64),
    SessionId(u64),
    BlobId(u64),
    Blob(Blob),
    SessionInfo(SessionInfo, u64),
    SessionJson(Value),
    SessionAlreadyDeleted,
    BlobAlreadyDeleted,
}

#[derive(Debug, Clone)]
pub enum SessionErr {
    OverwriteError,
    SessionNotFoundError,
    BlobNotFoundError,
    BlobLockedError,
    DirectoryCreationError(String),
    FileError(String),
    MailboxError(String),
    NodeNotFound(NodeId),
    CannotCreatePeerDeployment,
}

impl ::std::fmt::Display for SessionErr {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> Result<(), ::std::fmt::Error> {
        write!(f, "session error")
    }
}

impl From<MailboxError> for SessionErr {
    fn from(e: MailboxError) -> Self {
        SessionErr::MailboxError(format!("{}", e))
    }
}

impl actix_web::ResponseError for SessionErr {}

impl ::std::error::Error for SessionErr {}

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
            SessionErr::OverwriteError => HttpResponse::InternalServerError().body("Id conflict"),
            SessionErr::SessionNotFoundError => HttpResponse::NotFound().body("Session not found"),
            SessionErr::BlobNotFoundError => HttpResponse::NotFound().body("Blob not found"),
            SessionErr::BlobLockedError => {
                HttpResponse::build(StatusCode::from_u16(423).expect("Wrong http code - 423"))
                    .finish()
            }
            SessionErr::DirectoryCreationError(s) => {
                HttpResponse::InternalServerError().body(format!("Cannot create directory: {}", s))
            }
            SessionErr::FileError(s) => {
                HttpResponse::InternalServerError().body(format!("File related error: {}", s))
            }
            SessionErr::MailboxError(s) => {
                HttpResponse::InternalServerError().body(format!("Actix mailbox error: {}", s))
            }
            SessionErr::CannotCreatePeerDeployment => {
                HttpResponse::InternalServerError().body(format!("Cannot create peer deployment."))
            }
            SessionErr::NodeNotFound(node_id) => {
                HttpResponse::NotFound().body(format!("Node not found {:?}.", node_id))
            }
        }
    }
}

/*impl Into<ActixError> for SessionErr {
    fn into(self) -> ActixError {
        error!("{:?}", &self);

        InternalError::from_response("", Into::<HttpResponse>::into(self)).into()
    }
}*/
