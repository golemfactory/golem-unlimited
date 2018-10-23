#![allow(dead_code)]
#![allow(proc_macro_derive_resolution_fallback)]

use actix::{fut, prelude::*};
use actix_web::*;
use clap::{self, Arg, ArgMatches};
use connect::{self, ConnectionChangeMessage, ListingType};
use futures::prelude::*;
use gu_base::{Decorator, Module};
use gu_ethkey::{EthKey, EthKeyStore, SafeEthKey};
use gu_p2p::{rpc, NodeId};
use gu_persist::{
    config::{ConfigManager, ConfigModule, GetConfig, HasSectionId, SetConfig, SetConfigPath},
    daemon_module::DaemonModule,
    error::Error as ConfigError,
};
use hdman::HdMan;
use mdns::{Responder, Service};
use std::{
    borrow::Cow,
    net::{SocketAddr, ToSocketAddrs},
    sync::Arc,
};

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ServerConfig {
    #[serde(default = "ServerConfig::default_p2p_port")]
    p2p_port: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    control_socket: Option<String>,
    #[serde(default)]
    pub(crate) hub_addrs: Vec<SocketAddr>,
    #[serde(default)]
    publish_service: bool,
    #[serde(default = "ServerConfig::default_connect_mode")]
    pub(crate) connect_mode: ConnectMode,
}

impl Default for ServerConfig {
    fn default() -> Self {
        ServerConfig {
            p2p_port: Self::default_p2p_port(),
            control_socket: None,
            hub_addrs: Vec::new(),
            publish_service: true,
            connect_mode: Self::default_connect_mode(),
        }
    }
}

#[derive(Serialize, Deserialize, PartialEq, Clone, Message)]
#[rtype(result = "Result<Option<()>, String>")]
pub(crate) enum ConnectMode {
    Auto,
    Config,
}

impl ServerConfig {
    fn p2p_addr(&self) -> impl ToSocketAddrs {
        ("0.0.0.0", self.p2p_port)
    }

    fn default_p2p_port() -> u16 {
        61621
    }

    fn default_connect_mode() -> ConnectMode {
        ConnectMode::Config
    }
}

impl HasSectionId for ServerConfig {
    const SECTION_ID: &'static str = "provider-server-cfg";
}

pub struct ServerModule {
    config_path: Option<String>,
    hub_addrs: Option<Vec<SocketAddr>>,
}

impl ServerModule {
    pub fn new() -> Self {
        ServerModule {
            config_path: None,
            hub_addrs: None,
        }
    }
}

fn get_node_id(keys: Box<SafeEthKey>) -> NodeId {
    let node_id = NodeId::from(keys.address().as_ref());
    info!("node_id={:?}", node_id);
    node_id
}

impl Module for ServerModule {
    fn run<D: Decorator + Clone + 'static>(&self, decorator: D) {
        let daemon_module: &DaemonModule = decorator.extract().unwrap();
        if !daemon_module.run() {
            return;
        }

        let config_module: &ConfigModule = decorator.extract().unwrap();

        // TODO: introduce separate actor for key mgmt
        let keys = SafeEthKey::load_or_generate(config_module.keystore_path(), &"".into()).unwrap();

        let _ = ServerConfigurer {
            config_path: self.config_path.clone(),
            node_id: get_node_id(keys),
            hub_addrs: self.hub_addrs.clone().unwrap_or_default(),
            decorator: decorator.clone(),
        }
        .start();

        let _ = HdMan::start(config_module);

        let sys = System::new("gu-provider");
        let _ = sys.run();
    }
}

fn p2p_server(_r: &HttpRequest) -> &'static str {
    "ok"
}

fn mdns_publisher(port: u16) -> Service {
    let responder = Responder::new().expect("Failed to run mDNS publisher");

    responder.register(
        "_unlimited._tcp".to_owned(),
        "gu-provider".to_owned(),
        port,
        &["path=/", ""],
    )
}

struct ServerConfigurer<D> {
    decorator: D,
    config_path: Option<String>,
    node_id: NodeId,
    hub_addrs: Vec<SocketAddr>,
}

impl<D: Decorator + 'static + Sync + Send> Actor for ServerConfigurer<D> {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut <Self as Actor>::Context) {
        let config = ConfigManager::from_registry();

        println!("path={:?}", &self.config_path);
        if let Some(path) = &self.config_path {
            config.do_send(SetConfigPath::FsPath(Cow::Owned(path.clone())));
        }

        let node_id = self.node_id.clone();
        let hub_addrs = self.hub_addrs.clone();

        let decorator = self.decorator.clone();
        ctx.spawn(
            config
                .send(GetConfig::new())
                .map_err(|e| ConfigError::from(e))
                .and_then(|r| r)
                .map_err(|e| println!("error ! {}", e))
                .and_then(move |c: Arc<ServerConfig>| {
                    let decorator = decorator.clone();
                    let server = server::new(move || {
                        decorator.decorate_webapp(App::new().scope("/m", rpc::mock::scope))
                    });
                    let _ = server.bind(c.p2p_addr()).unwrap().start();

                    if c.publish_service {
                        Box::leak(Box::new(mdns_publisher(c.p2p_port)));
                    }

                    config.do_send(SetConfig::new(ServerConfig {
                        hub_addrs: hub_addrs.clone(),
                        ..(*c).clone()
                    }));
                    connect_to_multiple_hubs(node_id, &hub_addrs);
                    connect_to_multiple_hubs(node_id, &c.hub_addrs);
                    println!("{:?}", &c.hub_addrs);

                    Ok(())
                })
                .into_actor(self)
                .and_then(|_, _, ctx| fut::ok(ctx.stop())),
        );

        println!("configured");
    }
}

impl<D> Drop for ServerConfigurer<D> {
    fn drop(&mut self) {
        println!("provider server configured")
    }
}

fn connect_to_multiple_hubs(id: NodeId, hubs: &Vec<SocketAddr>) {
    for hub in hubs {
        rpc::ws::start_connection(id, *hub);
    }
}

impl<D: Decorator + 'static> Handler<ConnectMode> for ServerConfigurer<D> {
    type Result = ActorResponse<Self, Option<()>, String>;

    fn handle(&mut self, msg: ConnectMode, ctx: &mut Context<Self>) -> Self::Result {
        ActorResponse::async(connect::edit_config_connect_mode(msg).into_actor(self))
    }
}

//impl<D: Decorator + 'static> Handler<ConnectionChangeMessage> for ServerConfigurer<D> {
//    type Result = ActorResponse<Self, Option<()>, String>;
//
//    fn handle(&mut self, msg: ConnectionChangeMessage, ctx: &mut Context<Self>,
//    ) -> Self::Result {
//        for host in msg.hubs.iter() {
//            match msg.change {
//
//            }
//        }
//        ActorResponse::async(connect::edit_config_hosts(msg.hubs, msg.change).into_actor(self))
//    }
//}
