use actix::{Handler, MailboxError, Message, SystemService};
use actix_web::Path;
use actix_web::{
    error::{ErrorBadRequest, ErrorInternalServerError},
    http,
    http::ContentEncoding,
    http::StatusCode,
    App, AsyncResponder, Error as ActixError, HttpMessage, HttpRequest, HttpResponse, Json,
    Responder, Result as ActixResult, Scope,
};
use futures::future::Future;
use futures::stream::Stream;
use gu_actix::prelude::*;
use gu_base::Module;
use gu_model::session::HubSessionSpec;
use serde::de::DeserializeOwned;
use serde_json::Value;
use sessions::{manager, manager::SessionsManager, responses::*, session::SessionInfo};

#[derive(Default)]
pub struct SessionsModule {}

impl Module for SessionsModule {
    fn decorate_webapp<S: 'static>(&self, app: App<S>) -> App<S> {
        app.scope("/sessions", scope)
    }
}

fn scope<S: 'static>(scope: Scope<S>) -> Scope<S> {
    scope
        .resource("", |r| {
            r.name("hub-sessions");

            r.get().with_async(|()| {
                SessionsManager::from_registry()
                    .send(manager::List)
                    .flatten_fut()
                    .from_err::<actix_web::Error>()
                    .and_then(|sessions| Ok(HttpResponse::Ok().json(sessions)))
            });
            r.post().with_async_config(create_session, |(cfg,)| {
                cfg.limit(4096);
            });
        })
        .resource("/{sessionId}", |r| {
            r.get().with_async(get_session);
            r.delete().with_async(|path: Path<SessionPath>| {
                SessionsManager::from_registry()
                    .send(manager::Delete::with_session_id(path.session_id))
                    .flatten_fut()
                    .from_err::<actix_web::Error>()
                    .and_then(|()| Ok(HttpResponse::NoContent()))
            })
        })
        .resource("/{sessionId}/config", |r| {
            r.name("hub-session-config");
            r.get().with_async(get_config);
            r.put().with_async(set_config);
        })
        .resource("/{sessionId}/blobs", |r| {
            r.name("hub-session-blobs");
            r.post().with(create_blob_scope);
        })
        .resource("/{sessionId}/blobs/{blobId}", |r| {
            r.name("hub-session-blob");
            r.get().with(download_scope);
            r.put().with(upload_scope);
            r.delete().with_async(|path: Path<SessionBlobPath>| {
                let blob_id = path.blob_id;

                SessionsManager::from_registry()
                    .send(manager::Update::new(path.session_id, move |session| {
                        session.delete_blob(blob_id)
                    }))
                    .flatten_fut()
                    .map_err(|e| ErrorInternalServerError(format!("err: {}", e)))
                    .and_then(|_r| Ok(HttpResponse::build(StatusCode::NO_CONTENT).finish()))
            });
        })
}

