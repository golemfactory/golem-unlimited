use actix::Arbiter;
use actix::System;
use actix::SystemService;
use actix_web::error::ErrorBadRequest;
use actix_web::error::ErrorInternalServerError;
use actix_web::error::ParseError::Status;
use actix_web::http;
use actix_web::http::StatusCode;
use actix_web::AsyncResponder;
use actix_web::HttpRequest;
use actix_web::HttpResponse;
use actix_web::Responder;
use actix_web::Scope;
use futures::future;
use futures::future::Future;
use plugins::manager::ListPlugins;
use plugins::manager::PluginFile;
use plugins::manager::PluginManager;
use plugins::plugin::format_plugins_table;
use plugins::plugin::PluginInfo;
use plugins::zip::PluginParser;
use server::ServerClient;
use std::path::{Path, PathBuf};
use std::str::Bytes;

pub fn list_query() {
    System::run(|| {
        Arbiter::spawn(
            ServerClient::get("/plug")
                .and_then(|r| Ok(format_plugins_table(r)))
                .map_err(|e| error!("{}", e))
                .then(|_r| Ok(System::current().stop())),
        )
    });
}

pub fn install_query(path: &Path) {
    System::run(|| {
        Arbiter::spawn(
            ServerClient::get("/plug")
                .and_then(|r: Vec<PluginInfo>| Ok(format_plugins_table(r)))
                .map_err(|e| error!("{}", e))
                .then(|_r| Ok(System::current().stop())),
        )
    });
}

pub fn scope<S: 'static>(scope: Scope<S>) -> Scope<S> {
    scope.route("", http::Method::GET, list_scope).route(
        "/{pluginName}/{fileName}",
        http::Method::GET,
        file_scope,
    )
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
}

impl ToString for ContentType {
    fn to_string(&self) -> String {
        match self {
            ContentType::JavaScript => "application/javascript".to_string(),
            ContentType::Html => "text/html".to_string(),
        }
    }
}

fn valid_path_part(part: &Path) -> bool {
    let mut pos = 0;
    let lambda = |c: char| {
        pos += 1;
        c.is_alphanumeric() || c.is_whitespace() || (c == '.' && pos != 0)
    };

    match part.to_str() {
        Some(x) => x.chars().all(lambda),
        None => false,
    }
}

fn valid_path(path: &String) -> Result<ContentType, String> {
    let buf = PathBuf::from(path);
    if buf.ancestors().all(|x| valid_path_part(x)) {
        buf.file_name()
            .and_then(|a| Path::new(a).extension())
            .and_then(|ext| ext.to_str())
            .and_then(|ext| match ext {
                "js" => Some(ContentType::JavaScript),
                "html" => Some(ContentType::Html),
                _ => None,
            }).ok_or(format!("Unsupported file extension: {:?}", buf.file_name()).to_string())
    } else {
        Err("Invalid file path".to_string())
    }
}

fn file_scope<S>(r: HttpRequest<S>) -> impl Responder {
    let manager = PluginManager::from_registry();
    let match_info = r.match_info();

    let file = PluginFile {
        plugin: match_info
            .get("pluginName")
            .expect("Can't get plugin name from query")
            .to_string(),
        path: match_info
            .get("fileName")
            .expect("Can't get filename from query")
            .to_string(),
    };

    match valid_path(&file.path) {
        Ok(content_type) => manager
            .send(file)
            .map_err(|e| ErrorInternalServerError(format!("err: {}", e)))
            .and_then(|res| res.map_err(|e| ErrorInternalServerError(format!("err: {}", e))))
            .and_then(move |res| {
                Ok(HttpResponse::Ok()
                    .content_type(content_type.to_string())
                    .body(res))
            }).responder(),
        Err(e) => future::err(ErrorBadRequest(e)).responder(),
    }
}
