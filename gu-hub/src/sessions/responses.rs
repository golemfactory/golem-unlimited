use actix_web::dev::HttpResponseBuilder;
use actix_web::http::header::{HeaderValue, ETAG};
use actix_web::{error::InternalError, http::StatusCode, Error as ActixError, HttpResponse};
use serde_json::Value;
use sessions::manager::EnumeratedSessionInfo;
use sessions::{blob::Blob, session::SessionInfo};

pub type SessionResult = Result<SessionOk, SessionErr>;

pub fn to_response<A, B>(res: Result<A, B>) -> HttpResponse
where
    A: Into<HttpResponse>,
    B: Into<HttpResponse>,
{
    match res {
        Ok(a) => a.into(),
        Err(e) => e.into(),
    }
}

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

#[derive(Debug)]
pub enum SessionErr {
    OverwriteError,
    SessionNotFoundError,
    BlobNotFoundError,
    BlobLockedError,
    DirectoryCreationError(String),
    FileError(String),
    MailboxError(String),
}

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
        }
    }
}

impl Into<ActixError> for SessionErr {
    fn into(self) -> ActixError {
        error!("{:?}", &self);

        InternalError::from_response("", Into::<HttpResponse>::into(self)).into()
    }
}
