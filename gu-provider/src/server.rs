#![allow(proc_macro_derive_resolution_fallback)]

use std::{
    collections::HashSet,
    net::{SocketAddr, ToSocketAddrs},
    path::PathBuf,
    sync::Arc,
};

use ::actix::prelude::*;
use actix_web::*;
use clap::ArgMatches;
use futures::{future, prelude::*};
use log::{error, info, warn};
use serde::{Deserialize, Serialize};

use ethkey::prelude::*;
use gu_actix::flatten::FlattenFuture;
use gu_base::{
    daemon_lib::{DaemonCommand, DaemonHandler},
    Decorator, Module,
};
use gu_lan::MdnsPublisher;
use gu_net::{rpc, NodeId};
use gu_persist::{
    config::{ConfigManager, ConfigModule, GetConfig, HasSectionId},
    http::{ServerClient, ServerConfig},
};

use crate::connect::ListingType;
use crate::connect::{
    self, AutoMdns, Connect, ConnectManager, ConnectModeMessage, ConnectionChange,
    ConnectionChangeMessage, Disconnect, ListSockets,
};
use crate::hdman::HdMan;

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProviderConfig {
    #[serde(default = "ProviderConfig::default_p2p_port")]
    p2p_port: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    control_socket: Option<String>,
    #[serde(default)]
    pub(crate) hub_addrs: HashSet<SocketAddr>,
    #[serde(default)]
    publish_service: bool,
    #[serde(default = "ProviderConfig::default_connect_mode")]
    pub(crate) connect_mode: ConnectMode,
}

impl Default for ProviderConfig {
    fn default() -> Self {
        ProviderConfig {
            p2p_port: Self::default_p2p_port(),
            control_socket: None,
            hub_addrs: HashSet::new(),
            publish_service: true,
            connect_mode: Self::default_connect_mode(),
        }
    }
}

impl ServerConfig for ProviderConfig {
    fn port(&self) -> u16 {
        Self::default_p2p_port()
    }
}

pub(crate) type ProviderClient = ServerClient<ProviderConfig>;

#[derive(Serialize, Deserialize, PartialEq, Clone, Message, Debug, Copy)]
#[rtype(result = "Result<Option<()>, String>")]
pub(crate) enum ConnectMode {
    Auto,
    Manual,
}

impl ProviderConfig {
    fn p2p_addr(&self) -> impl ToSocketAddrs {
        ("0.0.0.0", self.p2p_port)
    }

    fn default_p2p_port() -> u16 {
        61621
    }

    fn default_connect_mode() -> ConnectMode {
        ConnectMode::Manual
    }
}

impl HasSectionId for ProviderConfig {
    const SECTION_ID: &'static str = "provider-server-cfg";
}

pub struct ServerModule {
    daemon_command: DaemonCommand,
}

impl ServerModule {
    pub fn new() -> Self {
        ServerModule {
            daemon_command: DaemonCommand::None,
        }
    }
}

fn get_node_id(keys: Box<EthAccount>) -> NodeId {
    let node_id = NodeId::from(keys.address().as_ref());
    info!("node_id={:?}", node_id);
    node_id
}

impl Module for ServerModule {
    fn args_declare<'a, 'b>(&self, app: gu_base::App<'a, 'b>) -> gu_base::App<'a, 'b> {
        app.subcommand(DaemonHandler::subcommand())
    }

    fn args_consume(&mut self, matches: &ArgMatches) -> bool {
        self.daemon_command = DaemonHandler::consume(matches);
        self.daemon_command != DaemonCommand::None
    }

    fn run<D: Decorator + Clone + 'static>(&self, decorator: D) {
        use gu_base;
        let dec = decorator.clone();
        let config_module: &ConfigModule = dec.extract().unwrap();

        if !DaemonHandler::provider(self.daemon_command, config_module.work_dir()).run() {
            return;
        }

        let sys = System::new("gu-provider");

        let socket_path = config_module.runtime_dir().join("gu-provider.socket");
        let keystore_path = config_module.keystore_path();

        gu_base::run_once(move || {
            let dec = decorator.to_owned();
            let config_module: &ConfigModule = dec.extract().unwrap();
            let _ = HdMan::start(config_module);

            ProviderServer::from_registry().do_send(InitServer {
                decorator,
                socket_path,
                keystore_path,
            });
        });

        let _ = sys.run();
    }

    fn decorate_webapp<S: 'static>(&self, app: actix_web::App<S>) -> actix_web::App<S> {
        app
    }
}

#[derive(Default)]
pub struct ProviderServer {
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

#[derive(Message, Clone)]
#[rtype(result = "Result<(), ()>")]
struct InitServer<D: Decorator> {
    decorator: D,
    socket_path: PathBuf,
    keystore_path: PathBuf,
}

impl<D: Decorator + 'static> Handler<InitServer<D>> for ProviderServer {
    type Result = ActorResponse<Self, (), ()>;

