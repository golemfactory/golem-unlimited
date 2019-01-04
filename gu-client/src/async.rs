use actix_web::{client, http, HttpMessage};
use bytes::Bytes;
use error::Error;
use futures::stream::Stream;
use futures::{future, Future};
use gu_actix::release::{AsyncRelease, Handle};
use gu_model::peers::PeerInfo;
use gu_model::session::{BlobInfo, HubSessionSpec, Metadata};
use gu_model::{envman, session};
use gu_net::types::NodeId;
use std::str;
use std::sync::Arc;
use url::Url;

/// Hub session information.
#[derive(Clone, Serialize, Deserialize, Debug, Default, Builder)]
#[builder(pattern = "owned", setter(into))]
pub struct HubSessionInfo {
    name: String,
    environment: String,
}

/// Peer session information.
#[derive(Clone, Serialize, Deserialize, Debug, Default, Builder)]
#[builder(pattern = "owned", setter(into))]
pub struct PeerSessionInfo {
    image: PeerSessionImage,
    environment: String,
}

/// Peer session image.
#[derive(Clone, Serialize, Deserialize, Debug, Default)]
pub struct PeerSessionImage {
    url: String,
    hash: String,
}

/// Connection to a single hub.
#[derive(Clone, Debug)]
pub struct HubConnection {
    hub_connection_inner: Arc<HubConnectionInner>,
}

#[derive(Debug)]
struct HubConnectionInner {
    url: Url,
}

impl HubConnection {
    /// creates a hub connection from a given address:port, e.g. 127.0.0.1:61621
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
        session_info_builder: HubSessionInfoBuilder,
    ) -> impl Future<Item = Handle<HubSession>, Error = Error> {
        let sessions_url = format!("{}sessions", self.hub_connection_inner.url);
        let session_info = match session_info_builder.build() {
            Ok(r) => r,
            Err(e) => return future::Either::A(future::err(Error::InvalidHubSessionParameters(e))),
        };
        let request = match client::ClientRequest::post(sessions_url).json(session_info) {
            Ok(r) => r,
            Err(e) => return future::Either::A(future::err(Error::CannotCreateRequest(e))),
        };
        let hub_connection_for_session = self.clone();
        future::Either::B(
            request
                .send()
                .map_err(Error::CannotSendRequest)
                .and_then(|response| {
                    if response.status() != http::StatusCode::CREATED {
                        return future::Either::A(future::err(Error::CannotCreateHubSession(
                            response.status(),
                        )));
                    }
                    future::Either::B(response.body().map_err(Error::CannotGetResponseBody))
                })
                .and_then(|body| {
                    future::ok(Handle::new(HubSession {
                        hub_connection: hub_connection_for_session,
                        session_id: match str::from_utf8(&body.to_vec()) {
                            Ok(str) => str.to_string(),
                            Err(e) => return future::err(Error::CannotConvertToUTF8(e)),
                        },
                    }))
                }),
        )
    }
    pub fn auth_app<T: Into<String>, U: Into<String>>(&self, _app_name: T, _token: Option<U>) {}
    /// returns all peers connected to the hub
    pub fn list_peers(&self) -> impl Future<Item = impl Iterator<Item = PeerInfo>, Error = Error> {
        let url = format!("{}peers", self.hub_connection_inner.url);
        match client::ClientRequest::get(url).finish() {
            Ok(r) => future::Either::A(
                r.send()
                    .map_err(Error::CannotSendRequest)
                    .and_then(|response| match response.status() {
                        http::StatusCode::OK => {
                            future::Either::A(response.json().map_err(Error::InvalidJSONResponse))
                        }
                        status => future::Either::B(future::err(Error::CannotListHubPeers(status))),
                    })
                    .and_then(|answer_json: Vec<PeerInfo>| future::ok(answer_json.into_iter())),
            ),
            Err(e) => future::Either::B(future::err(Error::CannotCreateRequest(e))),
        }
    }
    /// returns information about all hub sessions
    pub fn list_sessions(
        &self,
    ) -> impl Future<Item = impl Iterator<Item = HubSessionSpec>, Error = Error> {
        let url = format!("{}sessions", self.hub_connection_inner.url);
        match client::ClientRequest::get(url).finish() {
            Ok(r) => future::Either::A(
                r.send()
                    .map_err(Error::CannotSendRequest)
                    .and_then(|response| match response.status() {
                        http::StatusCode::OK => {
                            future::Either::A(response.json().map_err(Error::InvalidJSONResponse))
                        }
                        status => {
                            future::Either::B(future::err(Error::CannotListHubSessions(status)))
                        }
                    })
                    .and_then(|answer_json: Vec<HubSessionSpec>| {
                        future::ok(answer_json.into_iter())
                    }),
            ),
            Err(e) => future::Either::B(future::err(Error::CannotCreateRequest(e))),
        }
    }
    /// returns hub session object
    pub fn hub_session<T: Into<String>>(&self, session_id: T) -> HubSession {
        HubSession {
            hub_connection: self.clone(),
            session_id: session_id.into(),
        }
    }
}

