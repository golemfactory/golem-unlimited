#![allow(dead_code)]

use actix::fut;
use actix::prelude::*;
use futures::prelude::*;

use gu_persist::config;

use actix_web;
use clap::{App, ArgMatches, SubCommand};
use gu_actix::*;
use std::borrow::Cow;
use std::net::ToSocketAddrs;
use std::sync::Arc;

use actix_web::client;
use actix_web::client::ClientRequest;
use actix_web::error::JsonPayloadError;
use actix_web::http;
use actix_web::Body;
use bytes::Bytes;
use futures::future;
use gu_base::{Decorator, Module};
use gu_p2p::rpc;
use gu_p2p::rpc::mock;
use gu_p2p::NodeId;
use gu_persist::config::ConfigManager;
use mdns::Responder;
use mdns::Service;
use serde::de;
use serde::Serialize;
use serde_json;
use std::marker::PhantomData;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ServerConfig {
    #[serde(default = "ServerConfig::default_p2p_port")]
    pub(crate) p2p_port: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    control_socket: Option<String>,
    #[serde(default = "ServerConfig::publish_service")]
    pub(crate) publish_service: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        ServerConfig {
            p2p_port: Self::default_p2p_port(),
            control_socket: None,
            publish_service: Self::publish_service(),
        }
    }
}

impl ServerConfig {
    fn default_p2p_port() -> u16 {
        61622
    }

    fn publish_service() -> bool {
        true
    }

    fn discover_service() -> bool {
        true
    }

    pub fn p2p_addr(&self) -> impl ToSocketAddrs {
        ("0.0.0.0", self.p2p_port)
    }

    pub fn port(&self) -> u16 {
        self.p2p_port
    }
}

impl config::HasSectionId for ServerConfig {
    const SECTION_ID: &'static str = "server-cfg";
}

pub struct ServerModule {
    active: bool,
    config_path: Option<String>,
}

impl ServerModule {
    pub fn new() -> Self {
        ServerModule {
            active: false,
            config_path: None,
        }
    }
}

impl Module for ServerModule {
    fn args_declare<'a, 'b>(&self, app: App<'a, 'b>) -> App<'a, 'b> {
        app.subcommand(SubCommand::with_name("server").about("Hub server management"))
    }

    fn args_consume(&mut self, matches: &ArgMatches) -> bool {
        let config_path = match matches.value_of("config-dir") {
            Some(v) => Some(v.to_string()),
            None => None,
        };

        if let Some(_m) = matches.subcommand_matches("server") {
            self.active = true;
            self.config_path = config_path.to_owned();
            return true;
        }
        false
    }

    fn run<D: Decorator + 'static + Sync + Send>(&self, decorator: D) {
        if !self.active {
            return;
        }

        let sys = actix::System::new("gu-hub");

        let _config = ServerConfigurer::new(decorator, self.config_path.clone()).start();

        let _ = sys.run();
    }
}

fn p2p_server<S>(_r: &actix_web::HttpRequest<S>) -> &'static str {
    "ok"
}

fn mdns_publisher(port: u16) -> Service {
    let responder = Responder::new().expect("Failed to run mDNS publisher");

    responder.register(
        "_unlimited._tcp".to_owned(),
        "gu-hub".to_owned(),
        port,
        &["path=/", ""],
    )
}

fn chat_route(
    req: &actix_web::HttpRequest<NodeId>,
) -> Result<actix_web::HttpResponse, actix_web::Error> {
    rpc::ws::route(req, req.state().clone())
}

pub(crate) struct ServerConfigurer<D: Decorator> {
    decorator: D,
    path: Option<String>,
}

impl<D: Decorator + 'static + Sync + Send> ServerConfigurer<D> {
    fn new(decorator: D, path: Option<String>) -> Self {
        Self { decorator, path }
    }

    pub fn config(&self) -> Addr<ConfigManager> {
        let config = config::ConfigManager::from_registry();
        println!("path={:?}", &self.path);

        if let Some(path) = &self.path {
            config.do_send(config::SetConfigPath::FsPath(Cow::Owned(path.clone())));
        }
        config
    }

    fn hub_configuration(&mut self, c: Arc<ServerConfig>, node_id: NodeId) -> Result<(), ()> {
        let decorator = self.decorator.clone();
        let server = actix_web::server::new(move || {
            decorator.decorate_webapp(
                actix_web::App::with_state(node_id.clone())
                    .handler(
                        "/app",
                        actix_web::fs::StaticFiles::new("webapp")
                            .expect("cannot provide static files"),
                    ).scope("/m", mock::scope)
                    .resource("/ws/", |r| r.route().f(chat_route)),
            )
        });
        let _ = server.bind(c.p2p_addr()).unwrap().start();

        if c.publish_service {
            Box::leak(Box::new(mdns_publisher(c.p2p_port)));
        }

        Ok(())
    }
}