fn get_param<S>(r: &HttpRequest<S>, name: &'static str) -> ActixResult<u64> {
    r.match_info()
        .get(name)
        .ok_or("Parameter not found")
        .and_then(|s| s.parse().map_err(|_| "Cannot parse found"))
        .map_err(|_| ErrorBadRequest("Cannot parse parameter"))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SessionPath {
    session_id: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SessionBlobPath {
    session_id: u64,
    blob_id: u64,
}

fn session_id<S>(r: &HttpRequest<S>) -> ActixResult<u64> {
    get_param(r, "sessionId")
}

fn blob_id<S>(r: &HttpRequest<S>) -> ActixResult<u64> {
    get_param(r, "blobId")
}

fn create_session(
    spec: Json<HubSessionSpec>,
) -> impl Future<Item = HttpResponse, Error = actix_web::Error> + 'static {
    let info = SessionInfo {
        name: spec.into_inner().name,
    };

    SessionsManager::from_registry()
        .send(manager::Create::from_info(info))
        .flatten_fut()
        .from_err()
        .and_then(|session_id| {
            Ok(HttpResponse::build(StatusCode::CREATED)
                .header("Location", format!("/sessions/{}", session_id))
                .json(session_id))
        })
}

fn get_session(
    path: Path<SessionPath>,
) -> impl Future<Item = HttpResponse, Error = actix_web::Error> {
    SessionsManager::from_registry()
        .send(manager::Update::new(path.session_id, |session| {
            Ok(session.info())
        }))
        .flatten_fut()
        .from_err()
        .and_then(|session_details| Ok(HttpResponse::Ok().json(session_details)))
}

fn get_config(
    path: Path<SessionPath>,
) -> impl Future<Item = HttpResponse, Error = actix_web::Error> {
    SessionsManager::from_registry()
        .send(manager::Update::new(path.session_id, |session| {
            Ok(session.metadata().clone())
        }))
        .flatten_fut()
        .from_err()
        .and_then(|metadata| Ok(HttpResponse::Ok().json(metadata)))
}

fn set_config(
    (path, body): (Path<SessionPath>, Json<gu_model::session::Metadata>),
) -> impl Future<Item = HttpResponse, Error = actix_web::Error> {
    let new_metadata = body.into_inner();
    SessionsManager::from_registry()
        .send(manager::Update::new(path.session_id, |session| {
            session.set_metadata(new_metadata)
        }))
        .flatten_fut()
        .from_err()
        .and_then(|new_version| Ok(HttpResponse::Ok().json(new_version)))
}

fn create_blob_scope<S: 'static>(r: HttpRequest<S>) -> impl Responder {
    let session = session_id(&r).map_err(|e| return e).unwrap();

    let session_manager = SessionsManager::from_registry();

    if r.content_type() == "multipart/form-data" {
        r.multipart()
            .map_err(|e| ErrorInternalServerError(format!("err: {}", e)))
            .fold(Vec::new(), move |mut blobs, part| {
                session_manager
                    .send(manager::CreateBlob { session })
                    .flatten_fut()
                    .map_err(|e| ErrorInternalServerError(format!("err: {}", e)))
                    .and_then(|(blob_id, blob)| {
                        use actix_web::multipart::MultipartItem;

                        match part {
                            MultipartItem::Field(payload) => futures::future::Either::B(
                                blob.write(payload)
                                    .map_err(|e| ErrorInternalServerError(format!("err: {}", e)))
                                    .and_then(move |_| {
                                        blobs.push(blob_id);
                                        Ok(blobs)
                                    }),
                            ),
                            _ => futures::future::Either::A(futures::future::ok(blobs)),
                        }
                    })
            })
            .map_err(|e| ErrorInternalServerError(format!("err: {}", e)))
            .and_then(move |blobs| Ok(HttpResponse::Ok().json(blobs)))
            .responder()
    } else {
        session_manager
            .send(manager::CreateBlob { session })
            .flatten_fut()
            .map_err(|e| ErrorInternalServerError(format!("err: {}", e)))
            .and_then(|(blob_id, _blob)| Ok(HttpResponse::Ok().json(blob_id)))
            .responder()
    }
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

    let blob_fut = manager
        .send(manager::GetBlob { session, blob_id })
        .flatten_fut();
    let res_fut = blob_fut
        .and_then(move |res: SessionOk| match res {
            SessionOk::Blob(blob) => blob.write(r.payload()),
            _ => unreachable!(),
        })
        .and_then(|result| Ok(Into::<HttpResponse>::into(result)));

    session_future_responder(res_fut)
}

fn download_scope<S: 'static>(r: HttpRequest<S>) -> impl Responder {
    use actix_web::http::header::ETAG;

    let session = session_id(&r).map_err(|e| return e).unwrap();
    let blob_id = blob_id(&r).map_err(|e| return e).unwrap();
    let manager = SessionsManager::from_registry();

    let blob_fut = manager
        .send(manager::GetBlob { session, blob_id })
        .flatten_fut();
    let res_fut = blob_fut
        .and_then(move |res: SessionOk| match res {
            SessionOk::Blob(blob) => blob.read(),
            _oth => unreachable!(),
        })
        .and_then(move |(n, sha)| {
            n.respond_to(&r)
                .and_then(|mut r| {
                    r.headers_mut().insert(ETAG, sha);
                    Ok(r)
                })
                .map_err(|e| SessionErr::FileError(e.to_string()))
        });

    session_future_responder(res_fut)
}