/// Hub session.
#[derive(Clone, Debug)]
pub struct HubSession {
    hub_connection: HubConnection,
    session_id: String,
}

impl HubSession {
    /// adds peers to the hub
    pub fn add_peers<T, U>(&self, peers: T) -> impl Future<Item = (), Error = Error>
    where
        T: IntoIterator<Item = U>,
        U: AsRef<str>,
    {
        let add_url = format!(
            "{}sessions/{}/peers",
            self.hub_connection.hub_connection_inner.url, self.session_id
        );
        let peer_vec: Vec<String> = peers.into_iter().map(|peer| peer.as_ref().into()).collect();
        let request = match client::ClientRequest::post(add_url).json(peer_vec) {
            Ok(r) => r,
            Err(e) => return future::Either::A(future::err(Error::CannotCreateRequest(e))),
        };
        let session_id_copy = self.session_id.clone();
        future::Either::B(
            request
                .send()
                .map_err(Error::CannotSendRequest)
                .and_then(|response| match response.status() {
                    http::StatusCode::NOT_FOUND => {
                        future::Either::A(future::err(Error::SessionNotFound(session_id_copy)))
                    }
                    http::StatusCode::INTERNAL_SERVER_ERROR => future::Either::A(future::err(
                        Error::CannotAddPeersToSession(response.status()),
                    )),
                    _ => future::Either::B(future::ok(())),
                }),
        )
    }
    /// creates a new blob
    pub fn new_blob(&self) -> impl Future<Item = Blob, Error = Error> {
        let new_blob_url = format!(
            "{}sessions/{}/blobs",
            self.hub_connection.hub_connection_inner.url, self.session_id
        );
        let request = match client::ClientRequest::post(new_blob_url).finish() {
            Ok(r) => r,
            Err(e) => return future::Either::A(future::err(Error::CannotCreateRequest(e))),
        };
        let hub_session_copy = self.clone();
        future::Either::B(
            request
                .send()
                .map_err(Error::CannotSendRequest)
                .and_then(|response| match response.status() {
                    http::StatusCode::CREATED => {
                        future::Either::A(response.body().map_err(Error::CannotGetResponseBody))
                    }
                    status => future::Either::B(future::err(Error::CannotCreateBlob(status))),
                })
                .and_then(|body| {
                    future::ok(Blob {
                        hub_session: hub_session_copy,
                        blob_id: match str::from_utf8(&body.to_vec()) {
                            Ok(str) => str.to_string(),
                            Err(e) => return future::err(Error::CannotConvertToUTF8(e)),
                        },
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
    /// returns all session peers
    pub fn list_peers(&self) -> impl Future<Item = impl Iterator<Item = PeerInfo>, Error = Error> {
        let url = format!(
            "{}sessions/{}/peers",
            self.hub_connection.hub_connection_inner.url, self.session_id
        );
        let request = match client::ClientRequest::get(url).finish() {
            Ok(r) => r,
            Err(e) => return future::Either::A(future::err(Error::CannotCreateRequest(e))),
        };
        future::Either::B(
            request
                .send()
                .map_err(Error::CannotSendRequest)
                .and_then(|response| match response.status() {
                    http::StatusCode::OK => {
                        future::Either::A(response.json().map_err(Error::InvalidJSONResponse))
                    }
                    status => future::Either::B(future::err(Error::CannotListSessionPeers(status))),
                })
                .and_then(|answer_json: Vec<PeerInfo>| future::ok(answer_json.into_iter())),
        )
    }
    /// gets single blob by its id
    pub fn blob(&self, blob_id: String) -> Blob {
        Blob {
            blob_id: blob_id,
            hub_session: self.clone(),
        }
    }
    /// returns all session blobs
    pub fn list_blobs(&self) -> impl Future<Item = impl Iterator<Item = BlobInfo>, Error = Error> {
        let url = format!(
            "{}sessions/{}/blobs",
            self.hub_connection.hub_connection_inner.url, self.session_id
        );
        let request = match client::ClientRequest::get(url).finish() {
            Ok(r) => r,
            Err(e) => return future::Either::A(future::err(Error::CannotCreateRequest(e))),
        };
        future::Either::B(
            request
                .send()
                .map_err(Error::CannotSendRequest)
                .and_then(|response| match response.status() {
                    http::StatusCode::OK => {
                        future::Either::A(response.json().map_err(Error::InvalidJSONResponse))
                    }
                    status => future::Either::B(future::err(Error::CannotListSessionBlobs(status))),
                })
                .and_then(|answer_json: Vec<BlobInfo>| future::ok(answer_json.into_iter())),
        )
    }
    /// gets information about hub session
    pub fn info(&self) -> impl Future<Item = HubSessionSpec, Error = Error> {
        let url = format!(
            "{}sessions/{}",
            self.hub_connection.hub_connection_inner.url, self.session_id
        );
        match client::ClientRequest::get(url).finish() {
            Ok(r) => future::Either::A(r.send().map_err(Error::CannotSendRequest).and_then(
                |response| match response.status() {
                    http::StatusCode::OK => {
                        future::Either::A(response.json().map_err(Error::InvalidJSONResponse))
                    }
                    status => future::Either::B(future::err(Error::CannotGetHubSession(status))),
                },
            )),
            Err(e) => future::Either::B(future::err(Error::CannotCreateRequest(e))),
        }
    }
    /// sets hub session config
    pub fn set_config(&self, config: Metadata) -> impl Future<Item = (), Error = Error> {
        let url = format!(
            "{}sessions/{}/config",
            self.hub_connection.hub_connection_inner.url, self.session_id
        );
        future::result(client::ClientRequest::put(url).json(config))
            .map_err(Error::CannotCreateRequest)
            .and_then(|request| request.send().map_err(Error::CannotSendRequest))
            .and_then(|response| match response.status() {
                http::StatusCode::OK => future::ok(()),
                status => future::err(Error::CannotSetHubSessionConfig(status)),
            })
    }
    /// gets hub session config
    pub fn config(&self) -> impl Future<Item = Metadata, Error = Error> {
        let url = format!(
            "{}sessions/{}/config",
            self.hub_connection.hub_connection_inner.url, self.session_id
        );
        future::result(client::ClientRequest::get(url).finish())
            .map_err(Error::CannotCreateRequest)
            .and_then(|request| request.send().map_err(Error::CannotSendRequest))
            .and_then(|response| match response.status() {
                http::StatusCode::OK => {
                    future::Either::A(response.json().map_err(Error::InvalidJSONResponse))
                }
                status => future::Either::B(future::err(Error::CannotGetHubSessionConfig(status))),
            })
    }
    /// updates hub session
    pub fn update(&self, command: session::Command) -> impl Future<Item = (), Error = Error> {
        let url = format!(
            "{}sessions/{}",
            self.hub_connection.hub_connection_inner.url, self.session_id
        );
        future::result(
            client::ClientRequest::build()
                .method(actix_web::http::Method::PATCH)
                .uri(url)
                .json(command),
        )
        .map_err(Error::CannotCreateRequest)
        .and_then(|request| request.send().map_err(Error::CannotSendRequest))
        .and_then(|response| match response.status() {
            http::StatusCode::OK => future::ok(()),
            status => future::err(Error::CannotUpdateHubSession(status)),
        })
    }
    /// deletes hub session
    pub fn delete(self) -> impl Future<Item = (), Error = Error> {
        let remove_url = format!(
            "{}sessions/{}",
            self.hub_connection.hub_connection_inner.url, self.session_id
        );
        let request = match client::ClientRequest::delete(remove_url).finish() {
            Ok(r) => r,
            Err(e) => return future::Either::A(future::err(Error::CannotCreateRequest(e))),
        };
        future::Either::B(
            request
                .send()
                .map_err(Error::CannotSendRequest)
                .and_then(|response| match response.status() {
                    http::StatusCode::OK => future::ok(()),
                    status_code => future::err(Error::CannotDeleteHubSession(status_code)),
                }),
        )
    }
}

impl AsyncRelease for HubSession {
    type Result = Box<Future<Item = (), Error = Error>>;
    fn release(self) -> Self::Result {
        Box::new(self.delete())
    }
}

/// Large binary object.
#[derive(Debug)]
pub struct Blob {
    hub_session: HubSession,
    blob_id: String,
}

impl Blob {
    /// uploads blob represented by a stream
    pub fn upload_from_stream<S, T>(&self, stream: S) -> impl Future<Item = (), Error = Error>
    where
        S: Stream<Item = Bytes, Error = T> + 'static,
        T: Into<actix_web::Error>,
    {
        let url = format!(
            "{}sessions/{}/blobs/{}",
            self.hub_session.hub_connection.hub_connection_inner.url,
            self.hub_session.session_id,
            self.blob_id
        );
        let request = match client::ClientRequest::put(url).streaming(stream) {
            Ok(r) => r,
            Err(e) => return future::Either::A(future::err(Error::CannotCreateRequest(e))),
        };
        future::Either::B(
            request
                .send()
                .map_err(Error::CannotSendRequest)
                .and_then(|response| match response.status() {
                    http::StatusCode::OK => future::ok(()),
                    status => future::err(Error::CannotUploadBlobFromStream(status)),
                }),
        )
    }
    /// downloads blob
    pub fn download(&self) -> impl Stream {
        let url = format!(
            "{}sessions/{}/blobs/{}",
            self.hub_session.hub_connection.hub_connection_inner.url,
            self.hub_session.session_id,
            self.blob_id
        );
        future::result(client::ClientRequest::get(url).finish())
            .map_err(Error::CannotCreateRequest)
            .and_then(|request| request.send().map_err(Error::CannotSendRequest))
            .and_then(|response| match response.status() {
                http::StatusCode::OK => {
                    future::ok(response.payload().map_err(Error::CannotReceiveBlobBody))
                }
                status => future::err(Error::CannotReceiveBlob(status)),
            })
            .flatten_stream()
    }
    /// deletes blob
    pub fn delete(self) -> impl Future<Item = (), Error = Error> {
        let remove_url = format!(
            "{}sessions/{}/blobs/{}",
            self.hub_session.hub_connection.hub_connection_inner.url,
            self.hub_session.session_id,
            self.blob_id
        );
        let request = match client::ClientRequest::delete(remove_url).finish() {
            Ok(r) => r,
            Err(e) => return future::Either::A(future::err(Error::CannotCreateRequest(e))),
        };
        future::Either::B(
            request
                .send()
                .map_err(Error::CannotSendRequest)
                .and_then(|response| match response.status() {
                    http::StatusCode::OK => future::ok(()),
                    status_code => future::err(Error::CannotDeleteBlob(status_code)),
                }),
        )
    }
}

/// Peer node.
#[derive(Clone)]
pub struct Peer {
    hub_session: HubSession,
    pub node_id: NodeId,
}

impl Peer {
    /// creates new peer session
    pub fn new_session(
        &self,
        builder: PeerSessionInfoBuilder,
    ) -> impl Future<Item = PeerSession, Error = Error> {
        let url = format!(
            "{}sessions/{}/peers/{}/deployments",
            self.hub_session.hub_connection.hub_connection_inner.url,
            self.hub_session.session_id,
            self.node_id.to_string()
        );
        let session_info = match builder.build() {
            Ok(r) => r,
            Err(e) => return future::Either::A(future::err(Error::InvalidPeerSessionParameters(e))),
        };
        let request = match client::ClientRequest::post(url).json(session_info) {
            Ok(r) => r,
            Err(e) => return future::Either::A(future::err(Error::CannotCreateRequest(e))),
        };
        let peer_copy = self.clone();
        future::Either::B(
            request
                .send()
                .map_err(Error::CannotSendRequest)
                .and_then(|response| {
                    if response.status() != http::StatusCode::CREATED {
                        return future::Either::A(future::err(Error::CannotCreatePeerSession(
                            response.status(),
                        )));
                    }
                    future::Either::B(response.body().map_err(Error::CannotGetResponseBody))
                })
                .and_then(|body| {
                    future::ok(PeerSession {
                        peer: peer_copy,
                        session_id: match str::from_utf8(&body.to_vec()) {
                            Ok(str) => str.to_string(),
                            Err(e) => return future::err(Error::CannotConvertToUTF8(e)),
                        },
                    })
                }),
        )
    }
    /// gets peer information
    pub fn info(&self) -> impl Future<Item = PeerInfo, Error = Error> {
        let url = format!(
            "{}peers/{}",
            self.hub_session.hub_connection.hub_connection_inner.url,
            self.node_id.to_string()
        );
        future::result(client::ClientRequest::get(url).finish())
            .map_err(Error::CannotCreateRequest)
            .and_then(|request| request.send().map_err(Error::CannotSendRequest))
            .and_then(|response| match response.status() {
                http::StatusCode::OK => {
                    future::Either::A(response.json().map_err(Error::InvalidJSONResponse))
                }
                status => future::Either::B(future::err(Error::CannotGetPeerInfo(status))),
            })
            .and_then(|answer_json: PeerInfo| future::ok(answer_json))
    }
}

/// Peer session.
pub struct PeerSession {
    peer: Peer,
    session_id: String,
}

impl PeerSession {
    /// updates deployment session by sending multiple peer commands
    pub fn update(&self, commands: Vec<envman::Command>) -> impl Future<Item = (), Error = Error> {
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
        .map_err(Error::CannotCreateRequest)
        .and_then(|request| request.send().map_err(Error::CannotSendRequest))
        .and_then(|response| match response.status() {
            http::StatusCode::OK => future::ok(()),
            status => future::err(Error::CannotUpdateDeployment(status)),
        })
    }
    /// deletes peer session
    pub fn delete(self) -> impl Future<Item = (), Error = Error> {
        let remove_url = format!(
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
        let request = match client::ClientRequest::delete(remove_url).finish() {
            Ok(r) => r,
            Err(e) => return future::Either::A(future::err(Error::CannotCreateRequest(e))),
        };
        future::Either::B(
            request
                .send()
                .map_err(Error::CannotSendRequest)
                .and_then(|response| match response.status() {
                    http::StatusCode::OK => future::ok(()),
                    status_code => future::err(Error::CannotDeletePeerSession(status_code)),
                }),
        )
    }
}

impl AsyncRelease for PeerSession {
    type Result = Box<Future<Item = (), Error = Error>>;
    fn release(self) -> Self::Result {
        Box::new(self.delete())
    }
}
