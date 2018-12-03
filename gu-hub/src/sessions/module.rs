use actix::{Handler, MailboxError, Message, SystemService};
use actix_web::{
    error::{ErrorBadRequest, ErrorInternalServerError},
    http, App, AsyncResponder, Error as ActixError, HttpMessage, HttpRequest, HttpResponse,
    Responder, Result as ActixResult, Scope,
};
use futures::future::Future;
use gu_actix::prelude::*;
use gu_base::Module;
use serde::de::DeserializeOwned;
use serde_json::Value;
use sessions::{manager::*, responses::*, session::SessionInfo};

#[derive(Default)]
pub struct SessionsModule {}

impl Module for SessionsModule {
    fn decorate_webapp<S: 'static>(&self, app: App<S>) -> App<S> {
        app.scope("/sessions", scope)
    }
}

fn scope<S: 'static>(scope: Scope<S>) -> Scope<S> {
    scope
        .route("", http::Method::GET, list_scope)
        .route("", http::Method::POST, crate_scope)
        .route("/{sessionId}", http::Method::GET, info_scope)
        .route("/{sessionId}", http::Method::DELETE, delete_scope)
        .route("/{sessionId}/config", http::Method::PUT, set_config)
        .route("/{sessionId}/config", http::Method::GET, get_config)
        .route("/{sessionId}/blob", http::Method::POST, create_blob_scope)
        .route(
            "/{sessionId}/blob/{blobId}",
            http::Method::DELETE,
            delete_blob_scope,
        ).route(
            "/{sessionId}/blob/{blobId}",
            http::Method::PUT,
            upload_scope,
        ).route(
            "/{sessionId}/blob/{blobId}",
            http::Method::GET,
            download_scope,
        )
}

fn manager_request<H, M>(msg: M) -> impl Future<Item = HttpResponse, Error = ActixError>
where
    H: Handler<M> + SystemService,
    M: Message<Result = SessionResult> + Send + 'static,
{
    H::from_registry()
        .send(msg)
        .map_err(|e| ErrorInternalServerError(format!("err: {}", e)))
        .and_then(|res: SessionResult| Ok(to_response(res)))
}

fn request_json<S, M>(r: HttpRequest<S>) -> impl Future<Item = M, Error = ActixError>
where
    S: 'static,
    M: Send + DeserializeOwned + 'static,
{
    r.json()
        .map_err(|e| ErrorInternalServerError(format!("Cannot parse body: {}", e)))
        .and_then(|msg: M| Ok(msg))
}

fn get_param<S>(r: &HttpRequest<S>, name: &'static str) -> ActixResult<u64> {
    r.match_info()
        .get(name)
        .ok_or("Parameter not found")
        .and_then(|s| s.parse().map_err(|_| "Cannot parse found"))
        .map_err(|_| ErrorBadRequest("Cannot parse parameter"))
}

fn session_id<S>(r: &HttpRequest<S>) -> ActixResult<u64> {
    get_param(r, "sessionId")
}

fn blob_id<S>(r: &HttpRequest<S>) -> ActixResult<u64> {
    get_param(r, "blobId")
}

fn list_scope<S>(_r: HttpRequest<S>) -> impl Responder {
    manager_request::<SessionsManager, _>(ListSessions).responder()
}

fn crate_scope<S: 'static>(r: HttpRequest<S>) -> impl Responder {
    use actix_web::{http::ContentEncoding, HttpRequest, HttpResponse, Json};

    request_json(r)
        .and_then(|info: SessionInfo| {
            SessionsManager::from_registry()
                .send(CreateSession { info })
                .flatten_fut()
                .from_err()
        }).and_then(|v| Ok(HttpResponse::Ok().json(v)))
        .responder()
}

fn info_scope<S>(r: HttpRequest<S>) -> impl Responder {
    let session = session_id(&r).map_err(|e| return e).unwrap();

    manager_request::<SessionsManager, _>(GetSessionInfo { session }).responder()
}

fn delete_scope<S>(r: HttpRequest<S>) -> impl Responder {
    let session = session_id(&r).map_err(|e| return e).unwrap();

    manager_request::<SessionsManager, _>(DeleteSession { session }).responder()
}

fn get_config<S>(r: HttpRequest<S>) -> impl Responder {
    let session = session_id(&r).map_err(|e| return e).unwrap();

    manager_request::<SessionsManager, _>(GetMetadata { session }).responder()
}

fn set_config<S: 'static>(r: HttpRequest<S>) -> impl Responder {
    let session = session_id(&r).map_err(|e| return e).unwrap();

    request_json(r)
        .and_then(move |metadata: Value| {
            manager_request::<SessionsManager, _>(SetMetadata { session, metadata })
        }).responder()
}

fn create_blob_scope<S>(r: HttpRequest<S>) -> impl Responder {
    let session = session_id(&r).map_err(|e| return e).unwrap();

    manager_request::<SessionsManager, _>(CreateBlob { session }).responder()
}

fn delete_blob_scope<S>(r: HttpRequest<S>) -> impl Responder {
    let session = session_id(&r).map_err(|e| return e).unwrap();
    let blob_id = blob_id(&r).map_err(|e| return e).unwrap();

    manager_request::<SessionsManager, _>(DeleteBlob { session, blob_id }).responder()
}

fn flatten<F>(fut: F) -> impl Future<Item = SessionOk, Error = SessionErr>
where
    F: Future<Item = SessionResult, Error = MailboxError>,
{
    fut.map_err(|e| SessionErr::MailboxError(e.to_string()))
        .and_then(|res: SessionResult| res)
}

fn session_future_responder<F, E, R>(fut: F) -> impl Responder
where
    F: Future<Item = R, Error = E> + 'static,
    E: Into<ActixError> + 'static,
    R: Responder + 'static,
{
    fut.map_err(|err| Into::<ActixError>::into(err)).responder()
}

fn upload_scope<S: 'static>(r: HttpRequest<S>) -> impl Responder {
    let session = session_id(&r).map_err(|e| return e).unwrap();
    let blob_id = blob_id(&r).map_err(|e| return e).unwrap();
    let manager = SessionsManager::from_registry();

    let blob_fut = flatten(manager.send(GetBlob { session, blob_id }));
    let res_fut = blob_fut
        .and_then(move |res: SessionOk| match res {
            SessionOk::Blob(blob) => blob.write(r.payload()),
            _ => unreachable!(),
        }).and_then(|result| Ok(Into::<HttpResponse>::into(result)));

    session_future_responder(res_fut)
}

fn download_scope<S: 'static>(r: HttpRequest<S>) -> impl Responder {
    use actix_web::http::header::ETAG;

    let session = session_id(&r).map_err(|e| return e).unwrap();
    let blob_id = blob_id(&r).map_err(|e| return e).unwrap();
    let manager = SessionsManager::from_registry();

    let blob_fut = flatten(manager.send(GetBlob { session, blob_id }));
    let res_fut = blob_fut
        .and_then(move |res: SessionOk| match res {
            SessionOk::Blob(blob) => blob.read(),
            _oth => unreachable!(),
        }).and_then(move |(n, sha)| {
            n.respond_to(&r)
                .and_then(|mut r| {
                    r.headers_mut().insert(ETAG, sha);
                    Ok(r)
                }).map_err(|e| SessionErr::FileError(e.to_string()))
        });

    session_future_responder(res_fut)
}
