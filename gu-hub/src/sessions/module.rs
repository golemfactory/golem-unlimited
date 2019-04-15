use actix::{Handler, MailboxError, Message, SystemService};
use actix_web::Path;
use actix_web::{
    error::{ErrorBadRequest, ErrorInternalServerError},
    fs::NamedFile,
    http,
    http::{ContentEncoding, Method, StatusCode},
    App, AsyncResponder, Error as ActixError, HttpMessage, HttpRequest, HttpResponse, Json,
    Responder, Result as ActixResult, Scope,
};
use futures::future::Future;
use futures::stream::Stream;
use gu_actix::prelude::*;
use gu_base::Module;
use gu_model::session::HubSessionSpec;
use gu_net::NodeId;
use serde::de::DeserializeOwned;
use serde_json::Value;
use sessions::{manager, manager::SessionsManager, responses::*, session::SessionInfo};
use std::path::PathBuf;

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
                    .and_then(|sessions| {
                        Ok(HttpResponse::Ok().json(
                            sessions
                                .into_iter()
                                .map(|(session_id, session_info)| {
                                    gu_model::session::SessionDetails {
                                        id: session_id,
                                        created: Some(session_info.created),
                                        name: session_info.name,
                                        tags: session_info.tags.unwrap_or_default(),
                                        ..gu_model::session::SessionDetails::default()
                                    }
                                })
                                .collect::<Vec<gu_model::session::SessionDetails>>(),
                        ))
                    })
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
            r.get().with_async(list_blobs);
        })
        .resource("/{sessionId}/blobs/{blobId}", |r| {
            r.name("hub-session-blob");
            r.get().with(download_scope);
            /* r.get().with_async(download_blob); */
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
        .resource("/{sessionId}/peers", |r| {
            r.name("hub-session-peers");
            r.get().with_async(list_peers);
            r.post().with_async(add_peers);
        })
        .resource("/{sessionId}/peers/{nodeId}/deployments", |r| {
            r.name("hub-session-peers-deployments");
            r.post().with_async(create_deployment);
        })
        .resource(
            "/{sessionId}/peers/{nodeId}/deployments/{deploymentId}",
            |r| {
                r.name("hub-session-peers-deployment");
                r.delete().with_async(delete_deployment);
                r.method(Method::PATCH).with_async(update_deployment);
            },
        )
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SessionPeerPath {
    session_id: u64,
    node_id: NodeId,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SessionPeerDeploymentPath {
    session_id: u64,
    node_id: NodeId,
    deployment_id: String,
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
    let spec_inner = spec.into_inner();
    let info = SessionInfo {
        name: spec_inner.name,
        created: chrono::Utc::now(),
        expire: spec_inner.expires,
        tags: Some(spec_inner.tags),
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
            .and_then(|(blob_id, _blob)| Ok(HttpResponse::Created().json(blob_id)))
            .responder()
    }
}

fn list_blobs(
    path: Path<SessionPath>,
) -> impl Future<Item = HttpResponse, Error = actix_web::Error> {
    SessionsManager::from_registry()
        .send(manager::Update::new(path.session_id, |session| {
            Ok(session.list_blobs())
        }))
        .flatten_fut()
        .from_err()
        .and_then(|list| Ok(HttpResponse::Ok().json(list)))
}

fn list_peers(
    path: Path<SessionPath>,
) -> impl Future<Item = HttpResponse, Error = actix_web::Error> {
    SessionsManager::from_registry()
        .send(manager::Update::new(path.session_id, |session| {
            Ok(session.list_peers())
        }))
        .flatten_fut()
        .from_err()
        .and_then(|list| Ok(HttpResponse::Ok().json(list)))
}

fn add_peers(
    (path, body): (Path<SessionPath>, Json<Vec<NodeId>>),
) -> impl Future<Item = HttpResponse, Error = actix_web::Error> {
    SessionsManager::from_registry()
        .send(manager::Update::new(path.session_id, |session| {
            Ok(session.add_peers(body.into_inner()))
        }))
        .flatten_fut()
        .from_err()
        .and_then(|all_peers| Ok(HttpResponse::Ok().json(all_peers)))
}

fn delete_deployment(
    path: Path<SessionPeerDeploymentPath>,
) -> impl Future<Item = HttpResponse, Error = actix_web::Error> {
    SessionsManager::from_registry()
        .send(manager::DeleteDeployment::new(
            path.session_id,
            path.node_id,
            path.deployment_id.clone(),
        ))
        .flatten_fut()
        .from_err()
        .and_then(|result| Ok(HttpResponse::NoContent().json(result)))
}

fn create_deployment(
    (path, body): (
        Path<SessionPeerPath>,
        Json<gu_model::envman::GenericCreateSession>,
    ),
) -> impl Future<Item = HttpResponse, Error = actix_web::Error> {
    let node_id = path.node_id;
    SessionsManager::from_registry()
        .send(manager::CreateDeployment::new(
            path.session_id,
            path.node_id,
            body.into_inner(),
        ))
        .flatten_fut()
        .from_err()
        .and_then(move |peer_session_id| {
            Ok(HttpResponse::Created()
                .header(
                    "Location",
                    format!(
                        "/sessions/{}/peers/{}/deployments/{}",
                        path.session_id,
                        node_id.to_string(),
                        peer_session_id,
                    ),
                )
                .json(peer_session_id))
        })
}

fn update_deployment(
    (path, body): (
        Path<SessionPeerDeploymentPath>,
        Json<Vec<gu_model::envman::Command>>,
    ),
) -> impl Future<Item = HttpResponse, Error = actix_web::Error> {
    SessionsManager::from_registry()
        .send(manager::UpdateDeployment::new(
            path.session_id,
            path.node_id,
            path.deployment_id.clone(),
            body.into_inner(),
        ))
        .flatten_fut()
        .from_err()
        .and_then(|results| match results {
            Ok(results) => Ok(HttpResponse::Ok().json(results)),
            Err(results) => Ok(HttpResponse::InternalServerError()
                .header("x-processing-error", "1")
                .json(results)),
        })
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
        .and_then(|_| Ok(HttpResponse::build(StatusCode::NO_CONTENT).finish()));

    session_future_responder(res_fut)
}

/*
fn download_blob(
    path: Path<SessionBlobPath>,
) -> impl Future<Item = NamedFile, Error = actix_web::Error> {
    SessionsManager::from_registry()
        .send(manager::Update::new(path.session_id, move |session| {
            session.get_blob_path(path.blob_id).map(|r| r.to_owned())
        }))
        .flatten_fut()
        .from_err()
        .and_then(|path| {
            NamedFile::open(path)
                .map(|f| f.set_content_encoding(actix_web::http::ContentEncoding::Identity))
                .map_err(|_| ErrorInternalServerError("File Error".to_string()))
        })
}
*/

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
                    r.set_content_encoding(actix_web::http::ContentEncoding::Identity);
                    Ok(r)
                })
                .map_err(|e| SessionErr::FileError(e.to_string()))
        });

    session_future_responder(res_fut)
}
