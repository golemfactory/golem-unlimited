//! mDNS discovery for Golem Unlimited nodes.
//!

extern crate actix;
extern crate actix_web;
extern crate bytes;
extern crate clap;
extern crate dns_parser;
#[macro_use]
extern crate error_chain;
extern crate futures;
extern crate gu_actix;
extern crate gu_base;
extern crate gu_net;
extern crate hostname;
#[macro_use]
extern crate log;
extern crate mdns;
#[macro_use]
extern crate prettytable;
extern crate rand;
extern crate serde;
extern crate serde_json;
extern crate socket2;
extern crate tokio;
extern crate tokio_codec;

use std::net::SocketAddr;

use mdns::{Responder, Service};
use serde::{Deserialize, Serialize};

pub use continuous::{NewInstance, Subscription};
use gu_net::NodeId;
pub use service::ServiceDescription;

pub mod actor;
mod codec;
mod continuous;

pub mod errors;
pub mod module;
mod service;

pub const ID_LAN: u32 = 576411;

/// Hub mDNS data
#[derive(Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct HubDesc {
    /// ip & TCP port.
    pub address: SocketAddr,
    pub host_name: String,
    /// nodes public key hash
    pub node_id: NodeId,
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
///                            "name={}, addr={:?}, node_id={:?}",
///                            hub.host_name, hub.address, hub.node_id
///                        )
///                    }))).then(|_r| future::ok(System::current().stop()))
///         )
///     );
/// }
/// ```
pub fn list_hubs() -> impl futures::Future<Item = Vec<HubDesc>, Error = ()> {
    use self::actor::{MdnsActor, OneShot};
    use self::service::{ServiceInstance, ServicesDescription};
    use actix::prelude::*;
    use futures::prelude::*;
    use gu_actix::prelude::*;
    use std::collections::HashSet;

    let query = ServicesDescription::new(vec!["hub".into()]);

    MdnsActor::<OneShot>::from_registry()
        .send(query)
        .flatten_fut()
        .and_then(|mut r: HashSet<ServiceInstance>| {
            Ok(r.drain()
                .filter_map(|service_instance| {
                    let node_id = match service_instance.extract("node_id") {
                        Some(Ok(node_id)) => node_id,
                        _ => return None,
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
                })
                .collect())
        })
        .map_err(|_e| ())
}

pub struct MdnsPublisher {
    is_hub: bool,
    port: Option<u16>,
    txt: Vec<String>,
    service: Option<Service>,
}

impl Default for MdnsPublisher {
    fn default() -> Self {
        MdnsPublisher {
            is_hub: true,
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
            self.service = self.mdns_publisher()
        }
    }

    pub fn stop(&mut self) {
        self.service = None
    }

    fn mdns_publisher(&self) -> Option<Service> {
        use std::iter::FromIterator;

        if self.port.is_none() {
            error!("Cannot start mDNS publisher - server not properly initialized");
            panic!("mDNS publisher not initialized before use");
        }

        let service = match self.is_hub {
            true => "_hub",
            false => "_provider",
        };

        let port = self.port.unwrap();
        let responder = Responder::new()
            .or_else(|e| {
                error!("Failed to run mDNS publisher - {}", e);
                Err(())
            })
            .ok()?;

        let name = hostname::get_hostname().unwrap_or_else(|| {
            error!("Couldn't retrieve local hostname");
            "<blank hostname>".to_string()
        });

        Some(responder.register(
            format!("_gu{}._tcp", service),
            name,
            port,
            &Vec::from_iter(self.txt.iter().map(|s| s.as_str())).as_slice(),
        ))
    }

    pub fn init_publisher<S>(port: u16, node_id: S, is_hub: bool) -> Self
    where
        S: AsRef<str>,
    {
        let mut mdns = MdnsPublisher::default();
        mdns.is_hub = is_hub;
        let node_id = format!("node_id={}", node_id.as_ref());
        mdns.init(port, vec![node_id]);

        mdns
    }
}
