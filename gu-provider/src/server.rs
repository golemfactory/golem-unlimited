#![allow(dead_code)]
#![allow(proc_macro_derive_resolution_fallback)]

use actix::prelude::*;
use actix_web::*;
use clap::ArgMatches;
use futures::{future, prelude::*};
use gu_actix::flatten::FlattenFuture;
use gu_base::{Decorator, Module};
use gu_ethkey::prelude::*;
use gu_lan::MdnsPublisher;
use gu_net::{rpc, NodeId};
use gu_persist::{
    config::{ConfigManager, ConfigModule, GetConfig, HasSectionId},
    daemon_module::DaemonModule,
};
use hdman::HdMan;
use mdns::{Responder, Service};
use std::{
    collections::HashSet,
    net::{SocketAddr, ToSocketAddrs},
    sync::Arc,
};
use connect::ConnectManager;
use connect::AutoMdns;
use connect::Connect;

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
    Manual,
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

pub struct ServerModule;

impl ServerModule {
    pub fn new() -> Self {
        ServerModule
    }
}

fn get_node_id(keys: Box<SafeEthKey>) -> NodeId {
    let node_id = NodeId::from(keys.address().as_ref());
    info!("node_id={:?}", node_id);
    node_id
}

impl Module for ServerModule {
    fn args_consume(&mut self, _matches: &ArgMatches) -> bool {
        true
    }

    fn run<D: Decorator + Clone + 'static>(&self, decorator: D) {
        let dec = decorator.clone();
        let daemon_module: &DaemonModule = decorator.extract().unwrap();

        if !daemon_module.run() {
            return;
        }

        use gu_base;

        let sys = System::new("gu-provider");

        gu_base::run_once(move || {
            let config_module: &ConfigModule = dec.extract().unwrap();
            let _ = HdMan::start(config_module);

            ProviderServer::from_registry().do_send(InitServer);
        });

        let _ = sys.run();
    }
}

fn p2p_server(_r: &HttpRequest) -> &'static str {
    "ok"
}

#[derive(Default)]
struct ProviderServer {
    node_id: Option<NodeId>,
    p2p_port: Option<u16>,

    mdns_publisher: MdnsPublisher,
    connections: Option<Addr<ConnectManager>>,
}

impl ProviderServer {
    fn publish_service(&mut self, publish: bool) {
        match publish {
            true => self.mdns_publisher.start(),
            false => self.mdns_publisher.stop(),
        }
    }
}

impl Supervised for ProviderServer {}

impl SystemService for ProviderServer {}

impl Actor for ProviderServer {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        println!("started");
    }
}

#[derive(Message)]
#[rtype(result = "()")]
struct PublishMdns(bool);

impl Handler<PublishMdns> for ProviderServer {
    type Result = ();

    fn handle(&mut self, msg: PublishMdns, _ctx: &mut Context<Self>) -> () {
        self.publish_service(msg.0)
    }
}

#[derive(Message)]
#[rtype(result = "Result<(), ()>")]
struct InitServer;

impl Handler<InitServer> for ProviderServer {
    type Result = ActorResponse<Self, (), ()>;

    fn handle(&mut self, msg: InitServer, _ctx: &mut Context<Self>) -> Self::Result {
        use std::{iter::FromIterator, ops::Deref};

        ActorResponse::async(
            ConfigManager::from_registry()
                .send(GetConfig::new())
                .flatten_fut()
                .and_then(|config: Arc<ServerConfig>| Ok(config.deref().clone()))
                .map_err(|e| error!("{}", e))
                .into_actor(self)
                .and_then(|config: ServerConfig, act: &mut Self, _ctx| {
                    let keys = SafeEthKey::load_or_generate(
                        ConfigModule::new().keystore_path(),
                        &"".into(),
                    )
                    .unwrap();

                    act.node_id = Some(get_node_id(keys));
                    act.p2p_port = Some(config.p2p_port);
                    //act.hub_addrs = HashSet::from_iter(config.hub_addrs.into_iter());

                    // Init mDNS publisher
                    act.mdns_publisher = MdnsPublisher::init_provider(config.p2p_port, act.node_id.unwrap().to_string());
                    act.publish_service(config.publish_service);

                    let connect = ConnectManager::init(act.node_id.unwrap(), Vec::new()).start();
                    for i in config.hub_addrs {
                        connect.do_send(Connect(i));
                    }
                    connect.do_send(AutoMdns(true));
                    act.connections = Some(connect);

                    future::ok(()).into_actor(act)
                }),
        )
    }
}

fn connect_to_multiple_hubs(id: NodeId, hubs: &Vec<SocketAddr>) {
    for hub in hubs {
        rpc::ws::start_connection(id, *hub);
    }
}

//impl Handler<ConnectMode> for ServerConfigurer {
//    type Result = ActorResponse<Self, Option<()>, String>;
//
//    fn handle(&mut self, msg: ConnectMode, _ctx: &mut Context<Self>) -> Self::Result {
//        ActorResponse::async(connect::edit_config_connect_mode(msg).into_actor(self))
//    }
//}

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