impl<D: Decorator + 'static> Actor for ServerConfigurer<D> {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut <Self as Actor>::Context) {
        use rand::*;

        // TODO: use gu-ethkey
        let node_id: NodeId = thread_rng().gen();

        ctx.spawn(
            self.config()
                .send(config::GetConfig::new())
                .flatten_fut()
                .map_err(|e| println!("error ! {}", e))
                .into_actor(self)
                .and_then(move |config, act, ctx| {
                    let _ = act
                        .hub_configuration(config, node_id)
                        .map_err(|e| error!("Hub configuration error {:?}", e));
                    fut::ok(ctx.stop())
                }),
        );
    }
}

impl<D: Decorator> Drop for ServerConfigurer<D> {
    fn drop(&mut self) {
        info!("hub server configured")
    }
}

#[derive(Fail, Debug)]
pub enum ClientError {
    #[fail(display = "MailboxError {}", _0)]
    Mailbox(#[cause] MailboxError),
    #[fail(display = "ActixError {}", _0)]
    ActixError(actix_web::Error),
    #[fail(display = "{}", _0)]
    SendRequestError(#[cause] actix_web::client::SendRequestError),
    #[fail(display = "{}", _0)]
    Json(#[cause] JsonPayloadError),
    #[fail(display = "{}", _0)]
    SerdeJson(#[cause] serde_json::Error),
    #[fail(display = "config")]
    ConfigError,
}

impl From<actix_web::Error> for ClientError {
    fn from(e: actix_web::Error) -> Self {
        ClientError::ActixError(e)
    }
}

impl From<MailboxError> for ClientError {
    fn from(e: MailboxError) -> Self {
        ClientError::Mailbox(e)
    }
}

#[derive(Default)]
pub struct ServerClient {
    inner: (),
}

impl ServerClient {
    pub fn new() -> Self {
        ServerClient { inner: () }
    }

    pub fn get<T: de::DeserializeOwned + Send + 'static, IntoStr: Into<String>>(
        path: IntoStr,
    ) -> impl Future<Item = T, Error = ClientError> {
        ServerClient::from_registry()
            .send(ResourceGet::new(path.into()))
            .flatten_fut()
    }

    pub fn delete<T: de::DeserializeOwned + Send + 'static, IntoStr: Into<String>>(
        path: IntoStr,
    ) -> impl Future<Item = T, Error = ClientError> {
        ServerClient::from_registry()
            .send(ResourceDelete::new(path.into()))
            .flatten_fut()
    }

    pub fn patch<T: de::DeserializeOwned + Send + 'static, IntoStr: Into<String>>(
        path: IntoStr,
    ) -> impl Future<Item = T, Error = ClientError> {
        ServerClient::from_registry()
            .send(ResourcePatch::new(path.into()))
            .flatten_fut()
    }

    pub fn post<
        T: de::DeserializeOwned + Send + 'static,
        IntoStr: Into<String>,
        IntoBody: Into<Bytes>,
    >(
        path: IntoStr,
        body: IntoBody,
    ) -> impl Future<Item = T, Error = ClientError> {
        ServerClient::from_registry()
            .send(ResourcePost::new(path.into(), body.into()))
            .flatten_fut()
    }

    pub fn post_json<
        T: de::DeserializeOwned + Send + 'static,
        IntoStr: Into<String>,
        Ser: Serialize,
    >(
        path: IntoStr,
        body: Ser,
    ) -> impl Future<Item = T, Error = ClientError> {
        future::result(serde_json::to_string(&body))
            .map_err(|e| ClientError::SerdeJson(e))
            .and_then(|body| Self::post::<T, _, _>(path, body))
    }

    pub fn empty_post<T: de::DeserializeOwned + Send + 'static, IntoStr: Into<String>>(
        path: IntoStr,
    ) -> impl Future<Item = T, Error = ClientError> {
        ServerClient::from_registry()
            .send(ResourcePost::new(path.into(), "null".into()))
            .flatten_fut()
    }
}

