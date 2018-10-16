use actix_web::http::StatusCode;
use actix_web::HttpResponse;
use serde_json::Value;
use sessions::blob::Blob;
use sessions::session::SessionInfo;

pub type SessionResult = Result<SessionOk, SessionErr>;

pub fn to_response(res: Result<SessionOk, SessionErr>) -> HttpResponse {
    match res {
        Ok(a) => a.into(),
        Err(e) => e.into(),
    }
}

#[derive(Debug)]
pub enum SessionOk {
    Ok,
    SessionsList(Vec<SessionInfo>),
    SessionId(u64),
    BlobId(u64),
    Blob(Blob),
    SessionInfo(SessionInfo),
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
    FileError(String),
    MailboxError(String),
}

impl Into<HttpResponse> for SessionOk {
    fn into(self) -> HttpResponse {
        match self {
            SessionOk::Ok => HttpResponse::Ok().finish(),
            SessionOk::SessionsList(list) => HttpResponse::Ok().json(list),
            SessionOk::SessionId(id) => HttpResponse::Created().json(id),
            SessionOk::BlobId(id) => HttpResponse::Created().json(id),
            SessionOk::Blob(_blob) => HttpResponse::Ok().finish(),
            SessionOk::SessionInfo(info) => HttpResponse::Ok().json(info),
            SessionOk::SessionJson(val) => HttpResponse::Ok().json(val),
            SessionOk::SessionAlreadyDeleted => HttpResponse::Ok().finish(),
            SessionOk::BlobAlreadyDeleted => HttpResponse::Ok().finish(),
        }
    }
}

impl Into<HttpResponse> for SessionErr {
    fn into(self) -> HttpResponse {
        match self {
            SessionErr::OverwriteError => HttpResponse::InternalServerError().body("Id conflict"),
            SessionErr::SessionNotFoundError => HttpResponse::NotFound().body("Session not found"),
            SessionErr::BlobNotFoundError => HttpResponse::NotFound().body("Blob not found"),
            SessionErr::BlobLockedError => {
                HttpResponse::build(StatusCode::from_u16(423).expect("Wrong http code - 423"))
                    .finish()
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
