#![allow(dead_code)]

use actix_web::{client, HttpMessage};
use error::Error;
use futures::{future, Future};
use gu_net::rpc::peer::PeerInfo;
use std::sync::Arc;

#[derive(Clone, Serialize, Deserialize, Debug, Default, Builder)]
#[builder(setter(into))]
pub struct SessionInfo {
    name: String,
    environment: String,
}

/// Represents a connection to a single hub.
#[derive(Clone)]
pub struct Driver {
    driver_inner: Arc<DriverInner>,
}

struct DriverInner {
    url: String,
}

impl Driver {
    /// creates a driver from a given URL
    pub fn from_addr(addr: &str) -> Driver {
        Driver {
            driver_inner: Arc::new(DriverInner {
                url: format!("http://{}/", addr),
            }),
        }
    }
    /// creates a new hub session
    pub fn new_session(
        &self,
        session_info_builder: &SessionInfoBuilder,
    ) -> impl Future<Item = HubSession, Error = Error> {
        let sessions_url = format!("{}{}", self.driver_inner.url, "sessions");
        let session_info = match session_info_builder.build() {
            Ok(r) => r,
            _ => return future::Either::A(future::err(Error::InvalidHubSessionParameters)),
        };
        let request = match client::ClientRequest::post(sessions_url.clone()).json(session_info) {
            Ok(r) => r,
            _ => return future::Either::A(future::err(Error::CannotCreateRequest)),
        };
        let driver_for_session = self.clone();
        future::Either::B(
            request
                .send()
                .map_err(|_| Error::CannotSendRequest)
                .and_then(|response| {
                    println!("response {:?}", response);
                    response.body().map_err(|_| Error::CannotGetResponseBody)
                }).and_then(|body| {
                    println!("BODY:{:?}", body);
                    future::ok(HubSession {
                        driver: driver_for_session,
                    })
                }),
        )
    }
    pub fn auth_app(&self, _app_name: String, _token: Option<String>) {}
    /// returns all peers connected to the hub
    pub fn list_peers(&self) -> impl Future<Item = impl Iterator<Item = PeerInfo>, Error = Error> {
        let url = format!("{}{}", self.driver_inner.url, "peer");
        return match client::ClientRequest::get(url.clone()).finish() {
            Ok(r) => future::Either::A(
                r.send()
                    .map_err(|_| Error::CannotSendRequest)
                    .and_then(|response| response.json().map_err(|_| Error::InvalidJSONResponse))
                    .and_then(|answer_json: Vec<PeerInfo>| future::ok(answer_json.into_iter())),
            ),
            _ => future::Either::B(future::err(Error::CannotCreateRequest)),
        };
    }
}

pub struct HubSession {
    driver: Driver,
}
