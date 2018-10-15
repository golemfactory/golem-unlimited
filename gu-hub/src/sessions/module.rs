use gu_base::Module;
use actix::SystemService;
use actix_web::{App, Scope, http, HttpRequest, HttpResponse, Responder, AsyncResponder, HttpMessage};
use futures::future::Future;
use actix_web::error::{ErrorInternalServerError, ErrorBadRequest};
use sessions::responses::*;
use sessions::manager::*;
use actix::Handler;
use actix::Message;
use sessions::session::SessionInfo;
use actix_web::Error;
use serde::de::DeserializeOwned;
use serde_json::Value;
use sessions::blob::Blob;

#[derive(Default)]
pub struct SessionsModule {

}

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
        .route("/{sessionId}/config", http::Method::PUT, set_config_scope)
        .route("/{sessionId}/config", http::Method::GET, get_config_scope)
        .route("/{sessionId}/blob", http::Method::POST, create_blob_scope)
        .route("/{sessionId}/blob/{blobId}", http::Method::DELETE, delete_blob_scope)
        .route("/{sessionId}/blob/{blobId}", http::Method::POST, upload_scope)
        .route("/{sessionId}/blob/{blobId}", http::Method::GET, download_scope)
}

fn manager_request<H, M>(msg: M) -> impl Future<Item=HttpResponse, Error=Error>
    where
        H: Handler<M> + SystemService,
        M: Message<Result=SessionResponse> + Send + 'static,
{
    H::from_registry()
        .send(msg)
        .map_err(|e| ErrorInternalServerError(format!("err: {}", e)))
        .and_then(|res: SessionResponse| Ok(Into::<HttpResponse>::into(res)))
}

fn request_json<S, M>(r: HttpRequest<S>) -> impl Future<Item=M, Error=Error>
    where
        S: 'static,
        M: Send + DeserializeOwned + 'static,
{
    r.json()
        .map_err(|e| ErrorInternalServerError(format!("Cannot parse body: {}", e)))
        .and_then(|msg: M| Ok(msg))
}

fn get_param<S>(r: &HttpRequest<S>, name: &'static str) -> Result<u64, Error> {
    r.match_info()
        .get(name)
        .ok_or("Parameter not found")
        .and_then(|s| s.parse().map_err(|_| "Cannot parse found"))
        .map_err(|_| ErrorBadRequest("Cannot parse parameter"))
}

fn session_id<S>(r: &HttpRequest<S>) -> Result<u64, Error> {
    get_param(r, "sessionId")
}

fn blob_id<S>(r: &HttpRequest<S>) -> Result<u64, Error> {
    get_param(r, "blobId")
}

fn list_scope<S>(_r: HttpRequest<S>) -> impl Responder {
    manager_request::<SessionsManager, _>(ListSessions).responder()
}

fn crate_scope<S: 'static>(r: HttpRequest<S>) -> impl Responder {
    request_json(r)
        .and_then(|info: SessionInfo| manager_request::<SessionsManager, _>(CreateSession { info }))
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

fn get_config_scope<S>(r: HttpRequest<S>) -> impl Responder {
    let session = session_id(&r).map_err(|e| return e).unwrap();

    manager_request::<SessionsManager, _>(GetMetadata { session }).responder()
}

fn set_config_scope<S: 'static>(r: HttpRequest<S>) -> impl Responder {
    let session = session_id(&r).map_err(|e| return e).unwrap();

    request_json(r)
        .and_then(move |metadata: Value|
            manager_request::<SessionsManager, _>(SetMetadata { session, metadata })
        )
        .responder()
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

fn upload_scope<S: 'static>(r: HttpRequest<S>) -> impl Responder {
    let session = session_id(&r).map_err(|e| return e).unwrap();
    let blob_id = blob_id(&r).map_err(|e| return e).unwrap();

    request_json(r)
        .and_then(move |blob: Blob|
            manager_request::<SessionsManager, _>(UploadBlob { session, blob_id, blob })
        )
        .responder()
}

fn download_scope<S>(r: HttpRequest<S>) -> impl Responder {
    let session = session_id(&r).map_err(|e| return e).unwrap();
    let blob_id = blob_id(&r).map_err(|e| return e).unwrap();

    manager_request::<SessionsManager, _>(DownloadBlob { session, blob_id }).responder()
}