use std::{
    fs::File,
    io::{Cursor, Read},
    path::{Path, PathBuf},
};

use actix::{Arbiter, System, SystemService};
use actix_web::{
    error::{ErrorBadRequest, ErrorInternalServerError},
    http, AsyncResponder, HttpMessage, HttpRequest, HttpResponse, Responder, Scope,
};
use bytes::{buf::IntoBuf, Bytes};
use futures::{
    future::{self, Future},
    stream::Stream,
};
use log::{debug, error};

use crate::server::HubClient as ServerClient;

use super::{
    manager::{
        ChangePluginState, InstallDevPlugin, InstallPlugin, ListPlugins, PluginFile, PluginManager,
        QueriedStatus,
    },
    plugin::{format_plugins_table, PluginInfo},
    rest_result::{InstallQueryResult, RestResponse, ToHttpResponse},
};

pub fn list_query() {
    System::run(|| {
        Arbiter::spawn(
            ServerClient::get("/plug")
                .and_then(|r: Vec<PluginInfo>| Ok(format_plugins_table(r)))
                .map_err(|e| error!("{}", e))
                .then(|_r| Ok(System::current().stop())),
        )
    });
}

pub fn read_file(path: &Path) -> Result<Vec<u8>, ()> {
    File::open(path)
        .map_err(|e| {
            error!("Cannot open {:?} file", path.clone());
            debug!("Error details: {:?}", e)
        })
        .and_then(|mut file| {
            let mut buf = Vec::new();
            file.read_to_end(&mut buf).map(|_| buf).map_err(|e| {
                error!("Cannot read {:?} file", path.clone());
                debug!("Error details: {:?}", e)
            })
        })
}

pub fn install_query_inner(buf: Vec<u8>) -> impl Future<Item = (), Error = ()> {
    ServerClient::post("/plug", buf)
        .and_then(|r: RestResponse<InstallQueryResult>| Ok(debug!("{}", r.message.message())))
        .map_err(|e| {
            error!("Error on server connection");
            debug!("Error details: {:?}", e)
        })
        .then(|_r| Ok(System::current().stop()))
}

pub fn install_query(path: PathBuf) {
    System::run(move || {
        Arbiter::spawn(
            future::result(read_file(&path))
                .and_then(|buf| install_query_inner(buf))
                .then(|_r| Ok(System::current().stop())),
        )
    });
}

pub fn uninstall_query(plugin: String) {
    System::run(move || {
        Arbiter::spawn(
            ServerClient::delete(format!("/plug/{}", plugin))
                .and_then(|_r: ()| Ok(()))
                .map_err(|e| error!("{}", e))
                .then(|_r| Ok(System::current().stop())),
        )
    });
}

pub fn status_query(plugin: String, status: QueriedStatus) {
    System::run(move || {
        Arbiter::spawn(
            ServerClient::patch(format!("/plug/{}/{}", plugin, status))
                .and_then(|_r: ()| Ok(()))
                .map_err(|e| error!("{}", e))
                .then(|_r| Ok(System::current().stop())),
        )
    });
}

pub fn dev_query(path: PathBuf) {
    let path = path
        .canonicalize()
        .expect("Cannot canonicalize dir path")
        .to_str()
        .expect("Cannot parse filepath to str")
        .to_string();

    System::run(move || {
        Arbiter::spawn(
            ServerClient::empty_post(format!("/plug/dev{}", path))
                .and_then(|r: RestResponse<InstallQueryResult>| {
                    Ok(debug!("{}", r.message.message()))
                })
                .map_err(|e| error!("{}", e))
                .then(|_r| Ok(System::current().stop())),
        )
    });
}

pub fn scope<S: 'static>(scope: Scope<S>) -> Scope<S> {
    scope
        .route("", http::Method::GET, list_scope)
        .route("", http::Method::POST, install_scope)
        .route("/dev/{pluginPath:.*}", http::Method::POST, dev_scope)
        .route("/{pluginName}", http::Method::DELETE, |r| {
            state_scope(QueriedStatus::Uninstall, r)
        })
        .route("/{pluginName}/activate", http::Method::PATCH, |r| {
            state_scope(QueriedStatus::Activate, r)
        })
        .route(
            "/{pluginName}/inactivate/inactivate",
            http::Method::PATCH,
            |r| state_scope(QueriedStatus::Inactivate, r),
        )
        .route("/{pluginName}/{fileName:.*}", http::Method::GET, file_scope)
}