    fn handle(&mut self, msg: InitServer<D>, _ctx: &mut Context<Self>) -> Self::Result {
        use std::ops::Deref;

        let uds_path = msg.clone().socket_path;
        let keystore_path = msg.clone().keystore_path;
        let server = server::new(move || {
            msg.decorator
                .decorate_webapp(App::new().scope("/m", rpc::mock::scope))
        });

        ActorResponse::r#async(
            ConfigManager::from_registry()
                .send(GetConfig::new())
                .flatten_fut()
                .and_then(|config: Arc<ProviderConfig>| Ok(config.deref().clone()))
                .map_err(|e| error!("{}", e))
                .into_actor(self)
                .and_then(move |config: ProviderConfig, act: &mut Self, _ctx| {
                    use tokio_uds;
                    let dir_path = uds_path.parent().unwrap();
                    if !dir_path.exists() {
                        info!("Creating {:?}.", dir_path);
                        let _ = std::fs::create_dir_all(dir_path)
                            .and_then(|_| Ok(info!("Created {:?}.", dir_path)))
                            .or_else(|_| Err(warn!("Cannot create {:?}.", dir_path)));
                    }
                    let listener = tokio_uds::UnixListener::bind(&uds_path)
                        .or_else(|e| {
                            info!("Cannot bind to socket ({:?}), error: {}.", uds_path, e);
                            if (std::path::Path::new(&uds_path)).exists() {
                                info!("Removing {:?}.", uds_path);
                                let _ = std::fs::remove_file(&uds_path).or_else(|e| {
                                    warn!("{}", e);
                                    Err(e)
                                });
                                info!("Binding again to {:?}.", uds_path);
                                tokio_uds::UnixListener::bind(&uds_path)
                            } else {
                                Err(e)
                            }
                        })
                        .map_err(|e| {
                            error!("Cannot bind to: {:?}. Please run with --user to create and use a unix domain socket in the user home directory", uds_path);
                            e
                        })
                        .unwrap();

                    let keys = EthAccount::load_or_generate(keystore_path, "").unwrap();

                    //let _ = server.bind(config.p2p_addr()).unwrap().start();
                    let _ = server.start_incoming(listener.incoming(), false);

                    act.node_id = Some(get_node_id(keys));
                    act.p2p_port = Some(config.p2p_port);

                    // Init mDNS publisher
                    act.mdns_publisher = MdnsPublisher::init_publisher(
                        config.p2p_port,
                        act.node_id.unwrap().to_string(),
                        false,
                    );
                    act.publish_service(config.publish_service);

                    let connect =
                        ConnectManager::init(act.node_id.unwrap(), config.hub_addrs).start();
                    connect.do_send(AutoMdns(config.connect_mode == ConnectMode::Auto));
                    act.connections = Some(connect);

                    future::ok(()).into_actor(act)
                }),
        )
    }
}

fn optional_save_future<F, R>(f: F, save: bool) -> impl Future<Item = Option<()>, Error = String>
where
    F: FnOnce() -> R,
    R: Future<Item = Option<()>, Error = String>,
{
    if save {
        future::Either::A(f())
    } else {
        future::Either::B(future::ok(None))
    }
}

impl Handler<ConnectModeMessage> for ProviderServer {
    type Result = ActorResponse<Self, Option<()>, String>;

    fn handle(&mut self, msg: ConnectModeMessage, _ctx: &mut Context<Self>) -> Self::Result {
        if let Some(ref connections) = self.connections {
            let mode = msg.mode.clone();
            let config_fut =
                optional_save_future(move || connect::edit_config_connect_mode(mode), msg.save);
            let state_fut = connections
                .send(AutoMdns(msg.mode == ConnectMode::Auto))
                .map_err(|e| e.to_string())
                .and_then(|r| r);

            return ActorResponse::r#async(
                config_fut
                    .join(state_fut)
                    .map_err(|e| e.to_string())
                    .and_then(|a| {
                        Ok(match a {
                            (None, None) => None,
                            _ => Some(()),
                        })
                    })
                    .into_actor(self),
            );
        }

        unreachable!()
    }
}

impl Handler<ListSockets> for ProviderServer {
    type Result = ActorResponse<Self, Vec<(SocketAddr, ListingType)>, String>;

    fn handle(&mut self, msg: ListSockets, _ctx: &mut Context<Self>) -> Self::Result {
        if let Some(ref connections) = self.connections {
            ActorResponse::r#async(
                connections
                    .send(msg)
                    .map_err(|e| e.to_string())
                    .and_then(|r| r)
                    .into_actor(self),
            )
        } else {
            unreachable!()
        }
    }
}

impl Handler<ConnectionChangeMessage> for ProviderServer {
    type Result = ActorResponse<Self, Option<()>, String>;

    fn handle(&mut self, msg: ConnectionChangeMessage, _ctx: &mut Context<Self>) -> Self::Result {
        let msg2 = msg.clone();
        let save = msg.save;
        let config_fut = optional_save_future(
            move || connect::edit_config_hosts(msg2.hubs, msg2.change, false),
            save,
        );

        if let Some(ref connections) = self.connections {
            let connections = connections.clone();
            let state_fut = match msg.change {
                ConnectionChange::Connect => {
                    future::Either::A(future::join_all(msg.hubs.into_iter().map(move |hub| {
                        connections.send(Connect(hub)).map_err(|e| e.to_string())
                    })))
                }
                ConnectionChange::Disconnect => {
                    future::Either::B(future::join_all(msg.hubs.into_iter().map(move |hub| {
                        connections
                            .send(Disconnect(hub))
                            .map_err(|e| e.to_string())
                            .and_then(|a| a)
                    })))
                }
            };

            return ActorResponse::r#async(
                config_fut
                    .and_then(|_| state_fut)
                    .and_then(|_| Ok(None))
                    .into_actor(self),
            );
        }

        unreachable!()
    }
}
