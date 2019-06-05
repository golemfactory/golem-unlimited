use actix::{
    Actor, ActorResponse, ArbiterService, Context, Handler, Message, Supervised, WrapFuture,
};
use actix_web::client::Connection;
use actix_web::{self, client::ClientRequest, http, Body};
use bytes::Bytes;
use config::{self, ConfigManager, ConfigModule, HasSectionId};
use futures::{future, Future};
use gu_actix::flatten::FlattenFuture;
use serde::{
    de::{self, DeserializeOwned},
    Serialize,
};
use serde_json;
use std::{marker::PhantomData, sync::Arc};

mod error {
    use actix::MailboxError;
    use actix_web::{self, error::JsonPayloadError};
    use serde_json;

    error_chain! {
        errors {
            Json(e: JsonPayloadError) {}
            MailboxError(e: MailboxError) {}
            ActixError(e: actix_web::Error) {}
            SerdeJson(e: serde_json::Error) {}
            SendRequestError(e: actix_web::client::SendRequestError) {}
            ConfigError {}
            IOError(e: std::io::Error) {}
        }

    }

    impl From<std::io::Error> for Error {
        fn from(e: std::io::Error) -> Self {
            ErrorKind::IOError(e).into()
        }
    }

    impl From<JsonPayloadError> for Error {
        fn from(e: JsonPayloadError) -> Self {
            ErrorKind::Json(e).into()
        }
    }

    impl From<serde_json::Error> for Error {
        fn from(e: serde_json::Error) -> Self {
            ErrorKind::SerdeJson(e).into()
        }
    }

    impl From<actix_web::client::SendRequestError> for Error {
        fn from(e: actix_web::client::SendRequestError) -> Self {
            ErrorKind::SendRequestError(e).into()
        }
    }

    impl From<actix_web::Error> for Error {
        fn from(e: actix_web::Error) -> Self {
            ErrorKind::ActixError(e).into()
        }
    }

    impl From<MailboxError> for Error {
        fn from(e: MailboxError) -> Self {
            ErrorKind::MailboxError(e).into()
        }
    }
}

type ClientError = error::Error;

pub trait ServerConfig:
    'static + Default + Serialize + DeserializeOwned + HasSectionId + Send + Sync
{
    fn port(&self) -> u16;
}

#[derive(Default)]
pub struct ServerClient<C> {
    _phantom: PhantomData<C>,
}

impl<C: ServerConfig> ServerClient<C> {
    pub fn new() -> Self {
        ServerClient {
            _phantom: PhantomData,
        }
    }

    pub fn get<T: de::DeserializeOwned + Send + 'static, IntoStr: Into<String>>(
        path: IntoStr,
    ) -> impl Future<Item = T, Error = ClientError> {
        ServerClient::<C>::from_registry()
            .send(ResourceGet::new(path.into()))
            .flatten_fut()
    }

    pub fn delete<T: de::DeserializeOwned + Send + 'static, IntoStr: Into<String>>(
        path: IntoStr,
    ) -> impl Future<Item = T, Error = ClientError> {
        ServerClient::<C>::from_registry()
            .send(ResourceDelete::new(path.into()))
            .flatten_fut()
    }

    pub fn patch<T: de::DeserializeOwned + Send + 'static, IntoStr: Into<String>>(
        path: IntoStr,
    ) -> impl Future<Item = T, Error = ClientError> {
        ServerClient::<C>::from_registry()
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
        ServerClient::<C>::from_registry()
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
            .map_err(|e| error::ErrorKind::SerdeJson(e).into())
            .and_then(|body| Self::post::<T, _, _>(path, body))
    }

    pub fn empty_post<T: de::DeserializeOwned + Send + 'static, IntoStr: Into<String>>(
        path: IntoStr,
    ) -> impl Future<Item = T, Error = ClientError> {
        ServerClient::<C>::from_registry()
            .send(ResourcePost::new(path.into(), "null".into()))
            .flatten_fut()
    }

    pub fn put<
        T: de::DeserializeOwned + Send + 'static,
        IntoStr: Into<String>,
        IntoBody: Into<Bytes>,
    >(
        path: IntoStr,
        body: IntoBody,
    ) -> impl Future<Item = T, Error = ClientError> {
        ServerClient::<C>::from_registry()
            .send(ResourcePut::new(path.into(), body.into()))
            .flatten_fut()
    }
    pub fn empty_put<T: de::DeserializeOwned + Send + 'static, IntoStr: Into<String>>(
        path: IntoStr,
    ) -> impl Future<Item = T, Error = ClientError> {
        ServerClient::<C>::from_registry()
            .send(ResourcePut::new(path.into(), "null".into()))
            .flatten_fut()
    }
}

impl<C: 'static> Actor for ServerClient<C> {
    type Context = Context<Self>;
}

impl<C: 'static> Supervised for ServerClient<C> {}
impl<C: 'static + Default> ArbiterService for ServerClient<C> {}

pub trait IntoRequest {
    fn into_request(
        self,
        url: &str,
        connection: Option<Connection>,
    ) -> Result<ClientRequest, actix_web::Error>;

    fn path(&self) -> &str;
}

struct ResourceGet<T>(String, PhantomData<T>);

impl<T> ResourceGet<T> {
    fn new(path: String) -> Self {
        ResourceGet::<T>(path, PhantomData)
    }
}

