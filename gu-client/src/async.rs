use std::sync::Arc;
use std::time::Duration;
use std::{env, str};

use actix_web::{client, http, HttpMessage};
use bytes::Bytes;
use futures::{future, prelude::*};
use serde::de::DeserializeOwned;
use serde::Serialize;
use url::Url;

use gu_actix::release::{AsyncRelease, Handle};
use gu_model::{
    deployment::DeploymentInfo,
    envman,
    peers::PeerInfo,
    session::{self, BlobInfo, HubExistingSession, HubSessionSpec, Metadata},
    HubInfo,
};
use gu_net::types::NodeId;
use gu_net::types::TryIntoNodeId;

use crate::error::Error;

pub type HubSessionRef = Handle<HubSession>;

/// Connection to a single hub.
#[derive(Clone, Debug)]
pub struct HubConnection {
    hub_connection_inner: Arc<HubConnectionInner>,
}

#[derive(Debug)]
struct HubConnectionInner {
    url: Url,
}

impl Default for HubConnection {
    fn default() -> Self {
        match env::var("GU_HUB_ADDR") {
            Ok(addr) => HubConnection::from_addr(addr).unwrap(),
            Err(_) => HubConnection::from_addr("127.0.0.1:61622").unwrap(),
        }
    }
}

impl HubConnection {
    /// creates a hub connection from a given address:port, e.g. 127.0.0.1:61622
    pub fn from_addr<T: Into<String>>(addr: T) -> Result<HubConnection, Error> {
        Url::parse(&format!("http://{}/", addr.into()))
            .map_err(Error::InvalidAddress)
            .map(|url| HubConnection {
                hub_connection_inner: Arc::new(HubConnectionInner { url: url }),
            })
    }

    /// creates a new hub session
    pub fn new_session(
        &self,
        session_info: HubSessionSpec,
    ) -> impl Future<Item = Handle<HubSession>, Error = Error> + 'static {
        let sessions_url = format!("{}sessions", self.hub_connection_inner.url);
        let hub_connection = self.clone();
        client::ClientRequest::post(sessions_url)
            .json(session_info)
            .into_future()
            .map_err(Error::CreateRequest)
            .and_then(|request| {
                request
                    .send()
                    .from_err()
                    .and_then(|response| match response.status() {
                        http::StatusCode::CREATED => Ok(response),
                        status => Err(Error::ResponseErr(status)),
                    })
                    .and_then(|response| {
                        response.json().from_err().and_then(|session_id: u64| {
                            Ok(Handle::new(HubSession {
                                hub_connection,
                                session_id,
                            }))
                        })
                    })
            })
    }

    pub fn auth_app<T: Into<String>, U: Into<String>>(&self, _app_name: T, _token: Option<U>) {}
    /// returns all peers connected to the hub
    pub fn list_peers(
        &self,
    ) -> impl Future<Item = impl Iterator<Item = PeerInfo>, Error = Error> + 'static {
        let url = format!("{}peers", self.url());

        self.fetch_json(&url)
            .and_then(|response: Vec<PeerInfo>| Ok(response.into_iter()))
    }
    /// returns information about all hub sessions
    pub fn list_sessions(
        &self,
    ) -> impl Future<Item = impl Iterator<Item = HubExistingSession>, Error = Error> + 'static {
        let url = format!("{}sessions", self.url());

        self.fetch_json(&url)
            .and_then(|answer_json: Vec<_>| future::ok(answer_json.into_iter()))
    }
    /// returns hub session object
    pub fn hub_session(&self, session_id: u64) -> HubSession {
        let hub_connection = self.clone();
        HubSession {
            hub_connection,
            session_id,
        }
    }

    pub fn peer<T: Into<NodeId>>(&self, node_id: T) -> ProviderRef {
        let connection = self.clone();
        let node_id = node_id.into();

        ProviderRef {
            connection,
            node_id,
        }
    }

    pub fn url(&self) -> &str {
        self.hub_connection_inner.url.as_ref()
    }

    fn fetch_json<T: DeserializeOwned + 'static>(
        &self,
        url: &str,
    ) -> impl Future<Item = T, Error = Error> + 'static {
        client::ClientRequest::get(&url)
            .finish()
            .into_future()
            .map_err(Error::CreateRequest)
            .and_then(|r| r.send().from_err())
            .and_then(|response| match response.status() {
                http::StatusCode::OK => Ok(response),
                status => Err(Error::ResponseErr(status)),
            })
            .and_then(|response| response.json().from_err())
    }

    fn delete_resource(&self, url: &str) -> impl Future<Item = (), Error = Error> + 'static {
        client::ClientRequest::delete(&url)
            .finish()
            .into_future()
            .map_err(Error::CreateRequest)
            .and_then(|r| r.send().from_err())
            .and_then(|response| match response.status() {
                http::StatusCode::NO_CONTENT => future::Either::A(future::ok(())),
                http::StatusCode::OK => future::Either::B(
                    response
                        .json()
                        .map_err(Error::InvalidJSONResponse)
                        .and_then(|j: serde_json::Value| Ok(eprintln!("{}", j))),
                ),
                http::StatusCode::NOT_FOUND => {
                    future::Either::A(future::err(Error::ResourceNotFound))
                }
                status => future::Either::A(future::err(Error::ResponseErr(status))),
            })
    }

    pub fn info(&self) -> impl Future<Item = HubInfo, Error = Error> + 'static {
        let url = format!("{}info", self.url());
        self.fetch_json(&url)
    }
}

