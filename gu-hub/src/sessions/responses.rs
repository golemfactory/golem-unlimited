use serde_json::Value;
use sessions::blob::Blob;
use sessions::session::SessionInfo;
use actix_web::HttpResponse;


pub enum SessionResponse {
    Ok,
    SessionsList(Vec<SessionInfo>),
    SessionId(u64),
    BlobId(u64),
    Blob(Blob),
    SessionInfo(SessionInfo),
    SessionJson(Value),
    SessionAlreadyDeleted,
    BlobAlreadyDeleted,

    OverwriteError,
    SessionNotFoundError,
    BlobNotFoundError,
}

impl Into<HttpResponse> for SessionResponse {
    fn into(self) -> HttpResponse {
        match self {
            SessionResponse::Ok => HttpResponse::Ok().finish(),
            SessionResponse::SessionsList(list) => HttpResponse::Ok().json(list),
            SessionResponse::SessionId(id) => HttpResponse::Ok().json(id),
            SessionResponse::BlobId(id) => HttpResponse::Ok().json(id),
            SessionResponse::Blob(_blob) => HttpResponse::Ok().finish(),
            SessionResponse::SessionInfo(info) => HttpResponse::Ok().json(info),
            SessionResponse::SessionJson(val) => HttpResponse::Ok().json(val),
            SessionResponse::SessionAlreadyDeleted => HttpResponse::Ok().finish(),
            SessionResponse::BlobAlreadyDeleted => HttpResponse::Ok().finish(),
            SessionResponse::OverwriteError => HttpResponse::InternalServerError().body("Id conflict"),
            SessionResponse::SessionNotFoundError => HttpResponse::NotFound().body("Session not found"),
            SessionResponse::BlobNotFoundError => HttpResponse::NotFound().body("Blob not found"),
        }
    }
}