impl Actor for ServerClient {
    type Context = Context<Self>;
}

impl Supervised for ServerClient {}
impl ArbiterService for ServerClient {}

pub trait IntoRequest {
    fn into_request(self, url: &str) -> Result<ClientRequest, actix_web::Error>;

    fn path(&self) -> &str;
}

struct ResourceGet<T>(String, PhantomData<T>);

impl<T> ResourceGet<T> {
    fn new(path: String) -> Self {
        ResourceGet::<T>(path, PhantomData)
    }
}

impl<T> IntoRequest for ResourceGet<T> {
    fn into_request(self, url: &str) -> Result<ClientRequest, actix_web::Error> {
        client::ClientRequest::get(url)
            .header("Accept", "application/json")
            .finish()
    }

    fn path(&self) -> &str {
        self.0.as_ref()
    }
}

struct ResourceDelete<T>(String, PhantomData<T>);

impl<T> ResourceDelete<T> {
    fn new(path: String) -> Self {
        ResourceDelete(path, PhantomData)
    }
}

impl<T> IntoRequest for ResourceDelete<T> {
    fn into_request(self, url: &str) -> Result<ClientRequest, actix_web::Error> {
        client::ClientRequest::delete(url)
            .header("Accept", "application/json")
            .finish()
    }

    fn path(&self) -> &str {
        self.0.as_ref()
    }
}

struct ResourcePatch<T>(String, PhantomData<T>);

impl<T> ResourcePatch<T> {
    fn new(path: String) -> Self {
        ResourcePatch(path, PhantomData)
    }
}

impl<T> IntoRequest for ResourcePatch<T> {
    fn into_request(self, url: &str) -> Result<ClientRequest, actix_web::Error> {
        let mut builder = ClientRequest::build();
        builder.method(http::Method::PATCH).uri(url);
        builder.header("Accept", "application/json").finish()
    }

    fn path(&self) -> &str {
        self.0.as_ref()
    }
}

struct ResourcePost<T>(String, Bytes, PhantomData<T>);

impl<T> ResourcePost<T> {
    fn new(path: String, body: Bytes) -> Self {
        ResourcePost(path, body, PhantomData)
    }
}

impl<T> IntoRequest for ResourcePost<T> {
    fn into_request(self, url: &str) -> Result<ClientRequest, actix_web::Error> {
        client::ClientRequest::post(url)
            .header("Accept", "application/json")
            .body::<Body>(Body::from(self.1))
    }

    fn path(&self) -> &str {
        self.0.as_ref()
    }
}

impl<T: de::DeserializeOwned + 'static> Message for ResourceGet<T> {
    type Result = Result<T, ClientError>;
}

impl<T: de::DeserializeOwned + 'static> Message for ResourceDelete<T> {
    type Result = Result<T, ClientError>;
}

impl<T: de::DeserializeOwned + 'static> Message for ResourcePost<T> {
    type Result = Result<T, ClientError>;
}

impl<T: de::DeserializeOwned + 'static> Message for ResourcePatch<T> {
    type Result = Result<T, ClientError>;
}

impl<T: de::DeserializeOwned + 'static, M: IntoRequest + Message> Handler<M> for ServerClient
where
    M: Message<Result = Result<T, ClientError>> + 'static,
{
    type Result = ActorResponse<ServerClient, T, ClientError>;

    fn handle(&mut self, msg: M, _ctx: &mut Self::Context) -> Self::Result {
        use actix_web::HttpMessage;
        use futures::future;

        ActorResponse::async(
            ConfigManager::from_registry()
                .send(config::GetConfig::new())
                .flatten_fut()
                .map_err(|_e| ClientError::ConfigError)
                .and_then(move |config: Arc<ServerConfig>| {
                    let url = format!("http://127.0.0.1:{}{}", config.port(), msg.path());
                    let client = match msg.into_request(&url) {
                        Ok(cli) => cli,
                        Err(err) => return future::Either::B(future::err(err.into())),
                    };
                    future::Either::A(
                        client
                            .send()
                            .map_err(|e| ClientError::SendRequestError(e))
                            .and_then(|r| r.json::<T>().map_err(|e| ClientError::Json(e))),
                    )
                }).into_actor(self),
        )
    }
}