/// Hub session.
#[derive(Clone, Debug)]
pub struct HubSession {
    hub_connection: HubConnection,
    session_id: u64,
}

impl HubSession {
    pub fn id(&self) -> u64 {
        self.session_id
    }

    /// adds peers to the hub session
    pub fn add_peers<T, U: TryIntoNodeId>(
        &self,
        peers: T,
    ) -> impl Future<Item = Vec<NodeId>, Error = Error> + 'static
    where
        T: IntoIterator<Item = U>,
    {
        let add_url = format!(
            "{}sessions/{}/peers",
            self.hub_connection.url(),
            self.session_id
        );

        let peers = match peers
            .into_iter()
            .map(|peer| TryIntoNodeId::into_node_id(peer))
            .collect::<Result<Vec<_>, _>>()
        {
            Ok(p) => p,
            Err(e) => return future::Either::A(future::err(Error::Other(format!("{}", e)))),
        };

        let request = match client::ClientRequest::post(add_url).json(peers) {
            Ok(r) => r,
            Err(e) => return future::Either::A(future::err(Error::CreateRequest(e))),
        };

        future::Either::B(
            request
                .send()
                .from_err()
                .and_then(|response| match response.status() {
                    http::StatusCode::NOT_FOUND => {
                        future::Either::A(future::err(Error::ResourceNotFound))
                    }
                    http::StatusCode::INTERNAL_SERVER_ERROR => {
                        future::Either::A(future::err(Error::ResponseErr(response.status())))
                    }
                    _ => future::Either::B(response.json().from_err()),
                }),
        )
    }
    /// creates a new blob
    pub fn new_blob(&self) -> impl Future<Item = Blob, Error = Error> + 'static {
        let new_blob_url = format!(
            "{}sessions/{}/blobs",
            self.hub_connection.url(),
            self.session_id
        );
        let request = match client::ClientRequest::post(new_blob_url).finish() {
            Ok(r) => r,
            Err(e) => return future::Either::A(future::err(Error::CreateRequest(e))),
        };
        let hub_session = self.clone();
        future::Either::B(
            request
                .send()
                .from_err()
                .and_then(|response| match response.status() {
                    http::StatusCode::CREATED => Ok(response),
                    status => Err(Error::ResponseErr(status)),
                })
                .and_then(|r| r.json().from_err())
                .and_then(|blob_id: u64| {
                    Ok(Blob {
                        hub_session,
                        blob_id,
                    })
                }),
        )
    }
    /// gets single peer by its id
    pub fn peer(&self, node_id: NodeId) -> Peer {
        Peer {
            node_id: node_id,
            hub_session: self.clone(),
        }
    }
    /// gets single peer by its id given as a string
    pub fn try_peer<T: TryIntoNodeId + ToString>(&self, node_id: T) -> Result<Peer, Error> {
        let peer_name = node_id.to_string();
        Ok(self.peer(
            node_id
                .into_node_id()
                .map_err(move |_| Error::InvalidPeer(peer_name))?,
        ))
    }

    /// returns all session peers
    pub fn list_peers(
        &self,
    ) -> impl Future<Item = impl Iterator<Item = PeerInfo>, Error = Error> + 'static {
        let url = format!(
            "{}sessions/{}/peers",
            self.hub_connection.url(),
            self.session_id
        );
        self.hub_connection
            .fetch_json(&url)
            .and_then(|resp: Vec<PeerInfo>| Ok(resp.into_iter()))
    }

    /// gets single blob by its id
    pub fn blob(&self, blob_id: u64) -> Blob {
        Blob {
            blob_id: blob_id,
            hub_session: self.clone(),
        }
    }
    /// returns all session blobs
    pub fn list_blobs(
        &self,
    ) -> impl Future<Item = impl Iterator<Item = BlobInfo>, Error = Error> + 'static {
        let url = format!(
            "{}sessions/{}/blobs",
            self.hub_connection.url(),
            self.session_id
        );
        self.hub_connection
            .fetch_json(&url)
            .and_then(|blobs: Vec<BlobInfo>| Ok(blobs.into_iter()))
    }

    /// gets information about hub session
    pub fn info(&self) -> impl Future<Item = HubSessionSpec, Error = Error> + 'static {
        let url = format!("{}sessions/{}", self.hub_connection.url(), self.session_id);
        self.hub_connection.fetch_json(&url)
    }

    /// sets hub session config
    pub fn set_config(&self, config: Metadata) -> impl Future<Item = (), Error = Error> + 'static {
        let url = format!(
            "{}sessions/{}/config",
            self.hub_connection.url(),
            self.session_id
        );
        future::result(client::ClientRequest::put(url).json(config))
            .map_err(Error::CreateRequest)
            .and_then(|request| request.send().from_err())
            .and_then(|response| match response.status() {
                http::StatusCode::OK => future::ok(()),
                status => future::err(Error::ResponseErr(status)),
            })
    }

    /// gets hub session config
    pub fn config(&self) -> impl Future<Item = Metadata, Error = Error> + 'static {
        let url = format!(
            "{}sessions/{}/config",
            self.hub_connection.url(),
            self.session_id
        );
        self.hub_connection.fetch_json(&url)
    }

    /// updates hub session
    pub fn update(
        &self,
        command: session::Command,
    ) -> impl Future<Item = (), Error = Error> + 'static {
        let url = format!("{}sessions/{}", self.hub_connection.url(), self.session_id);
        future::result(
            client::ClientRequest::build()
                .method(actix_web::http::Method::PATCH)
                .uri(url)
                .json(command),
        )
        .map_err(Error::CreateRequest)
        .and_then(|request| request.send().from_err())
        .and_then(|response| match response.status() {
            http::StatusCode::OK => future::ok(()),
            status => future::err(Error::ResponseErr(status)),
        })
    }
    /// deletes hub session
    pub fn delete(self) -> impl Future<Item = (), Error = Error> + 'static {
        let url = format!("{}sessions/{}", self.hub_connection.url(), self.session_id);
        self.hub_connection.delete_resource(&url)
    }
}

