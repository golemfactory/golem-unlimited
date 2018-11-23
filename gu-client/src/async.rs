#![allow(dead_code)]

use actix_web::{client, HttpMessage};
use error::Error;
use futures::{future, Future};
use gu_net::rpc::peer::PeerInfo;

#[derive(Clone, Serialize, Deserialize, Debug, Default, Builder)]
#[builder(setter(into))]
pub struct SessionInfo {
    name: String,
    environment: String,
}

/// Represents a connection to a single hub.
pub struct Driver {
    url: String,
}

impl Driver {
    // creates a driver from a given URL
    pub fn from_addr(addr: &str) -> Driver {
        Driver {
            url: format!("http://{}/", addr),
        }
    }
    pub fn auth_app(&self, _app_name: String, _token: Option<String>) {}
    // creates a new hub session
    pub fn new_session(&self, session_environment: &str) -> SessionInfoBuilder {
        let mut b = SessionInfoBuilder::default();
        b.environment(session_environment);
        b
    }
    /// returns all peers connected to the hub
    pub fn list_peers(&self) -> impl Future<Item = impl Iterator<Item = PeerInfo>, Error = Error> {
        let url = format!("{}{}", self.url, "peer");
        let request = client::ClientRequest::get(url.clone())
            .finish()
            .expect(format!("Unknown URL: {}", url).as_str());
        request
            .send()
            .map_err(|_| Error::ErrorTODO)
            .and_then(|response| response.json().map_err(|_| Error::ErrorTODO))
            .and_then(|answer_json: Vec<PeerInfo>| future::ok(answer_json.into_iter()))
    }
}

impl SessionInfoBuilder {
    pub fn send(&self) -> Box<Future<Item = HubSession, Error = ()>> {
        let url = "http://10.30.8.179:61622/sessions";
        println!("{}", url.clone());
        let request = client::ClientRequest::post(url.clone())
            .json(self.build().expect("Invalid SessionInfo object."))
            .expect(format!("Unknown URL: {}", url).as_str());
        Box::new(
            request
                .send()
                .map_err(|x| {
                    println!("request.send() err: {:?}", x);
                    ()
                }).and_then(|response| {
                    println!("response {:?}", response);
                    response.body().map_err(|x| {
                        println!("body() error {}", x);
                        ()
                    })
                }).and_then(|body| {
                    println!("BODY:{:?}", body);
                    future::ok(HubSession {})
                }),
        )
    }
}

/* TODO */
#[derive(Debug)]
pub struct HubSession {}