impl<T> IntoRequest for ResourceGet<T> {
    fn into_request(
        self,
        url: &str,
        connection: Option<Connection>,
    ) -> Result<ClientRequest, actix_web::Error> {
        let mut builder = ClientRequest::build();
        if connection.is_some() {
            builder.with_connection(connection.unwrap());
        }
        builder.method(http::Method::GET).uri(url);
        builder.header("Accept", "application/json").finish()
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
    fn into_request(
        self,
        url: &str,
        connection: Option<Connection>,
    ) -> Result<ClientRequest, actix_web::Error> {
        let mut builder = ClientRequest::build();
        if connection.is_some() {
            builder.with_connection(connection.unwrap());
        }
        builder.method(http::Method::DELETE).uri(url);
        builder.header("Accept", "application/json").finish()
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
    fn into_request(
        self,
        url: &str,
        connection: Option<Connection>,
    ) -> Result<ClientRequest, actix_web::Error> {
        let mut builder = ClientRequest::build();
        if connection.is_some() {
            builder.with_connection(connection.unwrap());
        }
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
    fn into_request(
        self,
        url: &str,
        connection: Option<Connection>,
    ) -> Result<ClientRequest, actix_web::Error> {
        let mut builder = ClientRequest::build();
        if connection.is_some() {
            builder.with_connection(connection.unwrap());
        }
        builder.method(http::Method::POST).uri(url);
        builder
            .header("Accept", "application/json")
            .body::<Body>(Body::from(self.1))
    }

    fn path(&self) -> &str {
        self.0.as_ref()
    }
}

struct ResourcePut<T>(String, Bytes, PhantomData<T>);

impl<T> ResourcePut<T> {
    fn new(path: String, body: Bytes) -> Self {
        ResourcePut(path, body, PhantomData)
    }
}

impl<T> IntoRequest for ResourcePut<T> {
    fn into_request(
        self,
        url: &str,
        connection: Option<Connection>,
    ) -> Result<ClientRequest, actix_web::Error> {
        let mut builder = ClientRequest::build();
        if connection.is_some() {
            builder.with_connection(connection.unwrap());
        }
        builder.method(http::Method::PUT).uri(url);
        builder.body::<Body>(Body::from(self.1))
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

impl<T: de::DeserializeOwned + 'static> Message for ResourcePut<T> {
    type Result = Result<T, ClientError>;
}

impl<T: de::DeserializeOwned + 'static> Message for ResourcePatch<T> {
    type Result = Result<T, ClientError>;
}

impl<C: ServerConfig, T: de::DeserializeOwned + 'static, M: IntoRequest + Message> Handler<M>
    for ServerClient<C>
where
    M: Message<Result = Result<T, ClientError>> + 'static,
{
    type Result = ActorResponse<ServerClient<C>, T, ClientError>;

    fn handle(&mut self, msg: M, _ctx: &mut Self::Context) -> Self::Result {
        use actix::SystemService;
        use actix_web::HttpMessage;
        use futures::future;

        let path = msg.path().to_string();

        /* Using Unix domain sockets on macOS and Linux, TCP sockets on Windows. */
        if cfg!(unix) {
            ActorResponse::r#async(
                ConfigManager::from_registry()
                    .send(config::GetConfig::new())
                    .flatten_fut()
                    .map_err(|_e| error::ErrorKind::ConfigError.into())
                    .and_then(move |config: Arc<C>| {
                        let url = format!("http://127.0.0.1:{}{}", config.port(), &path);
                        use tokio_uds::UnixStream;
                        let uds_path = ConfigModule::new().runtime_dir().join("gu-provider.socket");
                        info!("Connecting to unix domain socket at {:?}", &uds_path);
                        UnixStream::connect(uds_path)
                            .map_err(|e| error::ErrorKind::IOError(e).into())
                            .join(future::ok(url))
                    })
                    .and_then(move |(stream, url)| {
                        let connection = actix_web::client::Connection::from_stream(stream);
                        let client = match msg.into_request(&url, Some(connection)) {
                            Ok(cli) => cli,
                            Err(err) => return future::Either::B(future::err(err.into())),
                        };
                        future::Either::A(
                            client
                                .send()
                                .map_err(|e| error::ErrorKind::SendRequestError(e).into())
                                .and_then(|r| {
                                    r.json::<T>()
                                        .map_err(move |e| error::ErrorKind::Json(e).into())
                                }),
                        )
                    })
                    .into_actor(self),
            )
        } else {
            ActorResponse::r#async(
                ConfigManager::from_registry()
                    .send(config::GetConfig::new())
                    .flatten_fut()
                    .map_err(|_e| error::ErrorKind::ConfigError.into())
                    .and_then(move |config: Arc<C>| {
                        let url = format!("http://127.0.0.1:{}{}", config.port(), &path);
                        let client = match msg.into_request(&url, None) {
                            Ok(cli) => cli,
                            Err(err) => return future::Either::B(future::err(err.into())),
                        };
                        future::Either::A(
                            client
                                .send()
                                .map_err(|e| error::ErrorKind::SendRequestError(e).into())
                                .and_then(|r| {
                                    r.json::<T>()
                                        .map_err(move |e| error::ErrorKind::Json(e).into())
                                }),
                        )
                    })
                    .into_actor(self),
            )
        }
    }
}
