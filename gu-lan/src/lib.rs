#[macro_use]
extern crate error_chain;
#[macro_use]
extern crate log;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate prettytable;

extern crate gu_actix;
extern crate gu_base;
extern crate gu_net;

extern crate bytes;
extern crate mdns;
extern crate rand;
extern crate serde_json;
extern crate socket2;

extern crate actix;
extern crate actix_web;
extern crate clap;
extern crate dns_parser;
extern crate futures;
extern crate tokio;
extern crate tokio_codec;

pub use continuous::{NewInstance, Subscription};
pub use service::ServiceDescription;

pub mod actor;
mod codec;
mod continuous;
pub mod errors;
pub mod module;
mod service;

pub const ID_LAN: u32 = 576411;

use mdns::{Responder, Service};

pub struct MdnsPublisher {
    name: &'static str,
    port: Option<u16>,
    txt: Vec<String>,
    service: Option<Service>,
}

impl Default for MdnsPublisher {
    fn default() -> Self {
        MdnsPublisher {
            name: "",
            port: None,
            txt: Vec::new(),
            service: None,
        }
    }
}

impl MdnsPublisher {
    fn init(&mut self, port: u16, txt: Vec<String>) {
        self.port = Some(port);
        self.txt = txt;
    }

    pub fn start(&mut self) {
        if self.service.is_none() {
            self.service = Some(self.mdns_publisher())
        }
    }

    pub fn stop(&mut self) {
        self.service = None
    }

    fn mdns_publisher(&self) -> Service {
        use std::iter::FromIterator;

        if self.port.is_none() {
            error!("Cannot start mDNS publisher - server not properly initialized");
            panic!("mDNS publisher not initialized before use");
        }

        let port = self.port.unwrap();
        let responder = Responder::new().expect("Failed to run mDNS publisher");

        responder.register(
            "_unlimited._tcp".to_owned(),
            self.name.to_string(),
            port,
            &Vec::from_iter(self.txt.iter().map(|s| s.as_str())).as_slice(),
        )
    }

    pub fn init_provider<S>(port: u16, node_id: S) -> Self
    where
        S: AsRef<str>,
    {
        let mut mdns = MdnsPublisher::default();
        mdns.name = "gu-provider";
        let node_id = format!("node_id={}", node_id.as_ref());
        mdns.init(port, vec![node_id]);

        mdns
    }
}