impl AsyncRelease for HubSession {
    type Result = Box<Future<Item = (), Error = Error>>;
    fn release(self) -> Self::Result {
        Box::new(self.delete())
    }
}

/// Large binary object.
#[derive(Clone, Debug)]
pub struct Blob {
    hub_session: HubSession,
    blob_id: u64,
}

impl Blob {
    pub fn id(&self) -> u64 {
        self.blob_id
    }

    pub fn uri(&self) -> String {
        format!(
            "{}sessions/{}/blobs/{}",
            self.hub_session.hub_connection.url(),
            self.hub_session.session_id,
            self.blob_id
        )
    }

    /// uploads blob represented by a stream
    pub fn upload_from_stream<S, T>(&self, stream: S) -> impl Future<Item = (), Error = Error>
    where
        S: Stream<Item = Bytes, Error = T> + 'static,
        T: Into<actix_web::Error>,
    {
        let url = format!(
            "{}sessions/{}/blobs/{}",
            self.hub_session.hub_connection.url(),
            self.hub_session.session_id,
            self.blob_id
        );
        let request = match client::ClientRequest::put(url).streaming(stream) {
            Ok(r) => r,
            Err(e) => return future::Either::A(future::err(Error::CreateRequest(e))),
        };
        future::Either::B(
            request
                .send()
                .from_err()
                .and_then(|response| match response.status() {
                    http::StatusCode::NO_CONTENT => future::ok(()),
                    status => future::err(Error::ResponseErr(status)),
                }),
        )
    }

