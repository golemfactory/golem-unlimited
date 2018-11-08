//! mDNS discovery for Golem Unlimited nodes.
//!

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
use std::net::SocketAddr;

/// Hub mDNS data
#[derive(Clone)]
pub struct HubDesc {
    /// ip & TCP port.
    pub address: SocketAddr,
    pub host_name: String,
    /// nodes public key hash
    pub node_id: String,
}

/// Lists HUBs visible in local network.
///
/// # Example
///
/// ```
/// extern crate actix;
/// extern crate futures;
/// extern crate gu_lan;
///
/// use actix::prelude::*;
/// use futures::{future, prelude::*};
///
///
/// fn main() {
///     System::run(||
///         Arbiter::spawn(
///            gu_lan::list_hubs()
///               .and_then(|hubs|
///                    Ok(hubs.iter().for_each(|hub| {
///                        println!(
///                            "name={}, addr={:?}, node_id={}",
///                            hub.host_name, hub.address, hub.node_id
///                        )
///                    }))).then(|_r| future::ok(System::current().stop()))
///         )
///     );
/// }
/// ```
pub fn list_hubs() -> impl futures::Future<Item = Vec<HubDesc>, Error=()> {
    use self::actor::{MdnsActor, OneShot};
    use self::service::{ServiceInstance, ServicesDescription};
    use actix::prelude::*;
    use futures::prelude::*;
    use gu_actix::prelude::*;
    use std::collections::HashSet;

    let query = ServicesDescription::new(vec!["gu-hub".into()]);

    MdnsActor::<OneShot>::from_registry()
        .send(query)
        .flatten_fut()
        .and_then(|mut r: HashSet<ServiceInstance>| {
            Ok(r.drain()
                .filter_map(|service_instance| {
                    let node_id = match service_instance.extract("node_id") {
                        Some(Ok(node_id)) => node_id,
                        _ => String::default(),
                    };
                    let host_name = service_instance.host;
                    match (
                        service_instance.addrs_v4.first(),
                        service_instance.ports.first(),
                    ) {
                        (Some(address), Some(port)) => {
                            let address = (*address, *port as u16).into();
                            Some(HubDesc {
                                address,
                                host_name,
                                node_id,
                            })
                        }
                        (_, _) => {
                            warn!("instance not found");
                            None
                        }
                    }
                }).collect())
        }).map_err(|_e| ())
}

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
