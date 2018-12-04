#![allow(dead_code)]

use actix_web::{client, HttpMessage, http};
use bytes::Bytes;
use error::Error;
use futures::stream::Stream;
use futures::{future, Future};
use gu_net::rpc::peer::PeerInfo;
use std::str;
use std::sync::Arc;

/// Hub session information.
#[derive(Clone, Serialize, Deserialize, Debug, Default, Builder)]
#[builder(pattern = "owned", setter(into))]
pub struct SessionInfo {
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
pub struct Driver {
    driver_inner: Arc<DriverInner>,
}

#[derive(Debug)]
struct DriverInner {
    url: String,
}

impl Driver {
    /// creates a driver from a given address:port, e.g. 127.0.0.1:61621
    pub fn from_addr<T: Into<String>>(addr: T) -> Driver {
        Driver {
            driver_inner: Arc::new(DriverInner {
                url: format!("http://{}/", addr.into()),
            }),
        }
    }
    /// creates a new hub session
    /// assumption: url is /sessions, hub session is in the response body
    pub fn new_session(
        &self,
        session_info_builder: SessionInfoBuilder,
    ) -> impl Future<Item = HubSession, Error = Error> {
        let sessions_url = format!("{}sessions", self.driver_inner.url);
        let session_info = match session_info_builder.build() {
            Ok(r) => r,
            _ => return future::Either::A(future::err(Error::InvalidHubSessionParameters)),
        };
        let request = match client::ClientRequest::post(sessions_url).json(session_info) {
            Ok(r) => r,
            _ => return future::Either::A(future::err(Error::CannotCreateRequest)),
        };
        let driver_for_session = self.clone();
        future::Either::B(
            request
                .send()
                .map_err(|_| Error::CannotSendRequest)
                .and_then(|response| {
                    if response.status() != http::StatusCode::CREATED {
                        return future::Either::A(future::err(Error::CannotCreateSession));
                    }
                    future::Either::B(response.body().map_err(|_| Error::CannotGetResponseBody))
                }).and_then(|body| {
                    future::ok(HubSession {
                        driver: driver_for_session,
                        session_id: match str::from_utf8(&body.to_vec()) {
                            Ok(str) => str.to_string(),
                            _ => return future::err(Error::CannotGetResponseBody),
                        },
                    })
                }),
        )
    }
    pub fn auth_app<T: Into<String>, U: Into<String>>(&self, _app_name: T, _token: Option<U>) {}
    /// returns all peers connected to the hub
    /// assumption: url is /peer
    pub fn list_peers(&self) -> impl Future<Item = impl Iterator<Item = PeerInfo>, Error = Error> {
        let url = format!("{}peer", self.driver_inner.url);
        return match client::ClientRequest::get(url).finish() {
            Ok(r) => future::Either::A(
                r.send()
                    .map_err(|_| Error::CannotSendRequest)
                    .and_then(|response| match response.status() {
                        http::StatusCode::OK => future::Either::A(
                            response.json().map_err(|_| Error::InvalidJSONResponse),
                        ),
                        _ => future::Either::B(future::err(Error::InternalError)),
                    }).and_then(|answer_json: Vec<PeerInfo>| future::ok(answer_json.into_iter())),
            ),
            _ => future::Either::B(future::err(Error::CannotCreateRequest)),
        };
    }
}

/// Hub session.
#[derive(Clone, Debug)]
pub struct HubSession {
    driver: Driver,
    pub session_id: String,
}

impl HubSession {
    /// adds peers to the hub
    /// assumption: url is /sessions/{session_id}/peer
    pub fn add_peers<T, U>(&self, peers: T) -> impl Future<Item = (), Error = Error>
    where
        T: IntoIterator<Item = U>,
        U: AsRef<str>,
    {
        let add_url = format!(
            "{}sessions/{}/peer",
            self.driver.driver_inner.url, self.session_id
        );
        let peer_vec: Vec<String> = peers.into_iter().map(|peer| peer.as_ref().into()).collect();
        let request = match client::ClientRequest::post(add_url).json(peer_vec) {
            Ok(r) => r,
            _ => return future::Either::A(future::err(Error::CannotCreateRequest)),
        };
        let session_id_copy = self.session_id.clone();
        future::Either::B(
            request
                .send()
                .map_err(|_| Error::CannotSendRequest)
                .and_then(|response| match response.status() {
                    http::StatusCode::NOT_FOUND => future::Either::A(future::err(Error::SessionNotFound(session_id_copy))),
                    http::StatusCode::INTERNAL_SERVER_ERROR => future::Either::A(future::err(Error::InternalError)),
                    _ => future::Either::B(future::ok(())),
                }),
        )
    }
    /// creates a new blob
    /// assumption: url is /sessions/{session_id}/blob, blob_id is in the response body
    pub fn new_blob(&self) -> impl Future<Item = Blob, Error = Error> {
        let new_blob_url = format!(
            "{}sessions/{}/blob",
            self.driver.driver_inner.url, self.session_id
        );
        let request = match client::ClientRequest::post(new_blob_url).finish() {
            Ok(r) => r,
            _ => return future::Either::A(future::err(Error::CannotCreateRequest)),
        };
        let hub_session_copy = self.clone();
        future::Either::B(
            request
                .send()
                .map_err(|_| Error::CannotSendRequest)
                .and_then(|response| match response.status() {
                    http::StatusCode::CREATED => future::Either::A(response.body().map_err(|_| Error::CannotCreateBlob)),
                    _ => future::Either::B(future::err(Error::InternalError)),
                }).and_then(|body| {
                    future::ok(Blob {
                        hub_session: hub_session_copy,
                        blob_id: match str::from_utf8(&body.to_vec()) {
                            Ok(str) => str.to_string(),
                            _ => return future::err(Error::CannotGetResponseBody),
                        },
                    })
                }),
        )
    }
    /// gets a peer by its id
    /// assumption: url is /peer/{peer_id}
    pub fn peer<T: Into<String>>(&self, peer_id: T) -> impl Future<Item = Peer, Error = Error> {
        let url = format!("{}peer/{}", self.driver.driver_inner.url, peer_id.into());
        let hub_session_copy = self.clone();
        return match client::ClientRequest::get(url).finish() {
            Ok(r) => future::Either::A(
                r.send()
                    .map_err(|_| Error::CannotSendRequest)
                    .and_then(|response| match response.status() {
                        http::StatusCode::OK => future::Either::A(
                            response.json().map_err(|_| Error::InvalidJSONResponse),
                        ),
                        _ => future::Either::B(future::err(Error::InternalError)),
                    }).and_then(|answer_json: PeerInfo| {
                        future::ok(Peer {
                            peer_info: answer_json,
                            hub_session: hub_session_copy,
                        })
                    }),
            ),
            _ => future::Either::B(future::err(Error::CannotCreateRequest)),
        };
    }
}

/// Large binary object.
#[derive(Debug)]
pub struct Blob {
    hub_session: HubSession,
    blob_id: String,
}

struct StreamError<T: std::error::Error> {
    inner: T,
}

impl Blob {
    /// uploads blob represented by a stream
    pub fn upload_from_stream<S, T>(&self, stream: S) -> impl Future<Item = (), Error = Error>
    where
        S: Stream<Item = Bytes, Error = T> + 'static,
        T: Into<actix_web::Error>,
    {
        let url = format!(
            "{}sessions/{}/blob/{}",
            self.hub_session.driver.driver_inner.url, self.hub_session.session_id, self.blob_id
        );
        let request = match client::ClientRequest::post(url).streaming(stream) {
            Ok(r) => r,
            _ => return future::Either::A(future::err(Error::CannotCreateRequest)),
        };
        future::Either::B(
            request
                .send()
                .map_err(|_| Error::CannotSendRequest)
                .and_then(|response| match response.status() {
                    http::StatusCode::OK => future::ok(()),
                    _ => future::err(Error::InternalError),
                }),
        )
    }
}

/// Peer node.
#[derive(Clone)]
pub struct Peer {
    hub_session: HubSession,
    pub peer_info: PeerInfo,
}

impl Peer {
    /// creates new peer session
    /// assumption: url is /sessions/{session_id}/peer/{peer_id}/peer_sessions (was: /hd)
    pub fn new_session(
        &self,
        builder: PeerSessionInfoBuilder,
    ) -> impl Future<Item = PeerSession, Error = Error> {
        let url = format!(
            "{}sessions/{}/peer/{}/peer_sessions",
            self.hub_session.driver.driver_inner.url,
            self.hub_session.session_id,
            self.peer_info.node_id.to_string()
        );
        let session_info = match builder.build() {
            Ok(r) => r,
            _ => return future::Either::A(future::err(Error::InvalidPeerSessionParameters)),
        };
        let request = match client::ClientRequest::post(url).json(session_info) {
            Ok(r) => r,
            _ => return future::Either::A(future::err(Error::CannotCreateRequest)),
        };
        let peer_copy = self.clone();
        future::Either::B(
            request
                .send()
                .map_err(|_| Error::CannotSendRequest)
                .and_then(|response| {
                    if response.status() != http::StatusCode::CREATED {
                        return future::Either::A(future::err(Error::CannotCreateSession));
                    }
                    future::Either::B(response.body().map_err(|_| Error::CannotGetResponseBody))
                }).and_then(|body| {
                    future::ok(PeerSession {
                        peer: peer_copy,
                        session_id: match str::from_utf8(&body.to_vec()) {
                            Ok(str) => str.to_string(),
                            _ => return future::err(Error::CannotGetResponseBody),
                        },
                    })
                }),
        )
    }
}

/// Peer session.
pub struct PeerSession {
    peer: Peer,
    session_id: String,
}