    /// downloads blob
    pub fn download(&self) -> impl Stream<Item = Bytes, Error = Error> {
        let url = format!(
            "{}sessions/{}/blobs/{}",
            self.hub_session.hub_connection.url(),
            self.hub_session.session_id,
            self.blob_id
        );

        future::result(client::ClientRequest::get(url).finish())
            .map_err(Error::CreateRequest)
            .and_then(|request| request.send().timeout(Duration::from_secs(3600)).from_err())
            .and_then(|response| match response.status() {
                http::StatusCode::OK => future::ok(response.payload().from_err()),
                status => future::err(Error::ResponseErr(status)),
            })
            .flatten_stream()
    }
    /// deletes blob
    pub fn delete(self) -> impl Future<Item = (), Error = Error> {
        let url = format!(
            "{}sessions/{}/blobs/{}",
            self.hub_session.hub_connection.url(),
            self.hub_session.session_id,
            self.blob_id
        );

        self.hub_session.hub_connection.delete_resource(&url)
    }
}

/// Peer node.
#[derive(Clone, Debug)]
pub struct Peer {
    hub_session: HubSession,
    pub node_id: NodeId,
}

impl Peer {
    /// creates new peer session
    pub fn new_session<Options: Serialize>(
        &self,
        session_info: envman::CreateSession<Options>,
    ) -> impl Future<Item = PeerSession, Error = Error> {
        let url = format!(
            "{}sessions/{}/peers/{}/deployments",
            self.hub_session.hub_connection.url(),
            self.hub_session.session_id,
            self.node_id.to_string()
        );
        let request = match client::ClientRequest::post(url).json(session_info) {
            Ok(r) => r,
            Err(e) => return future::Either::A(future::err(Error::CreateRequest(e))),
        };
        let peer_copy = self.clone();
        future::Either::B(
            request
                .send()
                .timeout(Duration::from_secs(3600))
                .from_err()
                .and_then(|response| {
                    if response.status() != http::StatusCode::CREATED {
                        return future::Either::A(future::err(Error::ResponseErr(
                            response.status(),
                        )));
                    }
                    future::Either::B(response.json().map_err(Error::InvalidJSONResponse))
                })
                .and_then(|answer_json: String| {
                    future::ok(PeerSession {
                        peer: peer_copy,
                        session_id: answer_json,
                    })
                }),
        )
    }
    /// gets peer information
    pub fn info(&self) -> impl Future<Item = PeerInfo, Error = Error> {
        let url = format!(
            "{}peers/{:?}",
            self.hub_session.hub_connection.hub_connection_inner.url, self.node_id
        );
        self.hub_session.hub_connection.fetch_json(&url)
    }
}

impl From<Peer> for ProviderRef {
    fn from(peer: Peer) -> Self {
        Self {
            connection: peer.hub_session.hub_connection,
            node_id: peer.node_id,
        }
    }
}

/// Peer session.
#[derive(Clone, Debug)]
pub struct PeerSession {
    peer: Peer,
    session_id: String,
}

impl PeerSession {
    pub fn node_id(&self) -> NodeId {
        self.peer.node_id
    }

    pub fn id(&self) -> &str {
        self.session_id.as_ref()
    }

    /// updates deployment session by sending multiple peer commands
    pub fn update(
        &self,
        commands: Vec<envman::Command>,
    ) -> impl Future<Item = Vec<String>, Error = Error> {
        let url = format!(
            "{}sessions/{}/peers/{}/deployments/{}",
            self.peer
                .hub_session
                .hub_connection
                .hub_connection_inner
                .url,
            self.peer.hub_session.session_id,
            self.peer.node_id.to_string(),
            self.session_id,
        );
        future::result(
            client::ClientRequest::build()
                .method(actix_web::http::Method::PATCH)
                .uri(url)
                .json(commands),
        )
        .map_err(Error::CreateRequest)
        .and_then(|request| {
            request
                .send()
                .timeout(Duration::from_secs(24 * 3600))
                .from_err()
        })
        .and_then(|response| match response.status() {
            http::StatusCode::OK => {
                future::Either::A(future::Either::A(response.json().from_err()))
            }
            http::StatusCode::INTERNAL_SERVER_ERROR => {
                if response.content_type() == "application/json"
                    && response.headers().get("x-processing-error").is_some()
                {
                    future::Either::A(future::Either::B(
                        response
                            .json()
                            .from_err()
                            .and_then(|v: Vec<String>| Err(Error::ProcessingResult(v))),
                    ))
                } else {
                    future::Either::B(future::err(Error::ResponseErr(
                        http::StatusCode::INTERNAL_SERVER_ERROR,
                    )))
                }
            }
            status => future::Either::B(future::err(Error::ResponseErr(status))),
        })
    }
    /// deletes peer session
    pub fn delete(self) -> impl Future<Item = (), Error = Error> {
        let url = format!(
            "{}sessions/{}/peers/{:?}/deployments/{}",
            self.peer.hub_session.hub_connection.url(),
            self.peer.hub_session.session_id,
            self.peer.node_id,
            self.session_id,
        );

        self.peer.hub_session.hub_connection.delete_resource(&url)
    }
}