fn list_scope<S>(_r: HttpRequest<S>) -> impl Responder {
    use actix_web::AsyncResponder;
    let manager = PluginManager::from_registry();

    manager
        .send(ListPlugins)
        .map_err(|e| ErrorInternalServerError(format!("err: {}", e)))
        .and_then(|res| Ok(HttpResponse::Ok().json(res)))
        .responder()
}

enum ContentType {
    JavaScript,
    Html,
    Svg,
    Wasm,
    NotSupported,
}

impl<'a> From<&'a str> for ContentType {
    fn from(s: &'a str) -> Self {
        match s {
            "js" => ContentType::JavaScript,
            "html" => ContentType::Html,
            "svg" => ContentType::Svg,
            "wasm" => ContentType::Wasm,
            _ => ContentType::NotSupported,
        }
    }
}

impl ToString for ContentType {
    fn to_string(&self) -> String {
        match self {
            ContentType::JavaScript => "application/javascript".to_string(),
            ContentType::Html => "text/html".to_string(),
            ContentType::Svg => "image/svg+xml".to_string(),
            ContentType::Wasm => "application/wasm".to_string(),
            ContentType::NotSupported => "Content type not supported".to_string(),
        }
    }
}

fn file_scope<S>(r: HttpRequest<S>) -> impl Responder {
    let manager = PluginManager::from_registry();
    let match_info = r.match_info();

    let path = PathBuf::from(
        match_info
            .get("fileName")
            .expect("Can't get filename from query"),
    );

    let plugin = match_info
        .get("pluginName")
        .expect("Can't get plugin name from query")
        .to_string();

    let b = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|a| ContentType::from(a));

    match b {
        None => future::err(ErrorBadRequest("Cannot parse file extension")).responder(),
        Some(ContentType::NotSupported) => {
            future::err(ErrorBadRequest(ContentType::NotSupported.to_string())).responder()
        }
        Some(content) => manager
            .send(PluginFile { plugin, path })
            .map_err(|e| ErrorInternalServerError(format!("err: {}", e)))
            .and_then(|res| res.map_err(|e| ErrorInternalServerError(format!("err: {}", e))))
            .and_then(move |res| {
                Ok(HttpResponse::Ok()
                    .content_type(content.to_string())
                    .body(res))
            })
            .responder(),
    }
}

fn install_scope<S>(r: HttpRequest<S>) -> impl Responder {
    let manager = PluginManager::from_registry();

    r.payload()
        .map_err(|e| ErrorBadRequest(format!("Couldn't get request body: {:?}", e)))
        .concat2()
        .and_then(|a| Ok(a.into_buf()))
        .and_then(move |a: Cursor<Bytes>| {
            manager
                .send(InstallPlugin { bytes: a })
                .map_err(|e| ErrorInternalServerError(format!("{:?}", e)))
        })
        .and_then(|result| Ok(result.to_http_response()))
        .responder()
}

fn state_scope<S>(state: QueriedStatus, r: HttpRequest<S>) -> impl Responder {
    let manager = PluginManager::from_registry();
    let match_info = r.match_info();

    let plugin = match_info
        .get("pluginName")
        .expect("Can't get plugin name from query")
        .to_string();

    manager
        .send(ChangePluginState { plugin, state })
        .and_then(move |_res| {
            Ok(HttpResponse::Ok()
                .content_type("application/json")
                .body("null"))
        })
        .responder()
}

fn dev_scope<S>(r: HttpRequest<S>) -> impl Responder {
    let manager = PluginManager::from_registry();
    let match_info = r.match_info();

    let path = PathBuf::from(format!(
        "/{}",
        match_info
            .get("pluginPath")
            .expect("Can't get plugin name from query")
    ));

    manager
        .send(InstallDevPlugin { path })
        .and_then(|result| Ok(result.to_http_response()))
        .responder()
}