impl AsyncRelease for PeerSession {
    type Result = Box<Future<Item = (), Error = Error>>;
    fn release(self) -> Self::Result {
        Box::new(self.delete())
    }
}

pub struct ProviderRef {
    connection: HubConnection,
    node_id: NodeId,
}

pub struct DeploymentRef {
    connection: HubConnection,
    node_id: NodeId,
    info: DeploymentInfo,
}

impl DeploymentRef {
    pub fn id(&self) -> &str {
        self.info.id.as_ref()
    }

    pub fn name(&self) -> &str {
        self.info.name.as_ref()
    }

    pub fn tags<'a>(&'a self) -> impl Iterator<Item = impl AsRef<str> + 'a> {
        self.info.tags.iter() //.map(|v| v.as_ref())
    }

    pub fn note(&self) -> Option<&str> {
        self.info.note.as_ref().map(AsRef::as_ref)
    }

    pub fn delete(self) -> impl Future<Item = (), Error = Error> {
        let url = format!(
            "{}peers/{:?}/deployments/{}",
            self.connection.url(),
            &self.node_id,
            &self.info.id
        );
        self.connection.delete_resource(&url)
    }
}

impl ProviderRef {
    pub fn info(&self) -> impl Future<Item = PeerInfo, Error = Error> {
        let url = format!("{}peers/{:?}", self.connection.url(), self.node_id);
        self.connection.fetch_json(&url)
    }

    pub fn deployments(
        &self,
    ) -> impl Future<Item = impl IntoIterator<Item = DeploymentRef>, Error = Error> {
        let url = format!(
            "{}peers/{:?}/deployments",
            self.connection.url(),
            self.node_id
        );
        let connection = self.connection.clone();
        let node_id = self.node_id.clone();

        self.connection
            .fetch_json(&url)
            .and_then(move |list: Vec<_>| {
                Ok(list.into_iter().map(move |i| DeploymentRef {
                    connection: connection.clone(),
                    node_id: node_id.clone(),
                    info: i,
                }))
            })
    }

    pub fn deployment<DeploymentId: AsRef<str>>(
        &self,
        deployment_id: DeploymentId,
    ) -> impl Future<Item = DeploymentRef, Error = Error> {
        let url = format!(
            "{}peers/{:?}/deployments/{}",
            self.connection.url(),
            self.node_id,
            deployment_id.as_ref(),
        );
        let connection = self.connection.clone();
        let node_id = self.node_id.clone();
        self.connection
            .fetch_json(&url)
            .and_then(move |info: DeploymentInfo| {
                Ok(DeploymentRef {
                    connection,
                    node_id,
                    info,
                })
            })
    }

    /// internal use only
    pub fn rpc_call<T: super::PublicMessage + Serialize + 'static>(
        &self,
        msg: T,
    ) -> impl Future<Item = T::Result, Error = Error>
    where
        T::Result: DeserializeOwned,
    {
        let url = format!(
            "{}peers/send-to/{:?}/{}",
            self.connection.url(),
            self.node_id,
            T::ID
        );

        client::ClientRequest::post(url)
            .json(Body { b: msg })
            .into_future()
            .map_err(|e| Error::Other(format!("client request err: {}", e)))
            .and_then(|r| r.send().from_err())
            .and_then(|r| r.json().from_err())
            .and_then(|r: T::Result| Ok(r))
    }
}

#[derive(Serialize)]
struct Body<T: Serialize + 'static> {
    b: T,
}
